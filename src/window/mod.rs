use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::prelude::ContextMenu;
use dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use muda::MenuId;
use tao::window::{Fullscreen, Window, WindowId as TaoWindowId};

#[cfg(target_os = "macos")]
use tao::platform::macos::WindowExtMacOS;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use tao::platform::unix::WindowExtUnix;

#[cfg(windows)]
use tao::platform::windows::WindowExtWindows;

#[cfg(windows)]
use crate::{types::FocusState, utils::ArcMut};

#[cfg(windows)]
use crate::platform::prelude::{attach_resize_handler, detach_resize_handler, update_drag_hwnd_rgn_for_undecorated};

use crate::{
    error::{Error, Result},
    platform::prelude::WindowExt,
    prelude::Menu,
    types::{
        CloseRequestedHandler, Color, CursorIcon, Monitor, ProgressBarState, ResizeDirection, Theme, TitleBarStyle,
        UserAttentionType, WindowEventHandler, WindowSizeConstraints,
    },
    utils::{
        Icon, WindowWebViewMetaData, find_monitor_for_position, inner_size,
        wrappers::{
            CursorIconWrapper, MonitorHandleWrapper, ProgressBarStateWrapper, TaoIcon, UserAttentionTypeWrapper,
            WindowEventWrapper,
        },
    },
    webview::{ManagedWebview, WebViewId},
};

mod builder;
pub(crate) mod factory;

pub use self::builder::WindowBuilder;

/// Identifier of a window.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct WindowId(u32);

impl From<u32> for WindowId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// Ein einem Fenster zugeordnetes Menü.
///
/// `is_app_wide` gibt an, ob das Menü von mehreren Fenstern als
/// anwendungsweites Menü gemeinsam verwendet wird.
pub struct WindowMenu {
    pub is_app_wide: bool,
    pub menu: Menu,
}

/// Setup-Callback, der ein [`WindowMenu`] für ein Fenster baut.
///
/// Bekommt das native Tao-Fenster, damit der Callback z. B.
/// `menu.popup(window)` oder `window.set_menu(...)` aufrufen kann.
pub type SetupMenu = Box<dyn Fn(&Window) -> Result<WindowMenu> + Send + 'static>;

pub struct ManagedWindow {
    pub(crate) metadata: WindowWebViewMetaData,
    pub inner: Option<Arc<Window>>,
    pub(crate) menu: Arc<Mutex<Option<WindowMenu>>>,
    pub on_window_event: Option<WindowEventHandler>,
    pub close_requested_handler: Option<CloseRequestedHandler>,
    pub has_children: AtomicBool,
    pub webviews: Vec<ManagedWebview>,

    #[cfg(windows)]
    pub background_color: Option<tao::window::RGBA>,
    #[cfg(windows)]
    pub is_window_transparent: bool,
    #[cfg(windows)]
    pub surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    #[cfg(windows)]
    pub focused_webview: ArcMut<FocusState>,
}

impl fmt::Debug for ManagedWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedWindow")
            .field("label", &self.metadata.window_label)
            .field("inner", &self.inner)
            .field("has_close_requested_handler", &self.close_requested_handler.is_some())
            .finish() // menu wird bewusst weggelassen
    }
}
impl ManagedWindow {
    pub fn id(&self) -> TaoWindowId {
        self.metadata.native_id
    }

    pub fn inner_window(&self) -> Result<&Window> {
        self.inner
            .as_deref()
            .ok_or_else(|| Error::WindowNotInitialized(self.metadata.window_label.clone()))
    }

    pub fn window_atomic_id(&self) -> WindowId {
        self.metadata.window_id
    }

    pub fn webview_id(&self) -> WebViewId {
        self.metadata.webview_id
    }
    pub fn webview(&self, label: &str) -> Result<&ManagedWebview> {
        self.webviews
            .iter()
            .find(|webview| webview.metadata.webview_label == label)
            .ok_or_else(|| Error::WebviewNotFound(label.to_string()))
    }
    pub fn get_webview_mut(&mut self, label: &str) -> Result<&mut ManagedWebview> {
        self.webviews
            .iter_mut()
            .find(|webview| webview.metadata.webview_label == label)
            .ok_or_else(|| Error::WebviewNotFound(label.to_string()))
    }
    pub fn set_resizable(&self, resizable: bool) -> Result<()> {
        let window = self.window()?;

        window.set_resizable(resizable);

        #[cfg(windows)]
        {
            if !resizable {
                detach_resize_handler(window.hwnd());
            } else if !window.is_decorated() {
                attach_resize_handler(window.hwnd(), window.has_undecorated_shadow());
            }
        }

        Ok(())
    }

    pub fn set_decorations(&self, decorations: bool) -> Result<()> {
        let window = self.window()?;

        window.set_decorations(decorations);

        #[cfg(windows)]
        {
            if decorations {
                detach_resize_handler(window.hwnd());
            } else if window.is_resizable() {
                attach_resize_handler(window.hwnd(), window.has_undecorated_shadow());
            }
        }

        Ok(())
    }

    pub fn set_shadow(&self, enable: bool) -> Result<()> {
        let window = self.window()?;

        #[cfg(windows)]
        {
            window.set_undecorated_shadow(enable);

            update_drag_hwnd_rgn_for_undecorated(window.hwnd(), enable);
        }

        #[cfg(target_os = "macos")]
        window.set_has_shadow(enable);

        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = enable;

        Ok(())
    }
    #[inline]
    fn window(&self) -> Result<&Window> {
        self.inner
            .as_deref()
            .ok_or_else(|| Error::WindowNotInitialized(self.metadata.window_label.clone()))
    }

    pub fn label(&self) -> &str {
        &self.metadata.window_label
    }

    pub fn is_initialized(&self) -> bool {
        self.inner.is_some()
    }

    // -------------------------------------------------------------------------
    // Getters
    // -------------------------------------------------------------------------

    pub fn scale_factor(&self) -> Result<f64> {
        Ok(self.window()?.scale_factor())
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>> {
        self.window()?.inner_position().map_err(|_| Error::FailedToSendMessage)
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>> {
        self.window()?.outer_position().map_err(|_| Error::FailedToSendMessage)
    }

    pub fn inner_size(&self) -> Result<PhysicalSize<u32>> {
        let window = self.window()?;

        Ok(inner_size(
            window,
            &self.webviews,
            self.has_children.load(Ordering::Relaxed),
        ))
    }

    pub fn outer_size(&self) -> Result<PhysicalSize<u32>> {
        Ok(self.window()?.outer_size())
    }

    pub fn is_fullscreen(&self) -> Result<bool> {
        Ok(self.window()?.fullscreen().is_some())
    }

    pub fn is_minimized(&self) -> Result<bool> {
        Ok(self.window()?.is_minimized())
    }

    pub fn is_maximized(&self) -> Result<bool> {
        Ok(self.window()?.is_maximized())
    }

    pub fn is_focused(&self) -> Result<bool> {
        #[cfg(windows)]
        {
            if self.has_children.load(Ordering::Relaxed) {
                return Ok(matches!(
                    *self.focused_webview.lock().unwrap(),
                    FocusState::WindowFocused | FocusState::WebviewFocused { .. }
                ));
            }
        }

        Ok(self.window()?.is_focused())
    }

    pub fn is_decorated(&self) -> Result<bool> {
        Ok(self.window()?.is_decorated())
    }

    pub fn is_resizable(&self) -> Result<bool> {
        Ok(self.window()?.is_resizable())
    }

    pub fn is_maximizable(&self) -> Result<bool> {
        Ok(self.window()?.is_maximizable())
    }

    pub fn is_minimizable(&self) -> Result<bool> {
        Ok(self.window()?.is_minimizable())
    }

    pub fn is_closable(&self) -> Result<bool> {
        Ok(self.window()?.is_closable())
    }

    pub fn is_visible(&self) -> Result<bool> {
        Ok(self.window()?.is_visible())
    }

    pub fn get_title(&self) -> Result<String> {
        Ok(self.window()?.title())
    }

    pub fn current_monitor(&self) -> Result<Option<Monitor>> {
        Ok(self.window()?.current_monitor().map(|m| MonitorHandleWrapper(m).into()))
    }

    pub fn primary_monitor(&self) -> Result<Option<Monitor>> {
        Ok(self.window()?.primary_monitor().map(|m| MonitorHandleWrapper(m).into()))
    }

    pub fn monitor_from_point(&self, x: f64, y: f64) -> Result<Option<Monitor>> {
        Ok(self
            .window()?
            .monitor_from_point(x, y)
            .map(|m| MonitorHandleWrapper(m).into()))
    }

    pub fn available_monitors(&self) -> Result<Vec<Monitor>> {
        Ok(self
            .window()?
            .available_monitors()
            .map(|m| MonitorHandleWrapper(m).into())
            .collect())
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn gtk_window(&self) -> Result<gtk::Window> {
        Ok(self.window()?.gtk_window().clone())
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn gtk_box(&self) -> Result<gtk::Box> {
        self.window()?.default_vbox().cloned().ok_or(Error::FailedToSendMessage)
    }

    #[cfg(target_os = "android")]
    pub fn activity_name(&self) -> Result<String> {
        Ok(self.window()?.activity_name())
    }

    #[cfg(target_os = "ios")]
    pub fn scene_identifier(&self) -> Result<String> {
        Ok(self.window()?.scene_identifier())
    }

    pub fn theme(&self) -> Result<Theme> {
        Ok(match self.window()?.theme() {
            tao::window::Theme::Dark => Theme::Dark,
            _ => Theme::Light,
        })
    }

    pub fn raw_window_handle(&self) -> Result<raw_window_handle::RawWindowHandle> {
        use raw_window_handle::HasWindowHandle;

        self.window()?
            .window_handle()
            .map(|handle| handle.as_raw())
            .map_err(|_| Error::FailedToSendMessage)
    }

    pub fn is_enabled(&self) -> Result<bool> {
        Ok(self.window()?.is_enabled())
    }

    pub fn is_always_on_top(&self) -> Result<bool> {
        Ok(self.window()?.is_always_on_top())
    }

    // -------------------------------------------------------------------------
    // Setters / commands
    // -------------------------------------------------------------------------

    pub fn center(&self) -> Result<()> {
        self.window()?.center();
        Ok(())
    }

    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) -> Result<()> {
        self.window()?
            .request_user_attention(request_type.map(|request| UserAttentionTypeWrapper::from(request).0));
        Ok(())
    }

    pub fn set_maximizable(&self, maximizable: bool) -> Result<()> {
        self.window()?.set_maximizable(maximizable);
        Ok(())
    }

    pub fn set_minimizable(&self, minimizable: bool) -> Result<()> {
        self.window()?.set_minimizable(minimizable);
        Ok(())
    }

    pub fn set_closable(&self, closable: bool) -> Result<()> {
        self.window()?.set_closable(closable);
        Ok(())
    }

    pub fn set_title(&self, title: impl AsRef<str>) -> Result<()> {
        self.window()?.set_title(title.as_ref());
        Ok(())
    }

    pub fn maximize(&self) -> Result<()> {
        self.window()?.set_maximized(true);
        Ok(())
    }

    pub fn unmaximize(&self) -> Result<()> {
        self.window()?.set_maximized(false);
        Ok(())
    }

    pub fn minimize(&self) -> Result<()> {
        self.window()?.set_minimized(true);
        Ok(())
    }

    pub fn unminimize(&self) -> Result<()> {
        self.window()?.set_minimized(false);
        Ok(())
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        self.window()?.set_enabled(enabled);
        Ok(())
    }

    pub fn show(&self) -> Result<()> {
        self.window()?.set_visible(true);
        Ok(())
    }

    pub fn hide(&self) -> Result<()> {
        self.window()?.set_visible(false);
        Ok(())
    }

    /// Requests that this managed window be closed.
    ///
    /// The actual destruction remains on the event-loop side. This is deliberate:
    /// `ManagedWindow` stores the native window in an `Arc`, so dropping only this
    /// handle is not a reliable cross-owner close mechanism.
    pub fn close(&self) -> Result<bool> {
        self.window()?;

        let Some(handler) = &self.close_requested_handler else {
            return Ok(true);
        };

        let (tx, rx) = std::sync::mpsc::channel();
        handler(tx);

        match rx.recv_timeout(Duration::from_millis(100)) {
            // true = prevent close
            Ok(true) => Ok(false),
            // false = allow close
            Ok(false) => Ok(true),
            // timeout / disconnect = allow by default
            Err(_) => Ok(true),
        }
    }
    /// Immediately detaches this wrapper from the native window.
    ///
    /// Prefer `close()` when an event loop owns window lifetime.
    pub fn destroy(&mut self) {
        self.inner = None;
        #[cfg(windows)]
        self.surface.take();
    }

    pub fn set_always_on_bottom(&self, always_on_bottom: bool) -> Result<()> {
        self.window()?.set_always_on_bottom(always_on_bottom);
        Ok(())
    }

    pub fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
        self.window()?.set_always_on_top(always_on_top);
        Ok(())
    }

    pub fn set_visible_on_all_workspaces(&self, visible: bool) -> Result<()> {
        self.window()?.set_visible_on_all_workspaces(visible);
        Ok(())
    }

    pub fn set_content_protected(&self, protected: bool) -> Result<()> {
        self.window()?.set_content_protection(protected);
        Ok(())
    }

    pub fn set_size<S: Into<Size>>(&self, size: S) -> Result<()> {
        self.window()?.set_inner_size(size.into());
        Ok(())
    }

    pub fn set_min_size<S: Into<Size>>(&self, size: Option<S>) -> Result<()> {
        self.window()?.set_min_inner_size(size.map(Into::into));
        Ok(())
    }

    pub fn set_max_size<S: Into<Size>>(&self, size: Option<S>) -> Result<()> {
        self.window()?.set_max_inner_size(size.map(Into::into));
        Ok(())
    }

    pub fn set_size_constraints(&self, constraints: WindowSizeConstraints) -> Result<()> {
        self.window()?
            .set_inner_size_constraints(tao::window::WindowSizeConstraints {
                min_width: constraints.min_width,
                min_height: constraints.min_height,
                max_width: constraints.max_width,
                max_height: constraints.max_height,
            });
        Ok(())
    }

    pub fn set_position<P: Into<Position>>(&self, position: P) -> Result<()> {
        self.window()?.set_outer_position(position.into());
        Ok(())
    }

    pub fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        self.window()?.set_fullscreen(if fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        });
        Ok(())
    }

    pub fn set_fullscreen_on_monitor(&self, position: Position) -> Result<()> {
        let window = self.window()?;

        if let Some(monitor) = find_monitor_for_position(window.available_monitors(), position) {
            window.set_fullscreen(Some(Fullscreen::Borderless(Some(monitor))));
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn set_simple_fullscreen(&self, enable: bool) -> Result<()> {
        self.window()?.set_simple_fullscreen(enable);
        Ok(())
    }

    pub fn set_focus(&self) -> Result<()> {
        self.window()?.set_focus();
        Ok(())
    }

    pub fn set_focusable(&self, focusable: bool) -> Result<()> {
        self.window()?.set_focusable(focusable);
        Ok(())
    }

    pub fn set_icon(&self, icon: Icon<'_>) -> Result<()> {
        self.window()?.set_window_icon(Some(TaoIcon::try_from(icon)?.0));
        Ok(())
    }

    pub fn clear_icon(&self) -> Result<()> {
        self.window()?.set_window_icon(None);
        Ok(())
    }

    pub fn set_skip_taskbar(&self, skip: bool) -> Result<()> {
        #[cfg(any(
            windows,
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            let _ = self.window()?.set_skip_taskbar(skip);
        }

        #[cfg(not(any(
            windows,
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            let _ = skip;
            self.window()?;
        }

        Ok(())
    }

    pub fn set_cursor_grab(&self, grab_mod: bool) -> Result<()> {
        self.window()?
            .set_cursor_grab(grab_mod)
            .map_err(|_| Error::FailedToSendMessage)
    }

    pub fn set_cursor_visible(&self, visible: bool) -> Result<()> {
        self.window()?.set_cursor_visible(visible);
        Ok(())
    }

    pub fn set_cursor_icon(&self, icon: CursorIcon) -> Result<()> {
        self.window()?.set_cursor_icon(CursorIconWrapper::from(icon).0);
        Ok(())
    }

    pub fn set_cursor_position<P: Into<Position>>(&self, position: P) -> Result<()> {
        self.window()?
            .set_cursor_position(position.into())
            .map_err(|_| Error::FailedToSendMessage)
    }

    pub fn set_ignore_cursor_events(&self, ignore: bool) -> Result<()> {
        self.window()?
            .set_ignore_cursor_events(ignore)
            .map_err(|_| Error::FailedToSendMessage)
    }

    pub fn drag_window(&self) -> Result<()> {
        self.window()?.drag_window().map_err(|_| Error::FailedToSendMessage)
    }

    pub fn resize_drag_window(&self, direction: ResizeDirection) -> Result<()> {
        let direction = match direction {
            ResizeDirection::East => tao::window::ResizeDirection::East,
            ResizeDirection::North => tao::window::ResizeDirection::North,
            ResizeDirection::NorthEast => tao::window::ResizeDirection::NorthEast,
            ResizeDirection::NorthWest => tao::window::ResizeDirection::NorthWest,
            ResizeDirection::South => tao::window::ResizeDirection::South,
            ResizeDirection::SouthEast => tao::window::ResizeDirection::SouthEast,
            ResizeDirection::SouthWest => tao::window::ResizeDirection::SouthWest,
            ResizeDirection::West => tao::window::ResizeDirection::West,
        };

        self.window()?
            .drag_resize_window(direction)
            .map_err(|_| Error::FailedToSendMessage)
    }

    pub fn request_redraw(&self) -> Result<()> {
        self.window()?.request_redraw();
        Ok(())
    }

    pub fn set_badge_count(&self, count: Option<i64>, desktop_filename: Option<String>) -> Result<()> {
        let _window = self.window()?;

        #[cfg(target_os = "ios")]
        _window.set_badge_count(count.map_or(0, |x| x.clamp(i32::MIN as i64, i32::MAX as i64) as i32));

        #[cfg(target_os = "macos")]
        _window.set_badge_label(count.map(|x| x.to_string()));

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        _window.set_badge_count(count, desktop_filename);

        #[cfg(not(any(
            target_os = "ios",
            target_os = "macos",
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            let _ = (count, desktop_filename);
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn set_badge_label(&self, label: Option<String>) -> Result<()> {
        self.window()?.set_badge_label(label);
        Ok(())
    }

    pub fn emit_window_event(&self, tao_event: &tao::event::WindowEvent<'_>) {
        if let Some(event) = WindowEventWrapper::parse(&self, &tao_event).0 {
            if let Some(handler) = &self.on_window_event {
                handler(&self.metadata, &event);
            }
        }
    }
    #[cfg(windows)]
    pub fn set_overlay_icon(&self, icon: Option<Icon<'_>>) -> Result<()> {
        let icon = icon.map(TaoIcon::try_from).transpose()?;
        self.window()?.set_overlay_icon(icon.as_ref().map(|icon| &icon.0));
        Ok(())
    }

    pub fn set_progress_bar(&self, progress: ProgressBarState) -> Result<()> {
        self.window()?
            .set_progress_bar(ProgressBarStateWrapper::from(progress).0);
        Ok(())
    }

    pub fn set_title_bar_style(&self, style: TitleBarStyle) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let window = self.window()?;
            match style {
                TitleBarStyle::Visible => {
                    window.set_titlebar_transparent(false);
                    window.set_fullsize_content_view(true);
                }
                TitleBarStyle::Transparent => {
                    window.set_titlebar_transparent(true);
                    window.set_fullsize_content_view(false);
                }
                TitleBarStyle::Overlay => {
                    window.set_titlebar_transparent(true);
                    window.set_fullsize_content_view(true);
                }
                _ => {}
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = style;
            self.window()?;
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn set_traffic_light_position<P: Into<dpi::Position>>(&self, position: P) -> Result<()> {
        self.window()?.set_traffic_light_inset(position.into());
        Ok(())
    }

    pub fn set_theme(&self, theme: Option<Theme>) -> Result<()> {
        self.window()?.set_theme(theme.map(|theme| match theme {
            Theme::Dark => tao::window::Theme::Dark,
            _ => tao::window::Theme::Light,
        }));
        Ok(())
    }

    pub fn set_background_color(&self, color: Option<Color>) -> Result<()> {
        self.window()?.set_background_color(color.map(Into::into));
        Ok(())
    }

    /// Setzt ein vollständiges [`WindowMenu`] für dieses Fenster und gibt das
    /// zuvor gesetzte Menü zurück.
    ///
    /// Die native Menü-Initialisierung wird auf den unterstützten Plattformen
    /// durchgeführt, bevor das Menü im [`ManagedWindow`] gespeichert wird.
    #[cfg_attr(target_os = "macos", allow(unused_variables))]
    pub fn set_window_menu(&self, window_menu: WindowMenu) -> Result<Option<WindowMenu>> {
        let window = self.window()?;
        let menu = &window_menu.menu;

        #[cfg(windows)]
        {
            use crate::menu::map_from_tao_to_menu_theme;

            let hwnd = window.hwnd();
            let theme = map_from_tao_to_menu_theme(window.theme());

            unsafe {
                menu.init_for_hwnd_with_theme(hwnd as _, theme)?;
            }
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            if let (Ok(gtk_window), Some(gtk_box)) = (window.gtk_window(), window.default_vbox()) {
                menu.inner().init_for_gtk_window(&gtk_window, Some(&gtk_box))?;
            }
        }

        let previous = self.menu.lock().unwrap().replace(window_menu);
        Ok(previous)
    }

    /// Setzt ein normales, fensterspezifisches Menü und gibt das vorherige Menü zurück.
    pub fn set_menu(&self, menu: Menu) -> Result<Option<Menu>> {
        self.set_window_menu(WindowMenu {
            is_app_wide: false,
            menu,
        })
        .map(|previous| previous.map(|window_menu| window_menu.menu))
    }

    /// Entfernt das Menü mitsamt seiner Metadaten und gibt es zurück.
    pub fn remove_window_menu(&self) -> Result<Option<WindowMenu>> {
        let window = self.window()?;
        let previous = self.menu.lock().unwrap().take();

        #[cfg(not(target_os = "macos"))]
        if let Some(window_menu) = &previous {
            let menu = &window_menu.menu;

            #[cfg(windows)]
            {
                let hwnd = window.hwnd();
                unsafe {
                    menu.0.inner.remove_for_hwnd(hwnd as _)?;
                }
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                if let Ok(gtk_window) = window.gtk_window() {
                    menu.remove_for_gtk_window(&gtk_window)?;
                }
            }
        }

        Ok(previous)
    }

    /// Entfernt das aktuelle Menü und gibt nur das eigentliche [`Menu`] zurück.
    pub fn remove_menu(&self) -> Result<Option<Menu>> {
        Ok(self.remove_window_menu()?.map(|window_menu| window_menu.menu))
    }

    /// Entfernt das aktuelle Menü, ohne es zurückzugeben.
    pub fn clear_menu(&self) -> Result<()> {
        self.remove_window_menu()?;
        Ok(())
    }

    /// Gibt zurück, ob dem Fenster aktuell ein Menü zugeordnet ist.
    pub fn has_menu(&self) -> bool {
        self.menu.lock().unwrap().is_some()
    }

    /// Gibt zurück, ob das aktuelle Menü als anwendungsweit markiert ist.
    pub fn has_app_wide_menu(&self) -> bool {
        self.menu.lock().unwrap().as_ref().is_some_and(|menu| menu.is_app_wide)
    }

    /// Prüft, ob die angegebene Menü-ID zum aktuell gesetzten Fenster-Menü gehört.
    pub fn is_menu_in_use<I: PartialEq<MenuId>>(&self, id: &I) -> bool {
        self.menu
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|window_menu| id.eq(window_menu.menu.id()))
    }

    /// Gibt eine Kopie des aktuell gesetzten Menüs zurück.
    pub fn menu(&self) -> Option<Menu> {
        self.menu
            .lock()
            .unwrap()
            .as_ref()
            .map(|window_menu| window_menu.menu.clone())
    }

    /// Gibt ID und `is_app_wide` des aktuellen Menüs zurück.
    pub fn menu_info(&self) -> Option<(MenuId, bool)> {
        self.menu
            .lock()
            .unwrap()
            .as_ref()
            .map(|window_menu| (window_menu.menu.id().clone(), window_menu.is_app_wide))
    }

    /// Blendet das aktuelle Fenster-Menü aus.
    pub fn hide_menu(&self) -> Result<()> {
        let window = self.window()?;

        #[cfg(not(target_os = "macos"))]
        if let Some(window_menu) = self.menu.lock().unwrap().as_ref() {
            let menu = &window_menu.menu;

            #[cfg(windows)]
            {
                let hwnd = window.hwnd();
                unsafe {
                    menu.0.inner.hide_for_hwnd(hwnd as _)?;
                }
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                if let Ok(gtk_window) = window.gtk_window() {
                    menu.inner().hide_for_gtk_window(&gtk_window)?;
                }
            }
        }

        Ok(())
    }

    /// Blendet das aktuelle Fenster-Menü ein.
    pub fn show_menu(&self) -> Result<()> {
        let window = self.window()?;

        #[cfg(not(target_os = "macos"))]
        if let Some(window_menu) = self.menu.lock().unwrap().as_ref() {
            let menu = &window_menu.menu;

            #[cfg(windows)]
            {
                let hwnd = window.hwnd();
                unsafe {
                    menu.0.inner.show_for_hwnd(hwnd as _)?;
                }
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                if let Ok(gtk_window) = window.gtk_window() {
                    menu.inner().show_for_gtk_window(&gtk_window)?;
                }
            }
        }

        Ok(())
    }

    /// Gibt zurück, ob das aktuelle Fenster-Menü sichtbar ist.
    pub fn is_menu_visible(&self) -> Result<bool> {
        let window = self.window()?;

        #[cfg(not(target_os = "macos"))]
        if let Some(window_menu) = self.menu.lock().unwrap().as_ref() {
            let menu = &window_menu.menu;

            #[cfg(windows)]
            {
                let hwnd = window.hwnd();
                return Ok(unsafe { menu.0.inner.is_visible_on_hwnd(hwnd as _) });
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                if let Ok(gtk_window) = window.gtk_window() {
                    return Ok(menu.is_visible_on_gtk_window(&gtk_window));
                }
            }
        }

        Ok(false)
    }

    /// Zeigt das gespeicherte Fenster-Menü als Kontextmenü am Cursor an.
    pub fn popup_window_menu(&self) -> Result<()>
    where
        Menu: crate::prelude::ContextMenu,
    {
        let window = self.window()?;
        let guard = self.menu.lock().unwrap();
        let Some(window_menu) = guard.as_ref() else {
            return Ok(());
        };

        window_menu.menu.popup(window)?;
        Ok(())
    }

    /// Zeigt das gespeicherte Fenster-Menü an einer Position an.
    pub fn popup_window_menu_at<P: Into<Position>>(&self, position: P) -> Result<()>
    where
        Menu: crate::prelude::ContextMenu,
    {
        let window = self.window()?;
        let guard = self.menu.lock().unwrap();
        let Some(window_menu) = guard.as_ref() else {
            return Ok(());
        };

        window_menu.menu.popup_at(window, position)?;
        Ok(())
    }

    /// Zeigt ein beliebiges Kontextmenü an der Cursorposition an.
    pub fn popup_menu<M: crate::prelude::ContextMenu>(&self, menu: &M) -> Result<()> {
        let window = self.window()?;
        menu.popup(window)
    }

    /// Zeigt ein beliebiges Kontextmenü an der angegebenen Position an.
    ///
    /// Die Position ist relativ zur linken oberen Ecke des Fensters.
    pub fn popup_menu_at<M: crate::prelude::ContextMenu, P: Into<Position>>(
        &self,
        menu: &M,
        position: P,
    ) -> Result<()> {
        let window = self.window()?;
        menu.popup_at(window, position)
    }

    #[cfg(windows)]
    pub fn hpopupmenu(&self) -> Result<Option<isize>> {
        let guard = self.menu.lock().unwrap();

        match guard.as_ref() {
            Some(window_menu) => Ok(Some(window_menu.menu.hpopupmenu()?)),
            None => Ok(None),
        }
    }
}
