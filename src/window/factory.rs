use crate::{platform::prelude::WindowExt, types::FocusState};
use dpi::PhysicalSize;
use std::sync::Mutex;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tao::event_loop::EventLoopWindowTarget;

#[cfg(target_os = "macos")]
use dpi::Position;

use crate::{
    platform::prelude::{MonitorExt, calculate_window_center_position},
    types::{WebContextStore, WebviewUrl},
    utils::{WindowWebViewMetaData, find_monitor_for_position},
    webview::WebViewId,
    window::{ManagedWindow, WindowId, builder::WindowBuilder},
};

#[cfg(target_os = "macos")]
use crate::utils::TitleBarStyle;

#[cfg(target_os = "android")]
use tao::platform::android::WindowBuilderExtAndroid;

#[cfg(target_os = "ios")]
use tao::platform::ios::WindowBuilderExtIOS;

#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use tao::platform::unix::WindowBuilderExtUnix;

pub fn create_window<T: 'static, F>(
    builder: WindowBuilder,
    window_target: &EventLoopWindowTarget<T>,
    window_id: WindowId,
    web_context_store: WebContextStore,
    befor_webview_creation: Option<F>,
) -> crate::error::Result<ManagedWindow>
where
    F: for<'a> Fn(
            wry::WebViewBuilder<'a>,
            WebviewUrl,
        ) -> crate::error::Result<wry::WebViewBuilder<'a>>
        + Send
        + Clone
        + 'static,
{
    let WindowBuilder {
        label,
        mut inner,
        center,
        prevent_overflow,
        close_requested_handler,
        next_webview_id,
        pending_webviews,

        #[cfg(target_os = "macos")]
        tabbing_identifier,
        on_window_event,
    } = builder;

    // -------------------------------------------------------------------------
    // Platform state needed after the window has been created
    // -------------------------------------------------------------------------

    #[cfg(windows)]
    let background_color = inner.window.background_color;

    #[cfg(windows)]
    let is_window_transparent = inner.window.transparent;

    // -------------------------------------------------------------------------
    // macOS automatic tabbing
    // -------------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    {
        if tabbing_identifier.is_none()
            || inner.window.transparent
            || !inner.window.decorations
        {
            inner = inner.with_automatic_window_tabbing(false);
        }
    }
    #[cfg(windows)]
    let focused_webview = Arc::new(Mutex::new(FocusState::default()));
    // -------------------------------------------------------------------------
    // Center / prevent overflow
    // -------------------------------------------------------------------------

    if prevent_overflow.is_some() || center {
        let monitor = if let Some(window_position) = &inner.window.position {
            find_monitor_for_position(
                window_target.available_monitors(),
                *window_position,
            )
        } else {
            window_target.primary_monitor()
        };
        if let Some(monitor) = monitor {
            let scale_factor = monitor.scale_factor();
            let desired_size = inner
                .window
                .inner_size
                .unwrap_or_else(|| PhysicalSize::new(800, 600).into());
            let mut inner_size = inner
                .window
                .inner_size_constraints
                .clamp(desired_size, scale_factor)
                .to_physical::<u32>(scale_factor);
            let mut window_size = inner_size;
            #[allow(unused_mut)]
            // Left and right window shadow counts as part of the window on Windows
            // We need to include it when calculating positions, but not size
            let mut shadow_width = 0;
            #[cfg(windows)]
            if inner.window.decorations {
                use windows::Win32::UI::WindowsAndMessaging::{
                    AdjustWindowRect, WS_OVERLAPPEDWINDOW,
                };
                let mut rect = windows::Win32::Foundation::RECT::default();
                let result = unsafe {
                    AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, false)
                };
                if result.is_ok() {
                    shadow_width = (rect.right - rect.left) as u32;
                    // rect.bottom is made out of shadow, and we don't care about it
                    window_size.height += -rect.top as u32;
                }
            }

            if let Some(margin) = prevent_overflow {
                let work_area = monitor.work_area();
                let margin = margin.to_physical::<u32>(scale_factor);
                let constraint = PhysicalSize::new(
                    work_area.size.width - margin.width,
                    work_area.size.height - margin.height,
                );
                if window_size.width > constraint.width
                    || window_size.height > constraint.height
                {
                    if window_size.width > constraint.width {
                        inner_size.width = inner_size.width.saturating_sub(
                            window_size.width - constraint.width,
                        );
                        window_size.width = constraint.width;
                    }
                    if window_size.height > constraint.height {
                        inner_size.height = inner_size.height.saturating_sub(
                            window_size.height - constraint.height,
                        );
                        window_size.height = constraint.height;
                    }
                    inner.window.inner_size = Some(inner_size.into());
                }
            }

            if center {
                window_size.width += shadow_width;

                let position =
                    calculate_window_center_position(window_size, monitor);

                let logical_position = position.to_logical::<f64>(scale_factor);

                inner = inner.with_position(logical_position);
            }
        }
    };

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let (initial_position, is_fullscreen) =
        (inner.window.position, inner.window.fullscreen.is_some());

    // If fullscreen is requested with an explicit position, resolve the target
    // monitor up front so the window is created fullscreen on that display.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if let (true, Some(position)) = (is_fullscreen, initial_position) {
        if let Some(target_monitor) = find_monitor_for_position(
            window_target.available_monitors(),
            position,
        ) {
            inner.window.fullscreen =
                Some(Fullscreen::Borderless(Some(target_monitor)));
        }
    }

    let window = inner
        .build(window_target)
        .inspect_err(|e| log::error!("Error creating window: {e:?}"))
        .map_err(|_| crate::error::Error::CreateWindow)?;

    let has_children = pending_webviews.len() > 1
        || pending_webviews.iter().any(|builder| builder.kind);

    let mut webviews = Vec::with_capacity(pending_webviews.len());

    let mut window_webview_metadata: Option<WindowWebViewMetaData> = None;

    for webview_builder in pending_webviews {
        let webview_id: WebViewId =
            next_webview_id.fetch_add(1, Ordering::Relaxed).into();

        let webview_label = webview_builder.label.clone();

        let webview_metadata = WindowWebViewMetaData::new(
            window_id,
            webview_id,
            window.id(),
            label.clone(),
            webview_label,
        )?;

        // Metadata der ersten WebView als Metadata des ManagedWindow merken.
        if window_webview_metadata.is_none() {
            window_webview_metadata = Some(webview_metadata.clone());
        }

        let webview = webview_builder.build(
            &window,
            &webview_metadata,
            web_context_store.clone(),
            befor_webview_creation.clone(),
        )?;

        webviews.push(webview);
    }
    let metadata = window_webview_metadata
        .ok_or(crate::error::Error::WebviewNotFound("root".to_string()))?;
    let window = Arc::new(window);

    #[cfg(windows)]
    let surface = if is_window_transparent {
        if let Ok(context) = softbuffer::Context::new(window.clone()) {
            if let Ok(mut surface) =
                softbuffer::Surface::new(&context, window.clone())
            {
                window.draw_surface(&mut surface, background_color);
                Some(surface)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    Ok(ManagedWindow {
        on_window_event,
        metadata,
        has_children: AtomicBool::new(has_children),
        inner: Some(window),
        webviews,
        #[cfg(windows)]
        background_color,
        #[cfg(windows)]
        is_window_transparent,
        #[cfg(windows)]
        surface,
        #[cfg(windows)]
        focused_webview,
        close_requested_handler,
    })
}
