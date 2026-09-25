//! Shared resource loading and per-webview protocol configuration.
//!
//! Configure `ProtocolSystem` once with `FrontendDist` and a live Tokio runtime.
//! Each webview then supplies only its `WebviewUrl` to `ProtocolSystem::apply`.
//! Backends return resource responses; the per-webview adapter applies CORS.
//!
//! Scope: trusted, read-only application assets. This is not an IPC permission
//! system, a general HTTP reverse proxy, or a sandbox for a concurrently hostile
//! filesystem. See protocol_system_README.md for integration and limitations.

use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Debug},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use http::{
    HeaderMap, HeaderName, HeaderValue, Method, Request, Response, StatusCode,
    header::{
        ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
        ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, ALLOW, CONNECTION,
        CONTENT_LENGTH, CONTENT_TYPE, HOST, ORIGIN,
    },
};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Deserializer, Serialize};
use tokio::{fs, runtime::Handle, sync::Mutex, time::timeout};
use url::Url;
use wry::{RequestAsyncResponder, WebViewBuilder};

#[cfg(target_os = "android")]
use wry::WebViewBuilderExtAndroid;
#[cfg(target_os = "windows")]
use wry::WebViewBuilderExtWindows;

// Keep the application's existing image types. No WebviewConfig dependency is needed.
use crate::{
    image::{Icon, Image},
    schema::{FrontendDist, webview::WebviewUrl},
};

pub type StaticFileResponse = Response<Cow<'static, [u8]>>;
type IconCache = Mutex<HashMap<String, Icon<'static>>>;

const PROTOCOL: &str = "taurino";
const APP_HOST: &str = "localhost";
const APP_BASE: &str = "taurino://localhost/";
const EMPTY_BODY: Cow<'static, [u8]> = Cow::Borrowed(&[]);
const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
const ALLOWED_PREFLIGHT_HEADERS: &str =
    "range, if-none-match, if-modified-since, cache-control, pragma";

/// Backend contract. A manager can be shared by multiple webviews.
///
/// No window origin is stored here: a source and a browser origin are different
/// concepts. ProtocolSystem attaches the window-specific response headers.
#[async_trait]
pub trait ResourceManager: Debug + Send + Sync {
    /// An owned snapshot of the actual, normalized resource source.
    fn frontend_dist(&self) -> FrontendDist;

    async fn exists(&self, path: &str) -> bool;
    async fn load(&self, path: &str) -> Result<Vec<u8>>;
    async fn extract(&self, from: &str, to: &Path) -> Result<()>;
    async fn load_icon(&self, path: &str) -> Result<Icon<'static>>;
    async fn respond(&self, request: Request<Vec<u8>>) -> Result<StaticFileResponse>;
}

/// Loads application assets from a configured filesystem directory.
#[derive(Debug)]
pub struct FileSystemResource {
    root_dir: PathBuf,
    icon_cache: IconCache,
}

impl FileSystemResource {
    pub async fn new(root_dir: impl AsRef<Path>) -> Result<Arc<Self>> {
        let requested_root = root_dir.as_ref();
        let root_dir = fs::canonicalize(requested_root).await.with_context(|| {
            format!(
                "failed to resolve resource directory {}",
                requested_root.display()
            )
        })?;
        if !fs::metadata(&root_dir).await?.is_dir() {
            bail!("resource path is not a directory: {}", root_dir.display());
        }
        Ok(Arc::new(Self {
            root_dir,
            icon_cache: Mutex::new(HashMap::new()),
        }))
    }

    fn resolve(&self, path: &str) -> Result<PathBuf> {
        let relative = validate_resource_path(path)?;
        Ok(self.root_dir.join(relative.as_ref()))
    }

    /// Checks symlink targets as well as lexical traversal.
    /// The root must remain trusted: canonicalize + open is not race-free against
    /// an attacker concurrently replacing filesystem entries.
    async fn resolve_existing(&self, path: &str) -> Result<PathBuf> {
        let candidate = self.resolve(path)?;
        let canonical = fs::canonicalize(&candidate)
            .await
            .with_context(|| format!("failed to resolve resource {}", candidate.display()))?;
        if !canonical.starts_with(&self.root_dir) {
            bail!("resource resolves outside the configured directory");
        }
        Ok(canonical)
    }
}

#[async_trait]
impl ResourceManager for FileSystemResource {
    fn frontend_dist(&self) -> FrontendDist {
        FrontendDist::Directory(self.root_dir.clone())
    }

    async fn exists(&self, path: &str) -> bool {
        let Ok(path) = self.resolve_existing(path).await else {
            return false;
        };
        fs::metadata(path)
            .await
            .map(|metadata| metadata.is_file())
            .unwrap_or(false)
    }

    async fn load(&self, path: &str) -> Result<Vec<u8>> {
        let path = self.resolve_existing(path).await?;
        fs::read(&path)
            .await
            .with_context(|| format!("failed to read resource {}", path.display()))
    }

    async fn extract(&self, from: &str, to: &Path) -> Result<()> {
        let source = self.resolve_existing(from).await?;
        create_parent_directory(to).await?;
        fs::copy(&source, to)
            .await
            .with_context(|| format!("failed to extract resource to {}", to.display()))?;
        Ok(())
    }

    async fn load_icon(&self, path: &str) -> Result<Icon<'static>> {
        load_cached_icon(self, &self.icon_cache, path).await
    }

    async fn respond(&self, request: Request<Vec<u8>>) -> Result<StaticFileResponse> {
        if request.method() != Method::GET && request.method() != Method::HEAD {
            return Ok(method_not_allowed());
        }

        // The URL's query does not belong to the filesystem path.
        let request_path = request.uri().path();
        let asset_path = if request_path.is_empty() || request_path.ends_with('/') {
            format!("{request_path}index.html")
        } else {
            request_path.to_owned()
        };

        if self.resolve(&asset_path).is_err() {
            return Ok(empty_response(StatusCode::FORBIDDEN));
        }
        let resolved = match self.resolve_existing(&asset_path).await {
            Ok(path) => path,
            Err(error) if is_not_found(&error) => {
                return Ok(empty_response(StatusCode::NOT_FOUND));
            }
            Err(error) => return Err(error),
        };
        let metadata = match fs::metadata(&resolved).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(empty_response(StatusCode::NOT_FOUND));
            }
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() {
            return Ok(empty_response(StatusCode::NOT_FOUND));
        }

        let content_type = mime_guess::from_path(&resolved)
            .first_or_octet_stream()
            .to_string();
        let (body, content_length) = if request.method() == Method::HEAD {
            (EMPTY_BODY, metadata.len())
        } else {
            let bytes = fs::read(&resolved).await?;
            let length = bytes.len() as u64;
            (Cow::Owned(bytes), length)
        };

        let mut response = Response::new(body);
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_str(&content_type)?);
        // Insert once. Builder::header appends and could produce duplicate lengths.
        response.headers_mut().insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&content_length.to_string())?,
        );
        Ok(response)
    }
}

/// Loads read-only application assets from one configured HTTP(S) base directory.
///
/// This is not a general API, cookie-domain, or WebSocket proxy. GET and HEAD
/// are supported. Remote HTTP cache headers are passed through; no URL-only
/// shared body cache is used across windows or authorization contexts.
#[derive(Debug)]
pub struct RemoteHttpResource {
    base_url: Url,
    client: reqwest::Client,
    icon_cache: IconCache,
}

impl RemoteHttpResource {
    pub fn new(base_url: Url) -> Result<Arc<Self>> {
        let base_url = normalize_remote_base(base_url)?;
        let redirect_base = base_url.clone();
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 10 {
                    attempt.error("too many resource redirects")
                } else if remote_relative_reference(&redirect_base, attempt.url()).is_none() {
                    attempt.error("resource redirect escaped the configured base")
                } else {
                    attempt.follow()
                }
            }))
            // Keep encoded bytes consistent with Content-Encoding when proxying.
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .context("failed to create remote resource client")?;

        Ok(Arc::new(Self {
            base_url,
            client,
            icon_cache: Mutex::new(HashMap::new()),
        }))
    }

    fn resolve(&self, path_and_query: &str) -> Result<Url> {
        let (path, query) = path_and_query
            .split_once('?')
            .map_or((path_and_query, None), |(path, query)| (path, Some(query)));
        let _ = validate_resource_path(path)?;
        if path.contains('#') {
            bail!("resource request paths must not contain an unescaped fragment");
        }
        let mut url = self.base_url.join(path.trim_start_matches('/'))?;
        url.set_query(query);
        url.set_fragment(None);
        if remote_relative_reference(&self.base_url, &url).is_none() {
            bail!("resource path escaped the configured remote base");
        }
        Ok(url)
    }
}

#[async_trait]
impl ResourceManager for RemoteHttpResource {
    fn frontend_dist(&self) -> FrontendDist {
        FrontendDist::Url(self.base_url.clone())
    }

    async fn exists(&self, path: &str) -> bool {
        let Ok(url) = self.resolve(path) else {
            return false;
        };
        match self.client.head(url.clone()).send().await {
            Ok(response) if response.status().is_success() => true,
            Ok(response)
                if response.status() == StatusCode::METHOD_NOT_ALLOWED
                    || response.status() == StatusCode::NOT_IMPLEMENTED =>
            {
                self.client
                    .get(url)
                    .header(http::header::RANGE, "bytes=0-0")
                    .send()
                    .await
                    .map(|response| response.status().is_success())
                    .unwrap_or(false)
            }
            _ => false,
        }
    }

    async fn load(&self, path: &str) -> Result<Vec<u8>> {
        let url = self.resolve(path)?;
        let response = self
            .client
            .get(url)
            // Native callers need plain file bytes for images and extraction.
            .header(http::header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .context("failed to request remote resource")?
            .error_for_status()
            .context("remote resource returned an error status")?;
        if let Some(encoding) = response.headers().get(http::header::CONTENT_ENCODING) {
            if encoding != "identity" {
                bail!("remote server ignored the requested identity content encoding");
            }
        }
        Ok(response.bytes().await?.to_vec())
    }

    async fn extract(&self, from: &str, to: &Path) -> Result<()> {
        let bytes = self.load(from).await?;
        create_parent_directory(to).await?;
        fs::write(to, bytes)
            .await
            .with_context(|| format!("failed to extract resource to {}", to.display()))?;
        Ok(())
    }

    async fn load_icon(&self, path: &str) -> Result<Icon<'static>> {
        load_cached_icon(self, &self.icon_cache, path).await
    }

    async fn respond(&self, request: Request<Vec<u8>>) -> Result<StaticFileResponse> {
        if request.method() != Method::GET && request.method() != Method::HEAD {
            return Ok(method_not_allowed());
        }
        let path_and_query = request
            .uri()
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/");
        let url = match self.resolve(path_and_query) {
            Ok(url) => url,
            Err(_) => return Ok(empty_response(StatusCode::FORBIDDEN)),
        };
        let is_head = request.method() == Method::HEAD;
        let mut headers = copy_end_to_end_headers(request.headers());
        headers.remove(HOST);
        headers.remove(CONTENT_LENGTH);

        let upstream = self
            .client
            .request(request.method().clone(), url)
            .headers(headers)
            .send()
            .await
            .context("failed to fetch upstream resource")?;
        let status = upstream.status();
        let mut headers = copy_end_to_end_headers(upstream.headers());
        let has_no_body = is_head
            || status == StatusCode::NO_CONTENT
            || status == StatusCode::NOT_MODIFIED
            || status.is_informational();
        let body = if has_no_body {
            EMPTY_BODY
        } else {
            let bytes = upstream.bytes().await?.to_vec();
            headers.insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&bytes.len().to_string())?,
            );
            Cow::Owned(bytes)
        };
        if status == StatusCode::NO_CONTENT || status.is_informational() {
            headers.remove(CONTENT_LENGTH);
        }
        let mut response = Response::new(body);
        *response.status_mut() = status;
        *response.headers_mut() = headers;
        Ok(response)
    }
}

/// Build a backend once. No webview URL or window origin is needed here.
pub async fn create_resource_manager(source: FrontendDist) -> Result<Arc<dyn ResourceManager>> {
    match source {
        FrontendDist::Directory(path) => Ok(FileSystemResource::new(path).await?),
        FrontendDist::Url(url) => Ok(RemoteHttpResource::new(url)?),
    }
}

/// Settings shared by all webviews using one ProtocolSystem.
#[derive(Debug, Clone)]
pub struct ProtocolOptions {
    /// WRY's custom-protocol HTTP/HTTPS mapping on Windows and Android.
    pub use_https_scheme: bool,
    /// Maximum time for one custom-protocol response.
    pub request_timeout: Duration,
}

impl Default for ProtocolOptions {
    fn default() -> Self {
        Self {
            use_https_scheme: false,
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// The result of examining a WebviewUrl. Only App gets the app asset protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedWebview {
    App(Url),
    External(Url),
    /// The caller must register the third-party scheme's handler separately.
    CustomProtocol(Url),
    /// Decoded, validated UTF-8 HTML. Loaded with WRY's with_html, not with_url.
    Html(String),
}

/// Owns the shared resource source and runtime; each call to apply binds one view.
#[derive(Debug, Clone)]
pub struct ProtocolSystem {
    resources: Arc<dyn ResourceManager>,
    frontend_dist: FrontendDist,
    runtime: Handle,
    options: ProtocolOptions,
}

impl ProtocolSystem {
    pub async fn new(
        frontend_dist: FrontendDist,
        runtime: Handle,
        options: ProtocolOptions,
    ) -> Result<Self> {
        let resources = create_resource_manager(frontend_dist).await?;
        Self::from_resource_manager(resources, runtime, options)
    }

    pub fn from_resource_manager(
        resources: Arc<dyn ResourceManager>,
        runtime: Handle,
        options: ProtocolOptions,
    ) -> Result<Self> {
        if options.request_timeout.is_zero() {
            bail!("protocol request_timeout must be greater than zero");
        }
        // The GUI event loop may occupy the main thread for the whole app life.
        // Require worker threads instead of depending on a polled current-thread runtime.
        if !matches!(
            runtime.runtime_flavor(),
            tokio::runtime::RuntimeFlavor::MultiThread
        ) {
            bail!("ProtocolSystem requires a live Tokio multi-thread runtime");
        }
        let frontend_dist = match resources.frontend_dist() {
            FrontendDist::Url(url) => FrontendDist::Url(normalize_remote_base(url)?),
            source => source,
        };
        Ok(Self {
            resources,
            frontend_dist,
            runtime,
            options,
        })
    }

    pub fn resources(&self) -> &Arc<dyn ResourceManager> {
        &self.resources
    }

    /// Examines a destination without constructing a native WebView.
    pub fn resolve(&self, webview_url: &WebviewUrl) -> Result<ResolvedWebview> {
        resolve_webview(
            webview_url,
            &self.frontend_dist,
            self.options.use_https_scheme,
        )
    }

    /// Set the content before build(), on the GUI thread.
    ///
    /// Pass a builder without a URL or HTML already set. In particular, WRY's
    /// with_html does not override a previously configured URL.
    /// External/HTML/third-party-protocol views do not receive this system's
    /// internal asset handler. They do not automatically acquire app privileges.
    pub fn apply<'a>(
        &self,
        builder: WebViewBuilder<'a>,
        webview_url: &WebviewUrl,
    ) -> Result<WebViewBuilder<'a>> {
        match self.resolve(webview_url)? {
            ResolvedWebview::App(url) => {
                let handler = Arc::new(WindowProtocolHandler {
                    resources: Arc::clone(&self.resources),
                    context: WebResponseContext::new(&url, self.options.use_https_scheme)?,
                    request_timeout: self.options.request_timeout,
                    upstream_error_status: match &self.frontend_dist {
                        FrontendDist::Directory(_) => StatusCode::INTERNAL_SERVER_ERROR,
                        FrontendDist::Url(_) => StatusCode::BAD_GATEWAY,
                    },
                });
                let runtime = self.runtime.clone();
                let builder = configure_https_scheme(builder, self.options.use_https_scheme);
                let builder = builder.with_asynchronous_custom_protocol(
                    PROTOCOL.to_owned(),
                    move |_webview_id, request, responder| {
                        let handler = Arc::clone(&handler);
                        // Construct the guard before spawn. If the task is cancelled,
                        // dropped during shutdown, or unwinds, WRY still gets a reply.
                        let reply = PendingResponse::new(responder);
                        let task = runtime.spawn(async move {
                            let response = handler.respond(request).await;
                            reply.finish(response);
                        });
                        // Dropping JoinHandle detaches the task; it does not cancel it.
                        drop(task);
                    },
                );
                // Keep the canonical custom-scheme URL here. WRY performs its own
                // platform-specific HTTP(S) transformation when building the view.
                Ok(builder.with_url(url.as_str()))
            }
            ResolvedWebview::External(url) | ResolvedWebview::CustomProtocol(url) => {
                Ok(builder.with_url(url.as_str()))
            }
            ResolvedWebview::Html(html) => Ok(builder.with_html(html)),
        }
    }
}

/// Convenience wrapper for code that previously called apply_protocol_system.
/// The only per-webview content argument is webview_url.
pub fn apply_protocol_system<'a>(
    builder: WebViewBuilder<'a>,
    webview_url: &WebviewUrl,
    system: &ProtocolSystem,
) -> Result<WebViewBuilder<'a>> {
    system.apply(builder, webview_url)
}

fn configure_https_scheme<'a>(builder: WebViewBuilder<'a>, https: bool) -> WebViewBuilder<'a> {
    #[cfg(any(target_os = "windows", target_os = "android"))]
    {
        builder.with_https_scheme(https)
    }
    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    {
        let _ = https;
        builder
    }
}

fn resolve_webview(
    content: &WebviewUrl,
    frontend_dist: &FrontendDist,
    use_https_scheme: bool,
) -> Result<ResolvedWebview> {
    match content {
        WebviewUrl::App(path) => {
            let path = path.to_str().context("app URL path must be valid UTF-8")?;
            Ok(ResolvedWebview::App(app_url_from_reference(path)?))
        }
        WebviewUrl::External(url) => {
            if !matches!(url.scheme(), "http" | "https") {
                bail!("WebviewUrl::External requires an HTTP or HTTPS URL");
            }
            if is_platform_app_url(url, use_https_scheme) {
                return Ok(ResolvedWebview::App(app_url_from_reference(
                    &url_reference(url),
                )?));
            }
            if let FrontendDist::Url(base) = frontend_dist {
                // Match both origin and the configured base path. An IP address
                // does not imply a trusted app URL; make_relative() alone is not a scope check.
                if let Some(relative) = remote_relative_reference(base, url) {
                    return Ok(ResolvedWebview::App(app_url_from_reference(&relative)?));
                }
            }
            Ok(ResolvedWebview::External(url.clone()))
        }
        WebviewUrl::CustomProtocol(url) => match url.scheme() {
            PROTOCOL => {
                validate_app_authority(url)?;
                Ok(ResolvedWebview::App(app_url_from_reference(
                    &url_reference(url),
                )?))
            }
            "data" => Ok(ResolvedWebview::Html(decode_html_data_url(url)?)),
            "http" | "https" => resolve_webview(
                &WebviewUrl::External(url.clone()),
                frontend_dist,
                use_https_scheme,
            ),
            "file" | "javascript" => {
                bail!("direct file: and javascript: webview URLs are not supported");
            }
            _ => Ok(ResolvedWebview::CustomProtocol(url.clone())),
        },
    }
}

fn app_url_from_reference(reference: &str) -> Result<Url> {
    let raw_path = reference.split(['?', '#']).next().unwrap_or("");
    let _ = validate_resource_path(raw_path)?;
    let base = Url::parse(APP_BASE).context("invalid internal app base constant")?;
    let mut url = base.join(reference)?;
    validate_app_authority(&url)?;
    // Keep query and fragment while simplifying the default entry point.
    if url.path() == "/index.html" {
        url.set_path("/");
    }
    Ok(url)
}

fn validate_app_authority(url: &Url) -> Result<()> {
    if url.scheme() != PROTOCOL
        || url.host_str() != Some(APP_HOST)
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        bail!("app URLs must use taurino://localhost without credentials or a port");
    }
    Ok(())
}

fn is_platform_app_url(url: &Url, use_https_scheme: bool) -> bool {
    if !cfg!(any(target_os = "windows", target_os = "android")) {
        return false;
    }
    let scheme = if use_https_scheme { "https" } else { "http" };
    url.scheme() == scheme
        && url.host_str() == Some("taurino.localhost")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}

fn url_reference(url: &Url) -> String {
    append_query_and_fragment(url.path().to_owned(), url)
}

fn append_query_and_fragment(mut path: String, url: &Url) -> String {
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    if let Some(fragment) = url.fragment() {
        path.push('#');
        path.push_str(fragment);
    }
    path
}

fn normalize_remote_base(mut url: Url) -> Result<Url> {
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("remote frontend source must be an absolute HTTP(S) URL");
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("remote frontend base must not contain credentials, a query, or a fragment");
    }
    let _ = validate_resource_path(url.path())?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

/// A prefix with a trailing slash prevents /assets2 from matching /assets/.
fn remote_relative_reference(base: &Url, target: &Url) -> Option<String> {
    if target.scheme() != base.scheme()
        || target.host_str() != base.host_str()
        || target.port_or_known_default() != base.port_or_known_default()
        || !target.username().is_empty()
        || target.password().is_some()
    {
        return None;
    }
    let relative = if target.path() == base.path().trim_end_matches('/') {
        ""
    } else {
        target.path().strip_prefix(base.path())?
    };
    validate_resource_path(relative).ok()?;
    Some(append_query_and_fragment(relative.to_owned(), target))
}

/// The browser origin of an owned app URL or a standard URL.
/// Third-party custom schemes have no assumed origin mapping here: their
/// registration is outside this system. `null` is not an authorization token.
pub fn get_window_origin(url: &Url, use_https_scheme: bool) -> String {
    if url.scheme() == PROTOCOL {
        if validate_app_authority(url).is_err() {
            return "null".to_owned();
        }
        if cfg!(any(target_os = "windows", target_os = "android")) {
            let scheme = if use_https_scheme { "https" } else { "http" };
            format!("{scheme}://taurino.localhost")
        } else {
            "taurino://localhost".to_owned()
        }
    } else {
        // URL's standard origin serializer also handles IPv6 and default ports.
        url.origin().ascii_serialization()
    }
}

fn decode_html_data_url(url: &Url) -> Result<String> {
    // Bound the encoded input as well as the decoded output.
    if url.as_str().len() > MAX_HTML_BYTES * 3 + 1024 {
        bail!("HTML data URL exceeds the configured size limit");
    }
    if url.fragment().is_some() {
        bail!("HTML data URL fragments are not supported by this with_html adapter");
    }
    let data = data_url::DataUrl::process(url.as_str())
        .map_err(|error| anyhow::anyhow!("invalid data URL: {error:?}"))?;
    let mime: mime::Mime = data
        .mime_type()
        .to_string()
        .parse()
        .context("invalid data URL media type")?;
    if mime.type_() != mime::TEXT || mime.subtype() != mime::HTML {
        bail!("only data:text/html is supported as inline webview content");
    }
    if let Some(charset) = mime.get_param(mime::CHARSET) {
        let charset = charset.as_str();
        if !charset.eq_ignore_ascii_case("utf-8") && !charset.eq_ignore_ascii_case("us-ascii") {
            bail!("inline HTML must use UTF-8 or US-ASCII");
        }
    }
    let mut bytes = Vec::new();
    data.decode(|chunk| -> std::result::Result<(), &'static str> {
        if chunk.len() > MAX_HTML_BYTES.saturating_sub(bytes.len()) {
            return Err("decoded HTML exceeds 2 MiB");
        }
        bytes.extend_from_slice(chunk);
        Ok(())
    })
    .map_err(|error| anyhow::anyhow!("failed to decode HTML data URL: {error:?}"))?;
    if mime
        .get_param(mime::CHARSET)
        .is_some_and(|charset| charset.as_str().eq_ignore_ascii_case("us-ascii"))
        && !bytes.is_ascii()
    {
        bail!("HTML declares US-ASCII but contains non-ASCII bytes");
    }
    String::from_utf8(bytes).context("inline HTML is not valid UTF-8")
}

/// Validate before URL normalization can erase traversal components.
/// Resource paths are URL paths, not arbitrary OS paths. Nested percent escapes,
/// encoded separators, backslashes, colons, and controls are deliberately rejected.
fn validate_resource_path(path: &str) -> Result<Cow<'_, str>> {
    if path.starts_with("//") || path.contains('\\') || path.chars().any(char::is_control) {
        bail!("resource path contains an authority, backslash, or control character");
    }
    let lower = path.to_ascii_lowercase();
    if lower.contains("%2f") || lower.contains("%5c") || lower.contains("%25") {
        bail!("encoded separators and nested percent escapes are not accepted");
    }
    let decoded = percent_decode_str(path.trim_start_matches('/'))
        .decode_utf8()
        .context("resource path is not valid UTF-8")?;
    if decoded.contains(':') || decoded.contains('\\') || decoded.chars().any(char::is_control) {
        bail!("resource path contains a drive prefix, stream name, or control character");
    }
    for segment in decoded.split('/') {
        if segment == ".." {
            bail!("resource path must not contain parent traversal");
        }
        // Avoid Windows path aliases (including `.. `), alternate streams, and
        // normalization differences between platforms for served app assets.
        if !segment.is_empty()
            && segment != "."
            && (segment.ends_with('.') || segment.ends_with(' '))
        {
            bail!("resource path segments must not end with a dot or space");
        }
    }
    Ok(decoded)
}

#[derive(Debug, Clone)]
struct WebResponseContext {
    origin: HeaderValue,
    use_https_scheme: bool,
}

impl WebResponseContext {
    fn new(window_url: &Url, use_https_scheme: bool) -> Result<Self> {
        validate_app_authority(window_url)?;
        let origin = HeaderValue::from_str(&get_window_origin(window_url, use_https_scheme))?;
        Ok(Self {
            origin,
            use_https_scheme,
        })
    }

    fn accepts(&self, request: &Request<Vec<u8>>) -> bool {
        if request.headers().get_all(ORIGIN).iter().count() > 1 {
            return false;
        }
        if let Some(origin) = request.headers().get(ORIGIN) {
            if origin != &self.origin {
                return false;
            }
        }
        match (request.uri().scheme_str(), request.uri().authority()) {
            (None, None) => true, // Origin-form URI; useful for tests and adapters.
            (Some(_), Some(_)) => Url::parse(&request.uri().to_string())
                .map(|url| {
                    validate_app_authority(&url).is_ok()
                        || is_platform_app_url(&url, self.use_https_scheme)
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    fn apply(&self, mut response: StaticFileResponse, is_head: bool) -> StaticFileResponse {
        // A remote server must not widen the native app's CORS policy.
        let old_cors: Vec<HeaderName> = response
            .headers()
            .keys()
            .filter(|name| name.as_str().starts_with("access-control-"))
            .cloned()
            .collect();
        for name in old_cors {
            response.headers_mut().remove(name);
        }
        response
            .headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_ORIGIN, self.origin.clone());
        if is_head {
            *response.body_mut() = EMPTY_BODY;
        }
        response
    }

    fn preflight(&self, request: &Request<Vec<u8>>) -> StaticFileResponse {
        let method = request
            .headers()
            .get(ACCESS_CONTROL_REQUEST_METHOD)
            .and_then(|value| value.to_str().ok());
        if !matches!(method, Some("GET" | "HEAD")) {
            return self.apply(method_not_allowed(), false);
        }
        for value in request
            .headers()
            .get_all(ACCESS_CONTROL_REQUEST_HEADERS)
            .iter()
        {
            let Ok(value) = value.to_str() else {
                return empty_response(StatusCode::FORBIDDEN);
            };
            if value.split(',').any(|requested| {
                !ALLOWED_PREFLIGHT_HEADERS
                    .split(',')
                    .any(|allowed| allowed.trim().eq_ignore_ascii_case(requested.trim()))
            }) {
                return empty_response(StatusCode::FORBIDDEN);
            }
        }
        let mut response = self.apply(empty_response(StatusCode::NO_CONTENT), false);
        response.headers_mut().insert(
            ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, HEAD"),
        );
        response.headers_mut().insert(
            ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static(ALLOWED_PREFLIGHT_HEADERS),
        );
        response
    }
}

#[derive(Debug)]
struct WindowProtocolHandler {
    resources: Arc<dyn ResourceManager>,
    context: WebResponseContext,
    request_timeout: Duration,
    upstream_error_status: StatusCode,
}

impl WindowProtocolHandler {
    async fn respond(&self, request: Request<Vec<u8>>) -> StaticFileResponse {
        if !self.context.accepts(&request) {
            return empty_response(StatusCode::FORBIDDEN);
        }
        let is_head = request.method() == Method::HEAD;
        if request.method() == Method::OPTIONS {
            return self.context.preflight(&request);
        }
        if request.method() != Method::GET && !is_head {
            return self.context.apply(method_not_allowed(), is_head);
        }
        let response = match timeout(self.request_timeout, self.resources.respond(request)).await {
            Ok(Ok(response)) => response,
            Ok(Err(_error)) => {
                // Do not leak paths, upstream credentials, or internal diagnostics to JS.
                empty_response(self.upstream_error_status)
            }
            Err(_) => empty_response(StatusCode::GATEWAY_TIMEOUT),
        };
        self.context.apply(response, is_head)
    }
}

/// Exactly one completion in normal execution, including task cancellation.
/// An aborting process or a platform responder failure cannot be recovered here.
struct PendingResponse {
    responder: Option<RequestAsyncResponder>,
}

impl PendingResponse {
    fn new(responder: RequestAsyncResponder) -> Self {
        Self {
            responder: Some(responder),
        }
    }

    fn finish(mut self, response: StaticFileResponse) {
        if let Some(responder) = self.responder.take() {
            responder.respond(response);
        }
    }
}

impl Drop for PendingResponse {
    fn drop(&mut self) {
        if let Some(responder) = self.responder.take() {
            // Fail closed: the cancellation response contains no permissive CORS header.
            responder.respond(empty_response(StatusCode::SERVICE_UNAVAILABLE));
        }
    }
}

fn empty_response(status: StatusCode) -> StaticFileResponse {
    let mut response = Response::new(EMPTY_BODY);
    *response.status_mut() = status;
    response
}

fn method_not_allowed() -> StaticFileResponse {
    let mut response = empty_response(StatusCode::METHOD_NOT_ALLOWED);
    response
        .headers_mut()
        .insert(ALLOW, HeaderValue::from_static("GET, HEAD, OPTIONS"));
    response
}

fn is_not_found(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

async fn create_parent_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    Ok(())
}

async fn load_cached_icon(
    manager: &dyn ResourceManager,
    cache: &IconCache,
    path: &str,
) -> Result<Icon<'static>> {
    if let Some(icon) = cache.lock().await.get(path).cloned() {
        return Ok(icon);
    }
    let bytes = manager.load(path).await?;
    let image = Image::from_bytes(&bytes)
        .with_context(|| format!("failed to decode icon resource {path}"))?;
    let icon: Icon<'static> = image.into();
    let mut cache = cache.lock().await;
    Ok(cache.entry(path.to_owned()).or_insert(icon).clone())
}

/// Preserve duplicate end-to-end headers, but remove transport headers and
/// any additional fields nominated by Connection. Both requests and responses use this.
fn copy_end_to_end_headers(source: &HeaderMap) -> HeaderMap {
    let connection_fields: Vec<HeaderName> = source
        .get_all(CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|name| HeaderName::from_bytes(name.trim().as_bytes()).ok())
        .collect();
    let mut destination = HeaderMap::new();
    for (name, value) in source {
        if matches!(
            name.as_str(),
            "connection"
                | "keep-alive"
                | "proxy-authenticate"
                | "proxy-authorization"
                | "proxy-connection"
                | "te"
                | "trailer"
                | "transfer-encoding"
                | "upgrade"
        ) || connection_fields.contains(name)
        {
            continue;
        }
        destination.append(name.clone(), value.clone());
    }
    destination
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory_source() -> FrontendDist {
        FrontendDist::Directory("dist".into())
    }

    fn remote_source() -> FrontendDist {
        FrontendDist::Url(Url::parse("https://example.com/assets/").unwrap())
    }

    fn external(value: &str) -> WebviewUrl {
        WebviewUrl::External(Url::parse(value).unwrap())
    }

    fn custom(value: &str) -> WebviewUrl {
        WebviewUrl::CustomProtocol(Url::parse(value).unwrap())
    }

    fn app(value: &str) -> ResolvedWebview {
        ResolvedWebview::App(Url::parse(value).unwrap())
    }

    fn request(method: Method, uri: &str) -> Request<Vec<u8>> {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Vec::new())
            .unwrap()
    }

    fn context(https: bool) -> WebResponseContext {
        WebResponseContext::new(&Url::parse(APP_BASE).unwrap(), https).unwrap()
    }

    #[test]
    fn default_entry_point_is_the_app_root() {
        assert_eq!(
            resolve_webview(&WebviewUrl::default(), &directory_source(), false).unwrap(),
            app(APP_BASE),
        );
    }

    #[test]
    fn nested_app_path_query_and_fragment_are_preserved() {
        let input = WebviewUrl::App("settings/index.html?theme=dark#main".into());
        assert_eq!(
            resolve_webview(&input, &directory_source(), false).unwrap(),
            app("taurino://localhost/settings/index.html?theme=dark#main"),
        );
    }

    #[test]
    fn default_entry_simplification_keeps_query_and_fragment() {
        assert_eq!(
            app_url_from_reference("index.html?q=1#top")
                .unwrap()
                .as_str(),
            "taurino://localhost/?q=1#top",
        );
    }

    #[test]
    fn empty_app_path_is_the_root() {
        assert_eq!(app_url_from_reference("").unwrap().as_str(), APP_BASE);
    }

    #[test]
    fn root_relative_app_path_remains_inside_the_app() {
        assert_eq!(
            app_url_from_reference("/nested/page.html")
                .unwrap()
                .as_str(),
            "taurino://localhost/nested/page.html",
        );
    }

    #[test]
    fn app_paths_do_not_depend_on_the_backend() {
        let input = WebviewUrl::App("page.html".into());
        assert_eq!(
            resolve_webview(&input, &directory_source(), false).unwrap(),
            resolve_webview(&input, &remote_source(), false).unwrap(),
        );
    }

    #[test]
    fn external_url_inside_remote_base_becomes_an_app_route() {
        assert_eq!(
            resolve_webview(
                &external("https://example.com/assets/dashboard?q=1#tab"),
                &remote_source(),
                false,
            )
            .unwrap(),
            app("taurino://localhost/dashboard?q=1#tab"),
        );
    }

    #[test]
    fn external_url_outside_remote_base_stays_external() {
        for value in [
            "https://example.com/other/page.html",
            "https://example.com/assets2/page.html",
            "https://other.example/assets/page.html",
            "http://example.com/assets/page.html",
            "https://example.com:8443/assets/page.html",
            "https://user:password@example.com/assets/page.html",
        ] {
            let url = Url::parse(value).unwrap();
            assert_eq!(
                resolve_webview(&WebviewUrl::External(url.clone()), &remote_source(), false)
                    .unwrap(),
                ResolvedWebview::External(url),
                "{value}",
            );
        }
    }

    #[test]
    fn public_ip_does_not_implicitly_make_a_url_local() {
        let url = Url::parse("https://203.0.113.10/app/").unwrap();
        assert_eq!(
            resolve_webview(&WebviewUrl::External(url.clone()), &remote_source(), false).unwrap(),
            ResolvedWebview::External(url),
        );
    }

    #[test]
    fn custom_app_url_is_validated() {
        assert_eq!(
            resolve_webview(
                &custom("taurino://localhost/page.html"),
                &directory_source(),
                false
            )
            .unwrap(),
            app("taurino://localhost/page.html"),
        );
        for value in [
            "taurino://other/page.html",
            "taurino://localhost:123/page.html",
            "taurino://user@localhost/page.html",
        ] {
            assert!(resolve_webview(&custom(value), &directory_source(), false).is_err());
        }
    }

    #[test]
    fn third_party_custom_protocol_is_not_rebound_to_app_resources() {
        let url = Url::parse("doom://game/start").unwrap();
        assert_eq!(
            resolve_webview(
                &WebviewUrl::CustomProtocol(url.clone()),
                &directory_source(),
                false
            )
            .unwrap(),
            ResolvedWebview::CustomProtocol(url),
        );
    }

    #[test]
    fn direct_file_and_javascript_urls_are_rejected() {
        for value in ["file:///tmp/secret.txt", "javascript:alert(1)"] {
            assert!(resolve_webview(&custom(value), &directory_source(), false).is_err());
        }
    }

    #[test]
    fn external_variant_rejects_non_http_urls() {
        let wrong = WebviewUrl::External(Url::parse("file:///tmp/a").unwrap());
        assert!(resolve_webview(&wrong, &directory_source(), false).is_err());
    }

    #[test]
    fn app_path_rejects_traversal_and_authority_changes_before_join() {
        for path in [
            "../secret.txt",
            "%2e%2e/secret.txt",
            "a/../../secret.txt",
            "//evil.example/a",
            "https://evil.example/a",
            "C:/secret.txt",
            "C:\\secret.txt",
            "..%20/secret.txt",
            "%2fetc/passwd",
            "a%5c..%5csecret",
            "%252e%252e/secret",
            "a%00.txt",
        ] {
            assert!(app_url_from_reference(path).is_err(), "accepted {path}");
        }
    }

    #[test]
    fn normal_percent_encoded_filename_is_accepted() {
        assert_eq!(
            validate_resource_path("/hello%20world.txt").unwrap(),
            "hello world.txt"
        );
        assert_eq!(
            app_url_from_reference("hello%20world.html")
                .unwrap()
                .as_str(),
            "taurino://localhost/hello%20world.html",
        );
    }

    #[test]
    fn remote_source_is_normalized_to_a_directory() {
        assert_eq!(
            normalize_remote_base(Url::parse("https://example.com/assets").unwrap())
                .unwrap()
                .as_str(),
            "https://example.com/assets/",
        );
    }

    #[test]
    fn remote_source_rejects_ambiguous_base_configuration() {
        for value in [
            "file:///tmp/assets",
            "https://example.com/assets?token=1",
            "https://example.com/assets#tab",
            "https://user:pass@example.com/assets",
        ] {
            assert!(normalize_remote_base(Url::parse(value).unwrap()).is_err());
        }
    }

    #[tokio::test]
    async fn remote_resource_paths_preserve_query_and_base_prefix() {
        let manager =
            RemoteHttpResource::new(Url::parse("https://example.com/assets").unwrap()).unwrap();
        assert_eq!(
            manager.resolve("/app.js?v=42").unwrap().as_str(),
            "https://example.com/assets/app.js?v=42"
        );
        assert_eq!(
            manager.resolve("/").unwrap().as_str(),
            "https://example.com/assets/"
        );
        assert!(manager.resolve("%2e%2e/secret.txt").is_err());
        assert!(manager.resolve("//evil.example/a").is_err());
        assert!(manager.resolve("https://evil.example/a").is_err());
    }

    #[test]
    fn standard_origins_keep_ipv6_and_non_default_ports() {
        assert_eq!(
            get_window_origin(&Url::parse("https://[::1]:8443/page?q=1").unwrap(), false),
            "https://[::1]:8443"
        );
        assert_eq!(
            get_window_origin(&Url::parse("https://example.com:443/page").unwrap(), true),
            "https://example.com"
        );
        assert_eq!(
            get_window_origin(&Url::parse("data:text/html,hello").unwrap(), true),
            "null"
        );
    }

    #[test]
    fn app_origin_matches_the_platform_scheme_setting() {
        let url = Url::parse(APP_BASE).unwrap();
        if cfg!(any(target_os = "windows", target_os = "android")) {
            assert_eq!(get_window_origin(&url, false), "http://taurino.localhost");
            assert_eq!(get_window_origin(&url, true), "https://taurino.localhost");
        } else {
            assert_eq!(get_window_origin(&url, false), "taurino://localhost");
            assert_eq!(get_window_origin(&url, true), "taurino://localhost");
        }
    }

    #[test]
    fn html_data_url_is_decoded_without_mutating_the_url() {
        let input = "data:text/html;charset=utf-8,%3Ch1%3EHello%3C/h1%3E";
        assert_eq!(
            resolve_webview(&custom(input), &directory_source(), false).unwrap(),
            ResolvedWebview::Html("<h1>Hello</h1>".to_owned()),
        );
    }

    #[test]
    fn base64_html_data_url_is_supported() {
        assert_eq!(
            resolve_webview(
                &custom("data:text/html;base64,PGgxPkhlbGxvPC9oMT4="),
                &directory_source(),
                false
            )
            .unwrap(),
            ResolvedWebview::Html("<h1>Hello</h1>".to_owned()),
        );
    }

    #[test]
    fn data_mime_type_is_not_guessed_from_angle_brackets() {
        assert!(
            resolve_webview(
                &custom("data:text/plain,%3Ch1%3Enot%20HTML%3C/h1%3E"),
                &directory_source(),
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_data_encoding_or_charset_returns_an_error() {
        for value in [
            "data:text/html;base64,%%%",
            "data:text/html,%ff",
            "data:text/html;charset=iso-8859-1,hello",
            "data:text/html;charset=us-ascii,%c3%a4",
            "data:text/html,hello#anchor",
        ] {
            assert!(
                resolve_webview(&custom(value), &directory_source(), false).is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn oversized_inline_html_is_rejected() {
        let input = format!("data:text/html,{}", "a".repeat(MAX_HTML_BYTES + 1));
        assert!(decode_html_data_url(&Url::parse(&input).unwrap()).is_err());
    }

    #[tokio::test]
    async fn local_root_serves_index_html_without_a_global_origin() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("index.html"), b"<h1>app</h1>")
            .await
            .unwrap();
        let manager = FileSystemResource::new(temp.path()).await.unwrap();
        let response = manager
            .respond(request(Method::GET, "/?v=1"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body().as_ref(), b"<h1>app</h1>");
        assert!(!response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN));
        assert_eq!(response.headers().get_all(CONTENT_LENGTH).iter().count(), 1);
    }

    #[tokio::test]
    async fn local_head_preserves_length_without_a_body() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("hello.txt"), b"hello")
            .await
            .unwrap();
        let manager = FileSystemResource::new(temp.path()).await.unwrap();
        let response = manager
            .respond(request(Method::HEAD, "/hello.txt?v=1"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.body().is_empty());
        assert_eq!(response.headers()[CONTENT_LENGTH], "5");
    }

    #[tokio::test]
    async fn local_directory_request_serves_its_index() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("settings")).await.unwrap();
        fs::write(temp.path().join("settings/index.html"), b"settings")
            .await
            .unwrap();
        let manager = FileSystemResource::new(temp.path()).await.unwrap();
        let response = manager
            .respond(request(Method::GET, "/settings/"))
            .await
            .unwrap();
        assert_eq!(response.body().as_ref(), b"settings");
    }

    #[tokio::test]
    async fn local_missing_file_returns_404_instead_of_an_spa_fallback() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("index.html"), b"app")
            .await
            .unwrap();
        let manager = FileSystemResource::new(temp.path()).await.unwrap();
        assert_eq!(
            manager
                .respond(request(Method::GET, "/missing.js"))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn local_post_is_not_supported() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FileSystemResource::new(temp.path()).await.unwrap();
        assert_eq!(
            manager
                .respond(request(Method::POST, "/index.html"))
                .await
                .unwrap()
                .status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_symlink_cannot_escape_the_resource_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), b"secret")
            .await
            .unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            root.path().join("link.txt"),
        )
        .unwrap();
        let manager = FileSystemResource::new(root.path()).await.unwrap();
        assert!(manager.load("link.txt").await.is_err());
        assert!(!manager.exists("link.txt").await);
    }

    #[test]
    fn response_context_replaces_upstream_cors_without_mutating_another_context() {
        let first = WebResponseContext {
            origin: HeaderValue::from_static("https://first.example"),
            use_https_scheme: false,
        };
        let second = WebResponseContext {
            origin: HeaderValue::from_static("https://second.example"),
            use_https_scheme: false,
        };
        let mut source = empty_response(StatusCode::OK);
        source
            .headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
        source.headers_mut().insert(
            "access-control-allow-credentials",
            HeaderValue::from_static("true"),
        );
        let first_response = first.apply(source, false);
        let second_response = second.apply(empty_response(StatusCode::OK), false);
        assert_eq!(
            first_response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://first.example"
        );
        assert_eq!(
            second_response.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://second.example"
        );
        assert!(
            !first_response
                .headers()
                .contains_key("access-control-allow-credentials")
        );
    }

    #[test]
    fn response_context_rejects_an_unrelated_origin_and_invalid_host() {
        let ctx = context(false);
        let mut other_origin = request(Method::GET, "taurino://localhost/app.js");
        other_origin
            .headers_mut()
            .insert(ORIGIN, HeaderValue::from_static("https://evil.example"));
        assert!(!ctx.accepts(&other_origin));
        assert!(!ctx.accepts(&request(Method::GET, "taurino://other/app.js")));
        assert!(ctx.accepts(&request(Method::GET, "taurino://localhost/app.js")));
    }

    #[test]
    fn response_context_rejects_duplicate_origin_headers() {
        let ctx = context(false);
        let mut req = request(Method::GET, "/app.js");
        req.headers_mut().append(ORIGIN, ctx.origin.clone());
        req.headers_mut().append(ORIGIN, ctx.origin.clone());
        assert!(!ctx.accepts(&req));
    }

    #[test]
    fn allowed_preflight_returns_explicit_read_only_permissions() {
        let ctx = context(false);
        let mut req = request(Method::OPTIONS, "/app.js");
        req.headers_mut().insert(
            ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("GET"),
        );
        req.headers_mut().insert(
            ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("Range, If-None-Match"),
        );
        let response = ctx.preflight(&req);
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()[ACCESS_CONTROL_ALLOW_METHODS],
            "GET, HEAD"
        );
    }

    #[test]
    fn header_copy_keeps_duplicates_and_drops_connection_fields() {
        let mut source = HeaderMap::new();
        source.insert(
            CONNECTION,
            HeaderValue::from_static("keep-alive, x-private-hop"),
        );
        source.insert("x-private-hop", HeaderValue::from_static("remove"));
        source.insert("transfer-encoding", HeaderValue::from_static("chunked"));
        source.append("set-cookie", HeaderValue::from_static("a=1"));
        source.append("set-cookie", HeaderValue::from_static("b=2"));
        let copied = copy_end_to_end_headers(&source);
        assert!(!copied.contains_key(CONNECTION));
        assert!(!copied.contains_key("x-private-hop"));
        assert!(!copied.contains_key("transfer-encoding"));
        assert_eq!(copied.get_all("set-cookie").iter().count(), 2);
    }

    #[derive(Debug)]
    struct TestResource {
        fail: bool,
        delay: Duration,
    }

    #[async_trait]
    impl ResourceManager for TestResource {
        fn frontend_dist(&self) -> FrontendDist {
            directory_source()
        }
        async fn exists(&self, _path: &str) -> bool {
            false
        }
        async fn load(&self, _path: &str) -> Result<Vec<u8>> {
            bail!("unused test method")
        }
        async fn extract(&self, _from: &str, _to: &Path) -> Result<()> {
            bail!("unused test method")
        }
        async fn load_icon(&self, _path: &str) -> Result<Icon<'static>> {
            bail!("unused test method")
        }
        async fn respond(&self, _request: Request<Vec<u8>>) -> Result<StaticFileResponse> {
            tokio::time::sleep(self.delay).await;
            if self.fail {
                bail!("sensitive backend details must not reach the response")
            }
            Ok(Response::new(Cow::Borrowed(b"test".as_slice())))
        }
    }

    fn test_handler(
        fail: bool,
        delay: Duration,
        request_timeout: Duration,
    ) -> WindowProtocolHandler {
        WindowProtocolHandler {
            resources: Arc::new(TestResource { fail, delay }),
            context: context(false),
            request_timeout,
            upstream_error_status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    #[tokio::test]
    async fn backend_failure_produces_a_response_without_internal_details() {
        let handler = test_handler(true, Duration::ZERO, Duration::from_secs(1));
        let response = handler.respond(request(Method::GET, "/file.txt")).await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(response.body().is_empty());
        assert!(response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN));
    }

    #[tokio::test]
    async fn timeout_produces_a_gateway_timeout_response() {
        let handler = test_handler(false, Duration::from_secs(1), Duration::from_millis(1));
        let response = handler.respond(request(Method::GET, "/file.txt")).await;
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    }

    #[tokio::test]
    async fn handler_does_not_serve_other_origins() {
        let handler = test_handler(false, Duration::ZERO, Duration::from_secs(1));
        let mut req = request(Method::GET, "/file.txt");
        req.headers_mut().insert(
            ORIGIN,
            HeaderValue::from_static("https://elsewhere.example"),
        );
        let response = handler.respond(req).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN));
    }

    #[tokio::test]
    async fn handler_strips_a_body_even_if_a_backend_gets_head_wrong() {
        let handler = test_handler(false, Duration::ZERO, Duration::from_secs(1));
        let response = handler.respond(request(Method::HEAD, "/file.txt")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.body().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn one_system_resolves_multiple_windows_from_one_source() {
        let root = tempfile::tempdir().unwrap();
        let system = ProtocolSystem::new(
            FrontendDist::Directory(root.path().to_path_buf()),
            Handle::current(),
            ProtocolOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            system.resolve(&WebviewUrl::default()).unwrap(),
            app(APP_BASE)
        );
        assert_eq!(
            system
                .resolve(&WebviewUrl::App("settings.html".into()))
                .unwrap(),
            app("taurino://localhost/settings.html"),
        );
    }

    #[tokio::test]
    async fn current_thread_runtime_is_rejected_before_binding_a_webview() {
        let resources: Arc<dyn ResourceManager> = Arc::new(TestResource {
            fail: false,
            delay: Duration::ZERO,
        });
        assert!(
            ProtocolSystem::from_resource_manager(
                resources,
                Handle::current(),
                ProtocolOptions::default(),
            )
            .is_err()
        );
    }
}
