use dpi::{self, LogicalPosition, LogicalSize, PhysicalSize, Size};

use std::{
    fmt,
    sync::{Arc, atomic::AtomicU32, mpsc::Sender},
};
use tao::{
    event_loop::EventLoopWindowTarget,
    window::{Fullscreen, Theme as TaoTheme, WindowBuilder as TaoWindowBuilder},
};

#[cfg(target_os = "macos")]
use dpi::Position;

use crate::{
    error::Result,
    types::{
        CloseRequestedHandler, Color, PreventOverflowConfig, Theme, WebContextStore, WebviewUrl, WindowConfig,
        WindowEventHandler, WindowSizeConstraints,
    },
    utils::{
        Icon, RawWindow, WindowWebViewMetaData,
        wrappers::{TaoIcon, WindowEvent},
    },
    webview::WebViewBuilder,
    window::{ManagedWindow, WindowId, factory::create_window},
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

#[cfg(windows)]
use tao::platform::windows::WindowBuilderExtWindows;

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

pub struct WindowBuilder {
    pub on_window_event: Option<WindowEventHandler>,
    pub label: String,
    pub inner: TaoWindowBuilder,
    pub center: bool,
    pub prevent_overflow: Option<Size>,
    #[cfg(target_os = "macos")]
    pub tabbing_identifier: Option<String>,
    pub close_requested_handler: Option<CloseRequestedHandler>,
    pub next_webview_id: Arc<AtomicU32>,
    pub pending_webviews: Vec<WebViewBuilder>,
}

impl std::fmt::Debug for WindowBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("WindowBuilder");
        s.field("inner", &self.inner)
            .field("center", &self.center)
            .field("prevent_overflow", &self.prevent_overflow);
        #[cfg(target_os = "macos")]
        {
            s.field("tabbing_identifier", &self.tabbing_identifier);
        }
        s.finish()
    }
}
impl Default for WindowBuilder {
    fn default() -> Self {
        Self {
            on_window_event: None,
            label: "main".to_string(),
            inner: TaoWindowBuilder::default(),
            center: false,
            prevent_overflow: None,

            #[cfg(target_os = "macos")]
            tabbing_identifier: None,

            close_requested_handler: None,
            next_webview_id: Arc::new(AtomicU32::new(0)),
            pending_webviews: Vec::new(),
        }
    }
}
// SAFETY: this type is `Send` since `menu_items` are read only here
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for WindowBuilder {}

impl WindowBuilder {
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let mut builder = Self::default().focused(true);

        #[cfg(target_os = "macos")]
        {
            // TODO: find a proper way to prevent webview being pushed out of the window.
            // Workaround for issue: https://github.com/tauri-apps/tauri/issues/10225
            // The window requires `NSFullSizeContentViewWindowMask` flag to prevent devtools
            // pushing the content view out of the window.
            // By setting the default style to `TitleBarStyle::Visible` should fix the issue for most of the users.
            builder = builder.title_bar_style(TitleBarStyle::Visible);
        }

        builder = builder.title("Tauri App");

        #[cfg(windows)]
        {
            builder = builder.window_classname("Tauri Window");
        }

        builder
    }
    pub fn on_window_event<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WindowWebViewMetaData, &WindowEvent) + Send + Sync + 'static,
    {
        self.on_window_event = Some(Arc::new(handler));
        self
    }
    pub fn label<S: Into<String>>(mut self, label: S) -> Self {
        self.label = label.into();
        self
    }
    pub fn with_close_requested<F>(mut self, handler: F) -> Self
    where
        F: Fn(Sender<bool>) + Send + Sync + 'static,
    {
        self.close_requested_handler = Some(Arc::new(handler));
        self
    }
    pub fn with_config(config: &WindowConfig) -> Self {
        let mut window = WindowBuilder::new();

        #[cfg(target_os = "macos")]
        {
            window = window
                .hidden_title(config.hidden_title)
                .title_bar_style(config.title_bar_style);
            if let Some(identifier) = &config.tabbing_identifier {
                window = window.tabbing_identifier(identifier);
            }
            if let Some(position) = &config.traffic_light_position {
                window = window.traffic_light_position(dpi::LogicalPosition::new(position.x, position.y));
            }
        }

        #[cfg(any(not(target_os = "macos"), feature = "macos-private-api"))]
        {
            window = window.transparent(config.transparent);
        }
        #[cfg(all(target_os = "macos", not(feature = "macos-private-api"), debug_assertions))]
        if config.transparent {
            eprintln!(
        "The window is set to be transparent but the `macos-private-api` is not enabled.
        This can be enabled via the `tauri.macOSPrivateApi` configuration property <https://v2.tauri.app/reference/config/#macosprivateapi>
      ");
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            // Mouse event is disabled on Linux since sudden event bursts could block event loop.
            window.inner = window.inner.with_cursor_moved_event(false);
        }

        #[cfg(target_os = "android")]
        {
            if let Some(activity_name) = &config.activity_name {
                window.inner = window.inner.with_activity_name(activity_name.clone());
            }
            if let Some(activity_name) = &config.created_by_activity_name {
                window.inner = window.inner.with_created_by_activity_name(activity_name.clone());
            }
        }

        #[cfg(target_os = "ios")]
        {
            if let Some(scene_identifier) = &config.requested_by_scene_identifier {
                window.inner = window.inner.with_requesting_scene_identifier(scene_identifier.clone());
            }
        }

        // ignore size from config for mobile for backward compatibility
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            window = window.inner_size(config.width, config.height);
        }

        window = window
            .title(config.title.to_string())
            .focused(config.focus)
            .focusable(config.focusable)
            .visible(config.visible)
            .resizable(config.resizable)
            .fullscreen(config.fullscreen)
            .decorations(config.decorations)
            .maximized(config.maximized)
            .always_on_bottom(config.always_on_bottom)
            .always_on_top(config.always_on_top)
            .visible_on_all_workspaces(config.visible_on_all_workspaces)
            .content_protected(config.content_protected)
            .skip_taskbar(config.skip_taskbar)
            .theme(config.theme)
            .no_redirection_bitmap(config.no_redirection_bitmap)
            .closable(config.closable)
            .maximizable(config.maximizable)
            .minimizable(config.minimizable)
            .shadow(config.shadow);

        let mut constraints = WindowSizeConstraints::default();

        if let Some(min_width) = config.min_width {
            constraints.min_width = Some(tao::dpi::LogicalUnit::new(min_width).into());
        }
        if let Some(min_height) = config.min_height {
            constraints.min_height = Some(tao::dpi::LogicalUnit::new(min_height).into());
        }
        if let Some(max_width) = config.max_width {
            constraints.max_width = Some(tao::dpi::LogicalUnit::new(max_width).into());
        }
        if let Some(max_height) = config.max_height {
            constraints.max_height = Some(tao::dpi::LogicalUnit::new(max_height).into());
        }
        if let Some(color) = config.background_color {
            window = window.background_color(color);
        }
        window = window.inner_size_constraints(constraints);

        if let (Some(x), Some(y)) = (config.x, config.y) {
            window = window.position(x, y);
        }

        if config.center {
            window = window.center();
        }

        if let Some(window_classname) = &config.window_classname {
            window = window.window_classname(window_classname);
        }

        if let Some(prevent_overflow) = &config.prevent_overflow {
            window = match prevent_overflow {
                PreventOverflowConfig::Enable(true) => window.prevent_overflow(),
                PreventOverflowConfig::Margin(margin) => {
                    window.prevent_overflow_with_margin(PhysicalSize::new(margin.width, margin.height).into())
                }
                _ => window,
            };
        }

        window
    }

    pub fn add_webview_builder(mut self, builder: WebViewBuilder) -> Self {
        self.pending_webviews.push(builder);
        self
    }
    pub fn center(mut self) -> Self {
        self.center = true;
        self
    }

    pub fn position(mut self, x: f64, y: f64) -> Self {
        self.inner = self.inner.with_position(LogicalPosition::new(x, y));
        self
    }

    pub fn inner_size(mut self, width: f64, height: f64) -> Self {
        self.inner = self.inner.with_inner_size(LogicalSize::new(width, height));
        self
    }

    pub fn min_inner_size(mut self, min_width: f64, min_height: f64) -> Self {
        self.inner = self.inner.with_min_inner_size(LogicalSize::new(min_width, min_height));
        self
    }

    pub fn max_inner_size(mut self, max_width: f64, max_height: f64) -> Self {
        self.inner = self.inner.with_max_inner_size(LogicalSize::new(max_width, max_height));
        self
    }

    pub fn inner_size_constraints(mut self, constraints: WindowSizeConstraints) -> Self {
        self.inner.window.inner_size_constraints = tao::window::WindowSizeConstraints {
            min_width: constraints.min_width,
            min_height: constraints.min_height,
            max_width: constraints.max_width,
            max_height: constraints.max_height,
        };
        self
    }

    /// Prevent the window from overflowing the working area (e.g. monitor size - taskbar size) on creation
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    pub fn prevent_overflow(mut self) -> Self {
        self.prevent_overflow.replace(PhysicalSize::new(0, 0).into());
        self
    }

    /// Prevent the window from overflowing the working area (e.g. monitor size - taskbar size)
    /// on creation with a margin
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    pub fn prevent_overflow_with_margin(mut self, margin: Size) -> Self {
        self.prevent_overflow.replace(margin);
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.inner = self.inner.with_resizable(resizable);
        self
    }

    pub fn maximizable(mut self, maximizable: bool) -> Self {
        self.inner = self.inner.with_maximizable(maximizable);
        self
    }

    pub fn minimizable(mut self, minimizable: bool) -> Self {
        self.inner = self.inner.with_minimizable(minimizable);
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.inner = self.inner.with_closable(closable);
        self
    }

    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.inner = self.inner.with_title(title.into());
        self
    }

    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.inner = if fullscreen {
            self.inner.with_fullscreen(Some(Fullscreen::Borderless(None)))
        } else {
            self.inner.with_fullscreen(None)
        };
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.inner = self.inner.with_focused(focused);
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.inner = self.inner.with_focusable(focusable);
        self
    }

    pub fn maximized(mut self, maximized: bool) -> Self {
        self.inner = self.inner.with_maximized(maximized);
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.inner = self.inner.with_visible(visible);
        self
    }

    #[cfg(any(not(target_os = "macos"), feature = "macos-private-api"))]
    pub fn transparent(mut self, transparent: bool) -> Self {
        self.inner = self.inner.with_transparent(transparent);
        self
    }

    pub fn decorations(mut self, decorations: bool) -> Self {
        self.inner = self.inner.with_decorations(decorations);
        self
    }

    pub fn always_on_bottom(mut self, always_on_bottom: bool) -> Self {
        self.inner = self.inner.with_always_on_bottom(always_on_bottom);
        self
    }

    pub fn always_on_top(mut self, always_on_top: bool) -> Self {
        self.inner = self.inner.with_always_on_top(always_on_top);
        self
    }

    pub fn visible_on_all_workspaces(mut self, visible_on_all_workspaces: bool) -> Self {
        self.inner = self.inner.with_visible_on_all_workspaces(visible_on_all_workspaces);
        self
    }

    pub fn content_protected(mut self, protected: bool) -> Self {
        self.inner = self.inner.with_content_protection(protected);
        self
    }

    pub fn shadow(#[allow(unused_mut)] mut self, _enable: bool) -> Self {
        #[cfg(windows)]
        {
            self.inner = self.inner.with_undecorated_shadow(_enable);
        }
        #[cfg(target_os = "macos")]
        {
            self.inner = self.inner.with_has_shadow(_enable);
        }
        self
    }

    #[cfg(windows)]
    pub fn owner(mut self, owner: HWND) -> Self {
        self.inner = self.inner.with_owner_window(owner.0 as _);
        self
    }

    #[cfg(windows)]
    pub fn parent(mut self, parent: HWND) -> Self {
        self.inner = self.inner.with_parent_window(parent.0 as _);
        self
    }

    #[cfg(target_os = "macos")]
    pub fn parent(mut self, parent: *mut std::ffi::c_void) -> Self {
        self.inner = self.inner.with_parent_window(parent);
        self
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn transient_for(mut self, parent: &impl gtk::glib::IsA<gtk::Window>) -> Self {
        self.inner = self.inner.with_transient_for(parent);
        self
    }

    #[cfg(windows)]
    pub fn drag_and_drop(mut self, enabled: bool) -> Self {
        self.inner = self.inner.with_drag_and_drop(enabled);
        self
    }

    #[cfg(target_os = "macos")]
    pub fn title_bar_style(mut self, style: TitleBarStyle) -> Self {
        match style {
            TitleBarStyle::Visible => {
                self.inner = self.inner.with_titlebar_transparent(false);
                // Fixes rendering issue when resizing window with devtools open (https://github.com/tauri-apps/tauri/issues/3914)
                self.inner = self.inner.with_fullsize_content_view(true);
            }
            TitleBarStyle::Transparent => {
                self.inner = self.inner.with_titlebar_transparent(true);
                self.inner = self.inner.with_fullsize_content_view(false);
            }
            TitleBarStyle::Overlay => {
                self.inner = self.inner.with_titlebar_transparent(true);
                self.inner = self.inner.with_fullsize_content_view(true);
            }
            unknown => {
                #[cfg(feature = "tracing")]
                tracing::warn!("unknown title bar style applied: {unknown}");

                #[cfg(not(feature = "tracing"))]
                eprintln!("unknown title bar style applied: {unknown}");
            }
        }
        self
    }

    #[cfg(target_os = "macos")]
    pub fn traffic_light_position<P: Into<Position>>(mut self, position: P) -> Self {
        self.inner = self.inner.with_traffic_light_inset(position.into());
        self
    }

    #[cfg(target_os = "macos")]
    pub fn hidden_title(mut self, hidden: bool) -> Self {
        self.inner = self.inner.with_title_hidden(hidden);
        self
    }

    #[cfg(target_os = "macos")]
    pub fn tabbing_identifier(mut self, identifier: &str) -> Self {
        self.inner = self.inner.with_tabbing_identifier(identifier);
        self.tabbing_identifier.replace(identifier.into());
        self
    }

    pub fn icon(mut self, icon: Icon) -> Result<Self> {
        self.inner = self.inner.with_window_icon(Some(TaoIcon::try_from(icon)?.0));
        Ok(self)
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.inner = self.inner.with_background_color(color.into());
        self
    }

    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn skip_taskbar(mut self, skip: bool) -> Self {
        self.inner = self.inner.with_skip_taskbar(skip);
        self
    }

    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
    pub fn skip_taskbar(self, _skip: bool) -> Self {
        self
    }

    pub fn theme(mut self, theme: Option<Theme>) -> Self {
        self.inner = self.inner.with_theme(if let Some(t) = theme {
            match t {
                Theme::Dark => Some(TaoTheme::Dark),
                _ => Some(TaoTheme::Light),
            }
        } else {
            None
        });

        self
    }

    pub fn has_icon(&self) -> bool {
        self.inner.window.window_icon.is_some()
    }

    pub fn get_theme(&self) -> Option<Theme> {
        self.inner.window.preferred_theme.map(|theme| match theme {
            TaoTheme::Dark => Theme::Dark,
            _ => Theme::Light,
        })
    }

    #[cfg(windows)]
    pub fn window_classname<S: Into<String>>(mut self, window_classname: S) -> Self {
        self.inner = self.inner.with_window_classname(window_classname);
        self
    }
    #[cfg(not(windows))]
    pub fn window_classname<S: Into<String>>(self, _window_classname: S) -> Self {
        self
    }

    pub fn no_redirection_bitmap(#[allow(unused_mut)] mut self, _enable: bool) -> Self {
        #[cfg(windows)]
        {
            self.inner = self.inner.with_no_redirection_bitmap(_enable);
        }
        self
    }

    #[cfg(target_os = "android")]
    pub fn activity_name<S: Into<String>>(mut self, class_name: S) -> Self {
        self.inner = self.inner.with_activity_name(class_name.into());
        self
    }

    #[cfg(target_os = "android")]
    pub fn created_by_activity_name<S: Into<String>>(mut self, class_name: S) -> Self {
        self.inner = self.inner.with_created_by_activity_name(class_name.into());
        self
    }

    #[cfg(target_os = "ios")]
    pub fn requested_by_scene_identifier<S: Into<String>>(mut self, identifier: S) -> Self {
        self.inner = self.inner.with_requesting_scene_identifier(identifier.into());
        self
    }

    pub fn build<T: 'static, F, M: Fn(RawWindow) + Send + 'static>(
        self,
        window_target: &EventLoopWindowTarget<T>,
        window_id: WindowId,
        web_context_store: WebContextStore,
        befor_webview_creation: Option<F>,
        after_window_creation: Option<M>,
    ) -> crate::error::Result<ManagedWindow>
    where
        F: for<'a> Fn(wry::WebViewBuilder<'a>, WebviewUrl) -> crate::error::Result<wry::WebViewBuilder<'a>>
            + Send
            + Clone
            + 'static,
    {
        create_window(
            self,
            window_target,
            window_id,
            web_context_store,
            after_window_creation,
            befor_webview_creation,
        )
    }
}
