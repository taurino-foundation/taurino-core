#[cfg(not(any(target_os = "android", target_os = "ios")))]
use http::Request;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tao::window::Window;
use url::Url;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::types::WebContextStore;
use crate::{
    types::{
        DocumentTitleChangedHandler, DownloadEvent, DownloadHandler,
        NavigationHandler, NewWindowHandler, NewWindowResponse,
        OnPageLoadHandler, PageLoadEvent, PermissionKind,
        PermissionRequestHandler, PermissionResponse, Rect,
        UriSchemeProtocolHandler, WebviewIpcHandler, WebviewUrl,
    },
    utils::{NewWindowFeatures, WindowWebViewMetaData},
    webview::{ManagedWebview, factory::create_webview},
};

pub struct WebViewBuilder {
    pub auto_resize: bool,
    pub kind: bool,
    pub label: String,
    /// Custom protocols to register on the webview
    pub uri_scheme_protocols: HashMap<String, Box<UriSchemeProtocolHandler>>,

    /// How to handle IPC calls on the webview.
    pub ipc_handler: Option<WebviewIpcHandler>,

    /// A handler to decide if incoming url is allowed to navigate.
    pub navigation_handler: Option<Box<NavigationHandler>>,

    pub new_window_handler: Option<Box<NewWindowHandler>>,

    pub document_title_changed_handler:
        Option<Box<DocumentTitleChangedHandler>>,

    /// The resolved URL to load on the webview.
    pub url: Option<WebviewUrl>,

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

    pub on_page_load_handler: Option<Box<OnPageLoadHandler>>,

    pub download_handler: Option<Arc<DownloadHandler>>,

    pub permission_request_handler: Option<Box<PermissionRequestHandler>>,

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub on_web_content_process_terminate_handler:
        Option<Box<OnWebContentProcessTerminateHandler>>,
    pub data_directory: Option<PathBuf>,
    pub bounds: Option<Rect>,
}

impl Default for WebViewBuilder {
    fn default() -> Self {
        Self {
            kind: false,
            auto_resize: false,
            label: "root".to_string(),
            uri_scheme_protocols: HashMap::new(),
            ipc_handler: None,
            navigation_handler: None,
            new_window_handler: None,
            document_title_changed_handler: None,
            url: Some(WebviewUrl::External(
                Url::parse("https://tauri.app").unwrap(),
            )),
            #[cfg(target_os = "android")]
            on_webview_created: None,
            on_page_load_handler: None,
            download_handler: None,
            permission_request_handler: None,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            on_web_content_process_terminate_handler: None,
            data_directory: None,
            bounds: None,
        }
    }
}

impl WebViewBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(mut self, id: &str) -> Self {
        self.label = id.to_owned();
        self
    }

    // Variant B: explicit
    pub fn with_url(mut self, url: WebviewUrl) -> Self {
        self.url = Some(url);
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

    /// Sets the navigation handler.
    pub fn with_navigation_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, &Url) -> bool + Send + 'static,
    {
        self.navigation_handler = Some(Box::new(handler));
        self
    }

    /// Sets the new-window request handler.
    pub fn with_new_window_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(
                &WindowWebViewMetaData,
                Url,
                NewWindowFeatures,
            ) -> NewWindowResponse
            + 'static,
    {
        self.new_window_handler = Some(Box::new(handler));
        self
    }

    /// Sets the document-title changed handler.
    pub fn with_document_title_changed_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, String) + Send + 'static,
    {
        self.document_title_changed_handler = Some(Box::new(handler));
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

    /// Sets the download handler.
    pub fn with_download_handler<F>(mut self, handler: F) -> Self
    where
        F: for<'a> Fn(&WindowWebViewMetaData, DownloadEvent<'a>) -> bool
            + Send
            + Sync
            + 'static,
    {
        self.download_handler = Some(Arc::new(handler));
        self
    }

    /// Sets the permission-request handler.
    pub fn with_permission_request_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, PermissionKind) -> PermissionResponse
            + Send
            + Sync
            + 'static,
    {
        self.permission_request_handler = Some(Box::new(handler));
        self
    }

    pub(crate) fn build<F>(
        self,
        window: &Window,
        metadata: &WindowWebViewMetaData,
        web_context: WebContextStore,
        befor_webview_creation: Option<F>,
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
            befor_webview_creation,
        )
    }
}

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
