use http::Request;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tao::{dpi, window::Window};
use url::Url;

use crate::{
    types::{
        BackgroundThrottlingPolicy, Color, DocumentTitleChangedHandler, DownloadEvent,
        DownloadHandler, InitializationScript, NavigationHandler, NewWindowHandler,
        NewWindowResponse, OnPageLoadHandler, PageLoadEvent, PermissionKind,
        PermissionRequestHandler, PermissionResponse, Rect, ScrollBarStyle,
        UriSchemeProtocolHandler, WebContextStore, WebviewIpcHandler, WebviewUrl,
        WindowEffectsConfig,
    },
    utils::{NewWindowFeatures, WindowWebViewMetaData},
    webview::{factory::create_webview, ManagedWebview},
};

#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::types::OnWebContentProcessTerminateHandler;

#[cfg(target_os = "ios")]
use crate::types::InputAccessoryViewBuilderFn;

#[cfg(target_os = "android")]
use crate::types::CreationContext;

pub struct WebViewBuilder {
    pub accept_first_mouse: bool,
    pub additional_browser_args: Option<String>,
    pub allow_link_preview: bool,
    pub auto_resize: bool,
    pub background_color: Option<Color>,
    pub background_throttling: Option<BackgroundThrottlingPolicy>,
    pub bounds: Option<Rect>,
    pub browser_extensions_enabled: bool,
    pub clipboard: bool,
    pub data_directory: Option<PathBuf>,
    pub data_store_identifier: Option<[u8; 16]>,
    pub devtools: Option<bool>,
    pub document_title_changed_handler: Option<Box<DocumentTitleChangedHandler>>,
    pub download_handler: Option<Arc<DownloadHandler>>,
    pub drag_drop_handler_enabled: bool,

    #[cfg(windows)]
    pub environment:
        Option<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment>,

    pub extensions_path: Option<PathBuf>,
    pub focus: bool,
    pub general_autofill_enabled: bool,
    pub incognito: bool,
    pub initialization_scripts: Vec<InitializationScript>,

    #[cfg(target_os = "ios")]
    pub input_accessory_view_builder: Option<Box<InputAccessoryViewBuilderFn>>,

    pub ipc_handler: Option<WebviewIpcHandler>,
    pub javascript_disabled: bool,
    pub kind: bool,
    pub label: String,

    #[cfg(target_os = "ios")]
    pub limit_navigations_to_app_bound_domains: bool,

    pub navigation_handler: Option<Box<NavigationHandler>>,
    pub new_window_handler: Option<Box<NewWindowHandler>>,

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub on_web_content_process_terminate_handler: Option<Box<OnWebContentProcessTerminateHandler>>,

    pub on_page_load_handler: Option<Box<OnPageLoadHandler>>,

    #[cfg(target_os = "android")]
    #[allow(clippy::type_complexity)]
    pub on_webview_created: Option<
        Box<
            dyn Fn(
                    &WindowWebViewMetaData,
                    CreationContext<'_, '_>,
                ) -> Result<(), jni::errors::Error>
                + Send
                + Sync,
        >,
    >,

    pub permission_request_handler: Option<Box<PermissionRequestHandler>>,
    pub proxy_url: Option<Url>,

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub related_view: Option<webkit2gtk::WebView>,
    pub scroll_bar_style: ScrollBarStyle,
    pub traffic_light_position: Option<dpi::Position>,
    pub transparent: bool,
    pub uri_scheme_protocols: HashMap<String, Box<UriSchemeProtocolHandler>>,
    pub url: Option<WebviewUrl>,
    pub use_https_scheme: bool,
    pub user_agent: Option<String>,

    #[cfg(target_os = "macos")]
    pub webview_configuration: Option<objc2::rc::Retained<objc2_web_kit::WKWebViewConfiguration>>,

    pub window_effects: Option<WindowEffectsConfig>,
    pub zoom_hotkeys_enabled: bool,
}

impl Default for WebViewBuilder {
    fn default() -> Self {
        Self {
            accept_first_mouse: false,
            additional_browser_args: None,
            allow_link_preview: true,
            auto_resize: false,
            background_color: None,
            background_throttling: None,
            bounds: None,
            browser_extensions_enabled: false,
            clipboard: false,
            data_directory: None,
            data_store_identifier: None,
            devtools: None,
            document_title_changed_handler: None,
            download_handler: None,
            drag_drop_handler_enabled: true,

            #[cfg(windows)]
            environment: None,

            extensions_path: None,
            focus: true,
            general_autofill_enabled: true,
            incognito: false,
            initialization_scripts: Vec::new(),

            #[cfg(target_os = "ios")]
            input_accessory_view_builder: None,

            ipc_handler: None,
            javascript_disabled: false,
            kind: false,
            label: "root".to_string(),

            #[cfg(target_os = "ios")]
            limit_navigations_to_app_bound_domains: false,

            navigation_handler: None,
            new_window_handler: None,

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            on_web_content_process_terminate_handler: None,

            on_page_load_handler: None,

            #[cfg(target_os = "android")]
            on_webview_created: None,

            permission_request_handler: None,
            proxy_url: None,

            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "linux",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            related_view: None,

            scroll_bar_style: ScrollBarStyle::Default,
            traffic_light_position: None,
            transparent: false,
            uri_scheme_protocols: HashMap::new(),
            url: Some(WebviewUrl::External(
                Url::parse("https://tauri.app").expect("valid default URL"),
            )),
            use_https_scheme: false,
            user_agent: None,

            #[cfg(target_os = "macos")]
            webview_configuration: None,

            window_effects: None,
            zoom_hotkeys_enabled: false,
        }
    }
}

impl WebViewBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets first-mouse acceptance.
    pub fn with_accept_first_mouse(mut self, value: bool) -> Self {
        self.accept_first_mouse = value;
        self
    }

    /// Sets extra browser arguments.
    pub fn with_additional_browser_args(mut self, value: Option<String>) -> Self {
        self.additional_browser_args = value;
        self
    }

    /// Sets link-preview support.
    pub fn with_allow_link_preview(mut self, value: bool) -> Self {
        self.allow_link_preview = value;
        self
    }

    /// Sets automatic resizing.
    pub fn with_auto_resize(mut self, value: bool) -> Self {
        self.auto_resize = value;
        self
    }

    /// Sets the background color.
    pub fn with_background_color(mut self, value: Option<Color>) -> Self {
        self.background_color = value;
        self
    }

    /// Sets background throttling.
    pub fn with_background_throttling(mut self, value: Option<BackgroundThrottlingPolicy>) -> Self {
        self.background_throttling = value;
        self
    }

    /// Sets the bounds.
    pub fn with_bounds(mut self, value: Option<Rect>) -> Self {
        self.bounds = value;
        self
    }

    /// Sets browser-extension support.
    pub fn with_browser_extensions_enabled(mut self, value: bool) -> Self {
        self.browser_extensions_enabled = value;
        self
    }

    /// Sets clipboard access.
    pub fn with_clipboard(mut self, value: bool) -> Self {
        self.clipboard = value;
        self
    }

    /// Sets the data directory.
    pub fn with_data_directory(mut self, value: Option<PathBuf>) -> Self {
        self.data_directory = value;
        self
    }

    /// Sets the data-store identifier.
    pub fn with_data_store_identifier(mut self, value: Option<[u8; 16]>) -> Self {
        self.data_store_identifier = value;
        self
    }

    /// Sets DevTools availability.
    pub fn with_devtools(mut self, value: Option<bool>) -> Self {
        self.devtools = value;
        self
    }

    /// Sets the title-change handler.
    pub fn with_document_title_changed_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, String) + Send + 'static,
    {
        self.document_title_changed_handler = Some(Box::new(handler));
        self
    }

    /// Sets the download handler.
    pub fn with_download_handler<F>(mut self, handler: F) -> Self
    where
        F: for<'a> Fn(&WindowWebViewMetaData, DownloadEvent<'a>) -> bool + Send + Sync + 'static,
    {
        self.download_handler = Some(Arc::new(handler));
        self
    }

    /// Sets drag-and-drop handling.
    pub fn with_drag_drop_handler_enabled(mut self, value: bool) -> Self {
        self.drag_drop_handler_enabled = value;
        self
    }

    #[cfg(windows)]
    /// Sets the WebView2 environment.
    pub fn with_environment(
        mut self,
        value: Option<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment>,
    ) -> Self {
        self.environment = value;
        self
    }

    /// Sets the extensions path.
    pub fn with_extensions_path(mut self, value: Option<PathBuf>) -> Self {
        self.extensions_path = value;
        self
    }

    /// Sets initial focus.
    pub fn with_focus(mut self, value: bool) -> Self {
        self.focus = value;
        self
    }

    /// Sets general autofill.
    pub fn with_general_autofill_enabled(mut self, value: bool) -> Self {
        self.general_autofill_enabled = value;
        self
    }

    /// Sets incognito mode.
    pub fn with_incognito(mut self, value: bool) -> Self {
        self.incognito = value;
        self
    }

    /// Sets initialization scripts.
    pub fn with_initialization_scripts(mut self, value: Vec<InitializationScript>) -> Self {
        self.initialization_scripts = value;
        self
    }

    #[cfg(target_os = "ios")]
    /// Sets the input accessory builder.
    pub fn with_input_accessory_view_builder(
        mut self,
        value: Option<Box<InputAccessoryViewBuilderFn>>,
    ) -> Self {
        self.input_accessory_view_builder = value;
        self
    }

    /// Sets the IPC handler.
    pub fn with_ipc_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, Request<String>) + Send + 'static,
    {
        self.ipc_handler = Some(Box::new(handler));
        self
    }

    /// Sets JavaScript blocking.
    pub fn with_javascript_disabled(mut self, value: bool) -> Self {
        self.javascript_disabled = value;
        self
    }

    /// Sets the builder kind flag.
    pub fn with_kind(mut self, value: bool) -> Self {
        self.kind = value;
        self
    }

    /// Sets the label.
    pub fn with_label(mut self, value: impl Into<String>) -> Self {
        self.label = value.into();
        self
    }

    #[cfg(target_os = "ios")]
    /// Limits navigation to app-bound domains.
    pub fn with_limit_navigations_to_app_bound_domains(mut self, value: bool) -> Self {
        self.limit_navigations_to_app_bound_domains = value;
        self
    }

    /// Sets the navigation handler.
    pub fn with_navigation_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, &Url) -> bool + Send + 'static,
    {
        self.navigation_handler = Some(Box::new(handler));
        self
    }

    /// Sets the new-window handler.
    pub fn with_new_window_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, Url, NewWindowFeatures) -> NewWindowResponse + 'static,
    {
        self.new_window_handler = Some(Box::new(handler));
        self
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    /// Sets the web-content termination handler.
    pub fn with_on_web_content_process_terminate_handler(
        mut self,
        value: Option<Box<OnWebContentProcessTerminateHandler>>,
    ) -> Self {
        self.on_web_content_process_terminate_handler = value;
        self
    }

    /// Sets the page-load handler.
    pub fn with_on_page_load_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, Url, PageLoadEvent) + Send + 'static,
    {
        self.on_page_load_handler = Some(Box::new(handler));
        self
    }

    #[cfg(target_os = "android")]
    /// Sets the Android creation handler.
    pub fn with_on_webview_created(
        mut self,
        value: Option<
            Box<
                dyn Fn(
                        &WindowWebViewMetaData,
                        CreationContext<'_, '_>,
                    ) -> Result<(), jni::errors::Error>
                    + Send
                    + Sync,
            >,
        >,
    ) -> Self {
        self.on_webview_created = value;
        self
    }

    /// Sets the permission handler.
    pub fn with_permission_request_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, PermissionKind) -> PermissionResponse + Send + Sync + 'static,
    {
        self.permission_request_handler = Some(Box::new(handler));
        self
    }

    /// Sets the proxy URL.
    pub fn with_proxy_url(mut self, value: Option<Url>) -> Self {
        self.proxy_url = value;
        self
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    /// Sets the related WebKit view.
    pub fn with_related_view(mut self, value: Option<webkit2gtk::WebView>) -> Self {
        self.related_view = value;
        self
    }

    /// Sets the scrollbar style.
    pub fn with_scroll_bar_style(mut self, value: ScrollBarStyle) -> Self {
        self.scroll_bar_style = value;
        self
    }

    /// Sets the traffic-light position.
    pub fn with_traffic_light_position(mut self, value: Option<dpi::Position>) -> Self {
        self.traffic_light_position = value;
        self
    }

    /// Sets transparency.
    pub fn with_transparent(mut self, value: bool) -> Self {
        self.transparent = value;
        self
    }

    /// Sets custom URI protocols.
    pub fn with_uri_scheme_protocols(
        mut self,
        value: HashMap<String, Box<UriSchemeProtocolHandler>>,
    ) -> Self {
        self.uri_scheme_protocols = value;
        self
    }

    /// Sets the URL.
    pub fn with_url(mut self, value: Option<WebviewUrl>) -> Self {
        self.url = value;
        self
    }

    /// Sets HTTPS scheme usage.
    pub fn with_use_https_scheme(mut self, value: bool) -> Self {
        self.use_https_scheme = value;
        self
    }

    /// Sets the user agent.
    pub fn with_user_agent(mut self, value: Option<String>) -> Self {
        self.user_agent = value;
        self
    }

    #[cfg(target_os = "macos")]
    /// Sets the WebKit configuration.
    pub fn with_webview_configuration(
        mut self,
        value: Option<objc2::rc::Retained<objc2_web_kit::WKWebViewConfiguration>>,
    ) -> Self {
        self.webview_configuration = value;
        self
    }

    /// Sets window effects.
    pub fn with_window_effects(mut self, value: Option<WindowEffectsConfig>) -> Self {
        self.window_effects = value;
        self
    }

    /// Sets zoom hotkeys.
    pub fn with_zoom_hotkeys_enabled(mut self, value: bool) -> Self {
        self.zoom_hotkeys_enabled = value;
        self
    }

    pub(crate) fn build<F>(
        self,
        window: &Window,
        metadata: &WindowWebViewMetaData,
        web_context: WebContextStore,
        before_webview_creation: Option<F>,
    ) -> crate::error::Result<ManagedWebview>
    where
        F: for<'a> Fn(
                wry::WebViewBuilder<'a>,
                WebviewUrl,
            ) -> crate::error::Result<wry::WebViewBuilder<'a>>
            + Send
            + 'static,
    {
        create_webview(
            self,
            window,
            metadata.clone(),
            web_context,
            before_webview_creation,
        )
    }
}

// No URL.
// let builder = WebViewBuilder::new().with_url(None);

// Explicit URL.
// let builder = WebViewBuilder::new().with_url(Some(WebviewUrl::External(
//     Url::parse("https://example.com").unwrap(),
// )));

// Example: build without a URL.
// let mut builder = WebViewBuilder::new();
// builder.url = None;

// Example: build with an explicit URL.
// let builder = WebViewBuilder::new().with_url(WebviewUrl::External(
//     Url::parse("https://example.com").unwrap(),
// ));

// -----------------------------------------------------------------------------
// Example setup
// -----------------------------------------------------------------------------
//
// Each managed window owns multiple ManagedWebview values. The metadata is
// created by the window layer and passed into create_webview, so create_webview
// never invents labels or window identity itself.
//
// let metadata = WindowWebViewMetaData::new(
//     managed_window.label().clone(),
//     "root",
//     managed_window.window().id(),
// );
//
// let window_manager_for_popups = Rc::clone(&window_manager);
//
// let builder = WebViewBuilder::new()
//     .with_url("https://example.com")
//     .with_new_window_handler(move |_opener, _url, _features| {
//         let Some(window) = window_manager_for_popups.get_window("main") else {
//             return NewWindowResponse::Deny;
//         };
//
//         let Some(webview) = window
//             .webviews()
//             .iter()
//             .find(|webview| webview.metadata().view_label.as_ref() == "popup")
//             .cloned()
//         else {
//             return NewWindowResponse::Deny;
//         };
//
//         NewWindowResponse::Create { webview }
//     });
//
// let webview = create_webview(builder, managed_window.window(), metadata)?;
// managed_window.webviews_mut().push(webview);

/*
WebviewUrl Is None!!!;



let mut builder = WebViewBuilder::new();
builder.url = None;

let webview = builder.build(
    window,
    "blank",
    web_context,
    None::<fn(
        wry::WebViewBuilder<'_>,
        WebviewUrl,
    ) -> crate::Result<wry::WebViewBuilder<'_>>>,
)?;

-----------------------------------------------------------------
WebviewUrl Is Not None!!!;

let webview = WebViewBuilder::new()
    .with_url(WebviewUrl::External(
        Url::parse("https://example.com").unwrap(),
    ))





    .build(
        window,
        "main",
        web_context,
        Some(|builder, url| {
            let builder = match url {
                WebviewUrl::External(url) | WebviewUrl::CustomProtocol(url) => {
                    builder.with_url(url.to_string())
                }

                WebviewUrl::App(path) => {
                    builder.with_url(path.to_string_lossy().to_string())
                }
            };

            Ok(builder)
        }),
    )?;
*/
