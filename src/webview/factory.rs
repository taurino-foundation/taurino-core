#[cfg(not(any(target_os = "android", target_os = "ios")))]
use dpi::LogicalPosition;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::sync::Mutex;
use std::{rc::Rc, sync::Arc};
use tao::window::Window;

#[cfg(target_os = "macos")]
use objc2::ClassType;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::{types::WebContextStore, utils::WebContext};
use crate::{
    types::{DownloadEvent, NewWindowResponse, PageLoadEvent, WebviewBounds, WebviewUrl},
    utils::{wrappers::RectWrapper, NewWindowFeatures, WindowWebViewMetaData},
    webview::{builder::WebViewBuilder, ManagedWebview},
};

use crate::utils::{from_wry_permission_kind, parse_proxy_url, to_wry_permission_response};

#[cfg(any(target_os = "macos", target_os = "ios"))]
use wry::WebViewBuilderExtDarwin;

#[cfg(any(target_os = "macos", target_os = "ios"))]
use wry::WebViewBuilderExtDarwin;

#[cfg(target_os = "macos")]
use wry::{WebViewBuilderExtMacos, WebViewExtMacOS};

#[cfg(target_os = "ios")]
use wry::WebViewBuilderExtIos;

#[cfg(target_os = "android")]
use wry::WebViewBuilderExtAndroid;

#[cfg(windows)]
use wry::{WebViewBuilderExtWindows, WebViewExtWindows};

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
use wry::WebViewBuilderExtUnix;

use std::collections::{
    hash_map::Entry::{Occupied, Vacant},
    HashSet,
};

use wry::WebContext as WryWebContext;

pub fn create_webview<F>(
    builder: WebViewBuilder,
    window: &Window,
    metadata: WindowWebViewMetaData,
    web_context_store: WebContextStore,
    befor_webview_creation: Option<F>,
) -> crate::error::Result<ManagedWebview>
where
    F: for<'a> Fn(wry::WebViewBuilder<'a>, WebviewUrl) -> crate::error::Result<wry::WebViewBuilder<'a>>
        + Send
        + 'static,
{
    debug_assert_eq!(
        metadata.native_id,
        window.id(),
        "Managed webview metadata must belong to the window used to create the webview",
    );

    let WebViewBuilder {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        on_web_content_process_terminate_handler,

        #[cfg(target_os = "android")]
        on_webview_created,

        #[cfg(target_os = "ios")]
        input_accessory_view_builder,

        #[cfg(target_os = "ios")]
        limit_navigations_to_app_bound_domains,

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
        ))]
        related_view,

        #[cfg(target_os = "macos")]
        webview_configuration,

        #[cfg(windows)]
        environment,

        accept_first_mouse,
        additional_browser_args,
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        allow_link_preview,
        auto_resize,
        background_color,
        background_throttling,
        bounds,
        browser_extensions_enabled,
        clipboard,
        data_directory,
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        data_store_identifier,
        devtools,
        document_title_changed_handler,
        download_handler,
        drag_drop_handler_enabled: _,
        extensions_path,
        focus,
        general_autofill_enabled,
        incognito,
        initialization_scripts,
        ipc_handler,
        javascript_disabled,
        kind,
        label,
        navigation_handler,
        new_window_handler,
        on_page_load_handler,
        permission_request_handler,
        proxy_url,
        scroll_bar_style,
        #[cfg(target_os = "macos")]
        traffic_light_position,
        transparent,
        uri_scheme_protocols,
        url,
        use_https_scheme,
        user_agent,
        window_effects: _,
        zoom_hotkeys_enabled,
        ..
    } = builder;

    let mut web_context = web_context_store.lock().expect("poisoned WebContext store");
    let is_first_context = web_context.is_empty();
    // the context must be stored on the HashMap because it must outlive the WebView on macOS
    let automation_enabled = std::env::var("TAURI_WEBVIEW_AUTOMATION").as_deref() == Ok("true");
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
            web_context.set_allows_automation(if automation_enabled { is_first_context } else { false });
            vacant.insert(WebContext {
                inner: web_context,
                referenced_by_webviews: [label.clone()].into(),
                registered_custom_protocols: HashSet::new(),
            })
        }
    };

    let mut webview_builder = wry::WebViewBuilder::new_with_web_context(&mut web_context.inner)
        .with_id(&label)
        .with_focused(focus)
        .with_transparent(transparent)
        .with_accept_first_mouse(accept_first_mouse)
        .with_incognito(incognito)
        .with_clipboard(clipboard)
        .with_hotkeys_zoom(zoom_hotkeys_enabled)
        .with_general_autofill_enabled(general_autofill_enabled);

    #[cfg(target_os = "macos")]
    if let Some(webview_configuration) = webview_configuration {
        webview_builder = webview_builder.with_webview_configuration(webview_configuration);
    }

    #[cfg(windows)]
    {
        webview_builder = webview_builder.with_https_scheme(use_https_scheme);
    }

    #[cfg(target_os = "android")]
    {
        webview_builder = webview_builder.with_https_scheme(use_https_scheme);
    }

    if let Some(background_throttling) = background_throttling {
        webview_builder = webview_builder.with_background_throttling(match background_throttling {
            crate::types::BackgroundThrottlingPolicy::Disabled => wry::BackgroundThrottlingPolicy::Disabled,

            crate::types::BackgroundThrottlingPolicy::Suspend => wry::BackgroundThrottlingPolicy::Suspend,

            crate::types::BackgroundThrottlingPolicy::Throttle => wry::BackgroundThrottlingPolicy::Throttle,
        });
    }

    if javascript_disabled {
        webview_builder = webview_builder.with_javascript_disabled();
    }

    if let Some(background_color) = background_color {
        webview_builder = webview_builder.with_background_color(background_color.into());
    }

    if let Some(user_agent) = user_agent {
        webview_builder = webview_builder.with_user_agent(user_agent);
    }

    #[cfg(windows)]
    {
        if let Some(additional_browser_args) = additional_browser_args {
            webview_builder = webview_builder.with_additional_browser_args(additional_browser_args);
        }

        if let Some(environment) = environment {
            webview_builder = webview_builder.with_environment(environment);
        }

        webview_builder = webview_builder.with_theme(match window.theme() {
            tao::window::Theme::Dark => wry::Theme::Dark,
            tao::window::Theme::Light => wry::Theme::Light,
            _ => wry::Theme::Light,
        });

        webview_builder = webview_builder.with_scroll_bar_style(match scroll_bar_style {
            crate::types::ScrollBarStyle::Default => wry::ScrollBarStyle::Default,

            crate::types::ScrollBarStyle::FluentOverlay => wry::ScrollBarStyle::FluentOverlay,
        });

        webview_builder = webview_builder.with_browser_extensions_enabled(browser_extensions_enabled);
    }

    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    if let Some(path) = extensions_path {
        webview_builder = webview_builder.with_extensions_path(path);
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    if let Some(related_view) = related_view {
        webview_builder = webview_builder.with_related_view(related_view);
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        if let Some(data_store_identifier) = data_store_identifier {
            webview_builder = webview_builder.with_data_store_identifier(data_store_identifier);
        }

        webview_builder = webview_builder.with_allow_link_preview(allow_link_preview);
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        if let Some(data_store_identifier) = data_store_identifier {
            webview_builder = webview_builder.with_data_store_identifier(data_store_identifier);
        }

        webview_builder = webview_builder.with_allow_link_preview(allow_link_preview);
    }

    #[cfg(target_os = "ios")]
    {
        webview_builder =
            webview_builder.with_limit_navigations_to_app_bound_domains(limit_navigations_to_app_bound_domains);

        if let Some(input_accessory_view_builder) = input_accessory_view_builder {
            webview_builder =
                webview_builder.with_input_accessory_view_builder(move |webview| input_accessory_view_builder(webview));
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        if let Some(data_store_identifier) = data_store_identifier {
            webview_builder = webview_builder.with_data_store_identifier(data_store_identifier);
        }

        webview_builder = webview_builder.with_allow_link_preview(allow_link_preview);
    }
    #[cfg(target_os = "macos")]
    if let Some(position) = traffic_light_position {
        webview_builder = webview_builder.with_traffic_light_inset(position);
    }

    #[cfg(target_os = "ios")]
    {
        webview_builder =
            webview_builder.with_limit_navigations_to_app_bound_domains(limit_navigations_to_app_bound_domains);

        if let Some(input_accessory_view_builder) = input_accessory_view_builder {
            webview_builder =
                webview_builder.with_input_accessory_view_builder(move |webview| input_accessory_view_builder(webview));
        }
    }

    for script in initialization_scripts {
        webview_builder =
            webview_builder.with_initialization_script_for_main_only(script.script, script.for_main_frame_only);
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        webview_builder = webview_builder.with_devtools(devtools.unwrap_or(true));
    }

    #[cfg(target_os = "android")]
    if let Some(on_webview_created) = on_webview_created {
        let metadata = metadata.clone();

        webview_builder = webview_builder.on_webview_created(move |ctx| {
            on_webview_created(
                &metadata,
                crate::utils::CreationContext {
                    env: ctx.env,
                    activity: ctx.activity,
                    webview: ctx.webview,
                },
            )
        });
    }
    if let Some(proxy_url) = proxy_url {
        let config = parse_proxy_url(&proxy_url)?;

        webview_builder = webview_builder.with_proxy_config(config);
    }
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
                WebviewUrl::External(u) | WebviewUrl::CustomProtocol(u) => webview_builder.with_url(u.to_string()),
                WebviewUrl::App(path) => webview_builder.with_url(path.to_string_lossy().to_string()),
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

        webview_builder = webview_builder.with_new_window_req_handler(move |url, features| {
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
                        target_configuration: features.opener.target_configuration,
                    },
                ),
            ) {
                NewWindowResponse::Allow => wry::NewWindowResponse::Allow,

                NewWindowResponse::Create { webview } => wry::NewWindowResponse::Create {
                    #[cfg(target_os = "macos")]
                    webview: wry::WebViewExtMacOS::webview(&*webview.inner).as_super().into(),

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
                },

                NewWindowResponse::Deny => wry::NewWindowResponse::Deny,
            }
        });
    }

    if let Some(document_title_changed_handler) = document_title_changed_handler {
        let metadata = metadata.clone();
        webview_builder = webview_builder.with_document_title_changed_handler(move |title| {
            document_title_changed_handler(&metadata, title);
        });
    }

    if let Some(permission_request_handler) = permission_request_handler {
        let metadata = metadata.clone();
        webview_builder = webview_builder.with_permission_handler(move |kind| {
            let kind = from_wry_permission_kind(kind);
            to_wry_permission_response(permission_request_handler(&metadata, kind))
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
        webview_builder = webview_builder.with_download_started_handler(move |url, destination| {
            let Ok(url) = url.parse() else {
                return false;
            };

            started_handler(&started_metadata, DownloadEvent::Requested { url, destination })
        });

        let completed_metadata = metadata.clone();
        webview_builder = webview_builder.with_download_completed_handler(move |url, path, success| {
            if let Ok(url) = url.parse() {
                let _ = download_handler(&completed_metadata, DownloadEvent::Finished { url, path, success });
            }
        });
    }

    if let Some(page_load_handler) = on_page_load_handler {
        let metadata = metadata.clone();
        webview_builder = webview_builder.with_on_page_load_handler(move |event, url| {
            if let Ok(url) = url.parse() {
                page_load_handler(
                    &metadata,
                    url,
                    match event {
                        wry::PageLoadEvent::Started => PageLoadEvent::Started,
                        wry::PageLoadEvent::Finished => PageLoadEvent::Finished,
                    },
                );
            }
        });
    }

    #[cfg(target_os = "ios")]
    {
        webview_builder =
            webview_builder.with_limit_navigations_to_app_bound_domains(limit_navigations_to_app_bound_domains);

        if let Some(input_accessory_view_builder) = input_accessory_view_builder {
            webview_builder = webview_builder
                .with_input_accessory_view_builder(move |webview| input_accessory_view_builder.0(webview));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(position) = &traffic_light_position {
            webview_builder = webview_builder.with_traffic_light_inset(*position);
        }
    }

    for (scheme, protocol) in uri_scheme_protocols {
        let metadata = metadata.clone();
        webview_builder =
            webview_builder.with_asynchronous_custom_protocol(scheme, move |webview_id, request, responder| {
                protocol(
                    &metadata,
                    webview_id,
                    request,
                    Box::new(move |response| responder.respond(response)),
                );
            });
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    if let Some(on_web_content_process_terminate_handler) = on_web_content_process_terminate_handler {
        let metadata = metadata.clone();
        webview_builder = webview_builder.with_on_web_content_process_terminate_handler(move || {
            on_web_content_process_terminate_handler(&metadata);
        });
    }

    let webview = if kind {
        webview_builder.build(window)
    } else {
        #[cfg(any(target_os = "windows", target_os = "macos",))]
        {
            webview_builder.build_as_child(window)
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
        ))]
        {
            use tao::platform::unix::WindowExtUnix;

            let container = window.default_vbox().expect("Tao window has no GTK container");

            webview_builder.build_gtk(container)
        }

        #[cfg(any(target_os = "android", target_os = "ios",))]
        {
            webview_builder.build(window)
        }
    }
    .map_err(|e| crate::error::Error::CreateWebview(Box::new(e)))?;

    let webview = Rc::new(webview);
    Ok(ManagedWebview {
        metadata,
        context_store: web_context_store.clone(),
        context_key: if automation_enabled { None } else { web_context_key },
        bounds: Arc::new(Mutex::new(webview_bounds)),
        inner: webview,
    })
}
