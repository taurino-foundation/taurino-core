#[cfg(not(any(target_os = "android", target_os = "ios")))]
use dpi::LogicalPosition;
use http::Request;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::sync::Mutex;
use std::{collections::HashMap, path::PathBuf, rc::Rc, sync::Arc};
use tao::window::Window;
use url::Url;
#[cfg(windows)]
use wry::WebViewExtWindows;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::{types::WebContextStore, utils::WebContext, wrappers::RectWrapper};
use crate::{
    types::{
        DocumentTitleChangedHandler, DownloadEvent, DownloadHandler,
        NavigationHandler, NewWindowHandler, NewWindowResponse,
        OnPageLoadHandler, PageLoadEvent, PermissionKind,
        PermissionRequestHandler, PermissionResponse, Rect,
        UriSchemeProtocolHandler, WebviewBounds, WebviewIpcHandler, WebviewUrl,
    },
    utils::{NewWindowFeatures, WindowWebViewMetaData},
    webview::{ManagedWebview, WebViewId},
    window::WindowId,
};

use crate::utils::{from_wry_permission_kind, to_wry_permission_response};

use std::collections::{
    HashSet,
    hash_map::Entry::{Occupied, Vacant},
};

use wry::WebContext as WryWebContext;

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
    ) -> crate::Result<ManagedWebview>
    where
        F: for<'a> Fn(
                wry::WebViewBuilder<'a>,
                WebviewUrl,
            ) -> crate::Result<wry::WebViewBuilder<'a>>
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

pub fn create_webview<F>(
    builder: WebViewBuilder,
    window: &Window,
    metadata: WindowWebViewMetaData,
    web_context_store: WebContextStore,
    befor_webview_creation: Option<F>,
) -> crate::Result<ManagedWebview>
where
    F: for<'a> Fn(
            wry::WebViewBuilder<'a>,
            WebviewUrl,
        ) -> crate::Result<wry::WebViewBuilder<'a>>
        + Send
        + 'static,
{
    debug_assert_eq!(
        metadata.native_id,
        window.id(),
        "Managed webview metadata must belong to the window used to create the webview",
    );

    let WebViewBuilder {
        uri_scheme_protocols,
        ipc_handler,
        navigation_handler,
        new_window_handler,
        document_title_changed_handler,
        url,
        on_page_load_handler,
        download_handler,
        permission_request_handler,
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        on_web_content_process_terminate_handler,
        #[cfg(target_os = "android")]
        on_webview_created,
        data_directory,
        label,
        bounds,
        auto_resize,
        kind,
        ..
    } = builder;

    let mut web_context =
        web_context_store.lock().expect("poisoned WebContext store");
    let is_first_context = web_context.is_empty();
    // the context must be stored on the HashMap because it must outlive the WebView on macOS
    let automation_enabled =
        std::env::var("TAURI_WEBVIEW_AUTOMATION").as_deref() == Ok("true");
    let web_context_key = data_directory;
    let entry = web_context.entry(web_context_key.clone());
    let web_context = match entry {
        Occupied(occupied) => {
            let occupied = occupied.into_mut();
            occupied.referenced_by_webviews.insert(label.clone());
            occupied
        }
        Vacant(vacant) => {
            let mut web_context = WryWebContext::new(web_context_key.clone());
            web_context.set_allows_automation(if automation_enabled {
                is_first_context
            } else {
                false
            });
            vacant.insert(WebContext {
                inner: web_context,
                referenced_by_webviews: [label.clone()].into(),
                registered_custom_protocols: HashSet::new(),
            })
        }
    };

    let mut webview_builder =
        wry::WebViewBuilder::new_with_web_context(&mut web_context.inner);

    let webview_bounds = if let Some(bounds) = bounds {
        let bounds: RectWrapper = bounds.into();
        let bounds = bounds.0;

        let scale_factor = window.scale_factor();
        let position = bounds.position.to_logical::<f32>(scale_factor);
        let size = bounds.size.to_logical::<f32>(scale_factor);

        webview_builder = webview_builder.with_bounds(bounds);

        let window_size = window.inner_size().to_logical::<f32>(scale_factor);

        if auto_resize {
            Some(WebviewBounds {
                x_rate: position.x / window_size.width,
                y_rate: position.y / window_size.height,
                width_rate: size.width / window_size.width,
                height_rate: size.height / window_size.height,
            })
        } else {
            None
        }
    } else {
        if kind {
            webview_builder = webview_builder.with_bounds(wry::Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: window.inner_size().into(),
            });
            Some(WebviewBounds {
                x_rate: 0.,
                y_rate: 0.,
                width_rate: 1.,
                height_rate: 1.,
            })
        } else {
            None
        }
    };

    // Apply the initial URL before the backend webview is built.
    // Empty means: keep Wry's default blank document.
    if let Some(url) = url {
        webview_builder = match befor_webview_creation {
            Some(callback) => callback(webview_builder, url)?,
            None => match url {
                WebviewUrl::External(u) | WebviewUrl::CustomProtocol(u) => {
                    webview_builder.with_url(u.to_string())
                }
                WebviewUrl::App(path) => {
                    webview_builder.with_url(path.to_string_lossy().to_string())
                }
            },
        };
    }

    if let Some(navigation_handler) = navigation_handler {
        let metadata = metadata.clone();
        webview_builder = webview_builder.with_navigation_handler(move |url| {
            url.parse()
                .map(|url| navigation_handler(&metadata, &url))
                .unwrap_or(true)
        });
    }

    if let Some(new_window_handler) = new_window_handler {
        let metadata = metadata.clone();

        webview_builder = webview_builder.with_new_window_req_handler(
            move |url, features| {
                use crate::utils::NewWindowOpener;

                let Ok(url) = url.parse() else {
                    return wry::NewWindowResponse::Deny;
                };

                match new_window_handler(
                    &metadata,
                    url,
                    NewWindowFeatures::new(
                        features.size,
                        features.position,
                        NewWindowOpener {
                            webview: features.opener.webview,
                            #[cfg(windows)]
                            environment: features.opener.environment,
                            #[cfg(target_os = "macos")]
                            target_configuration: features
                                .opener
                                .target_configuration,
                        },
                    ),
                ) {
                    NewWindowResponse::Allow => wry::NewWindowResponse::Allow,

                    NewWindowResponse::Create { webview } => {
                        wry::NewWindowResponse::Create {
                            #[cfg(target_os = "macos")]
                            webview: wry::WebViewExtMacOS::webview(webview)
                                .as_super()
                                .into(),

                            #[cfg(any(
                                target_os = "linux",
                                target_os = "dragonfly",
                                target_os = "freebsd",
                                target_os = "netbsd",
                                target_os = "openbsd",
                            ))]
                            webview: webview.webview(),

                            #[cfg(windows)]
                            webview: webview.webview(),
                        }
                    }

                    NewWindowResponse::Deny => wry::NewWindowResponse::Deny,
                }
            },
        );
    }

    if let Some(document_title_changed_handler) = document_title_changed_handler
    {
        let metadata = metadata.clone();
        webview_builder =
            webview_builder.with_document_title_changed_handler(move |title| {
                document_title_changed_handler(&metadata, title);
            });
    }

    if let Some(permission_request_handler) = permission_request_handler {
        let metadata = metadata.clone();
        webview_builder =
            webview_builder.with_permission_handler(move |kind| {
                let kind = from_wry_permission_kind(kind);
                to_wry_permission_response(permission_request_handler(
                    &metadata, kind,
                ))
            });
    }

    if let Some(ipc_handler) = ipc_handler {
        let metadata = metadata.clone();
        webview_builder = webview_builder.with_ipc_handler(move |request| {
            ipc_handler(&metadata, request);
        });
    }

    if let Some(download_handler) = download_handler {
        let started_metadata = metadata.clone();
        let started_handler = Arc::clone(&download_handler);
        webview_builder = webview_builder.with_download_started_handler(
            move |url, destination| {
                let Ok(url) = url.parse() else {
                    return false;
                };

                started_handler(
                    &started_metadata,
                    DownloadEvent::Requested { url, destination },
                )
            },
        );

        let completed_metadata = metadata.clone();
        webview_builder = webview_builder.with_download_completed_handler(
            move |url, path, success| {
                if let Ok(url) = url.parse() {
                    let _ = download_handler(
                        &completed_metadata,
                        DownloadEvent::Finished { url, path, success },
                    );
                }
            },
        );
    }

    if let Some(page_load_handler) = on_page_load_handler {
        let metadata = metadata.clone();
        webview_builder =
            webview_builder.with_on_page_load_handler(move |event, url| {
                if let Ok(url) = url.parse() {
                    page_load_handler(
                        &metadata,
                        url,
                        match event {
                            wry::PageLoadEvent::Started => {
                                PageLoadEvent::Started
                            }
                            wry::PageLoadEvent::Finished => {
                                PageLoadEvent::Finished
                            }
                        },
                    );
                }
            });
    }

    for (scheme, protocol) in uri_scheme_protocols {
        let metadata = metadata.clone();
        webview_builder = webview_builder.with_asynchronous_custom_protocol(
            scheme,
            move |webview_id, request, responder| {
                protocol(
                    &metadata,
                    webview_id,
                    request,
                    Box::new(move |response| responder.respond(response)),
                );
            },
        );
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    if let Some(on_web_content_process_terminate_handler) =
        on_web_content_process_terminate_handler
    {
        let metadata = metadata.clone();
        webview_builder = webview_builder
            .with_on_web_content_process_terminated_handler(move || {
                on_web_content_process_terminate_handler(&metadata);
            });
    }

    let webview = Rc::new(
        webview_builder
            .build(window)
            .map_err(|e| crate::Error::CreateWebview(Box::new(e)))?,
    );
    Ok(ManagedWebview {
        metadata,
        inner: webview,
        context_store: web_context_store.clone(),
        context_key: if automation_enabled {
            None
        } else {
            web_context_key
        },
        bounds: Arc::new(Mutex::new(webview_bounds)),
    })
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
