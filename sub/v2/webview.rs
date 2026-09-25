use dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use std::{ops::Deref, path::PathBuf, rc::Rc};
use tao::window::Window;
use url::Url;
use wry::{WebView, cookie::Cookie};

use crate::{
    types::{WebContextStore, WebviewBounds},
    utils::{ArcMut, WindowWebViewMetaData},
};

#[derive(Clone)]
pub struct ManagedWebview {
    pub(crate) metadata: WindowWebViewMetaData,
    pub inner: Rc<WebView>,
    pub context_store: WebContextStore,
    pub context_key: Option<PathBuf>,
    pub bounds: ArcMut<Option<WebviewBounds>>,
}

impl Deref for ManagedWebview {
    type Target = WebView;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Drop for ManagedWebview {
    fn drop(&mut self) {
        if Rc::get_mut(&mut self.inner).is_some() {
            let mut context_store = self.context_store.lock().unwrap();

            if let Some(web_context) = context_store.get_mut(&self.context_key)
            {
                web_context
                    .referenced_by_webviews
                    .remove(&self.metadata.webview_label);

                // https://github.com/tauri-apps/tauri/issues/14626
                // Because WebKit does not close its network process even when no webviews are running,
                // we need to ensure to re-use the existing process on Linux by keeping the WebContext
                // alive for the lifetime of the app.
                // WebKit on macOS handles this itself.
                #[cfg(not(any(
                    target_os = "linux",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd"
                )))]
                if web_context.referenced_by_webviews.is_empty() {
                    context_store.remove(&self.context_key);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct WebViewId(u32);

impl From<u32> for WebViewId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl ManagedWebview {
    // -------------------------------------------------------------------------
    // Commands
    // -------------------------------------------------------------------------

    pub fn evaluate_script(&self, script: &str) -> crate::Result<()> {
        self.inner
            .evaluate_script(script)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn evaluate_script_with_callback<F>(
        &self,
        script: &str,
        callback: F,
    ) -> crate::Result<()>
    where
        F: Fn(String) + Send + 'static,
    {
        self.inner
            .evaluate_script_with_callback(script, callback)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn navigate(&self, url: &Url) -> crate::Result<()> {
        self.inner
            .load_url(url.as_str())
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn reload(&self) -> crate::Result<()> {
        self.inner
            .reload()
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn show(&self) -> crate::Result<()> {
        self.inner
            .set_visible(true)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn hide(&self) -> crate::Result<()> {
        self.inner
            .set_visible(false)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn print(&self) -> crate::Result<()> {
        self.inner
            .print()
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    // -------------------------------------------------------------------------
    // Bounds
    // -------------------------------------------------------------------------

    pub fn set_bounds(
        &self,
        window: &Window,
        bounds: crate::types::Rect,
    ) -> crate::Result<()> {
        let bounds: crate::wrappers::RectWrapper = bounds.into();
        let bounds = bounds.0;

        if let Some(b) = &mut *self.bounds.lock().unwrap() {
            let scale_factor = window.scale_factor();

            let size = bounds.size.to_logical::<f32>(scale_factor);

            let position = bounds.position.to_logical::<f32>(scale_factor);

            let window_size =
                window.inner_size().to_logical::<f32>(scale_factor);

            b.width_rate = size.width / window_size.width;

            b.height_rate = size.height / window_size.height;

            b.x_rate = position.x / window_size.width;

            b.y_rate = position.y / window_size.height;
        }

        self.inner
            .set_bounds(bounds)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn set_size(&self, window: &Window, size: Size) -> crate::Result<()> {
        let mut bounds = self
            .inner
            .bounds()
            .map_err(|_| crate::Error::FailedToSendMessage)?;

        bounds.size = size;

        let scale_factor = window.scale_factor();
        let size = size.to_logical::<f32>(scale_factor);

        if let Some(b) = &mut *self.bounds.lock().unwrap() {
            let window_size =
                window.inner_size().to_logical::<f32>(scale_factor);

            b.width_rate = size.width / window_size.width;

            b.height_rate = size.height / window_size.height;
        }

        self.inner
            .set_bounds(bounds)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn set_position(
        &self,
        window: &Window,
        position: Position,
    ) -> crate::Result<()> {
        let mut bounds = self
            .inner
            .bounds()
            .map_err(|_| crate::Error::FailedToSendMessage)?;

        bounds.position = position;

        let scale_factor = window.scale_factor();

        let position = position.to_logical::<f32>(scale_factor);

        if let Some(b) = &mut *self.bounds.lock().unwrap() {
            let window_size =
                window.inner_size().to_logical::<f32>(scale_factor);

            b.x_rate = position.x / window_size.width;

            b.y_rate = position.y / window_size.height;
        }

        self.inner
            .set_bounds(bounds)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    // -------------------------------------------------------------------------
    // Appearance / state
    // -------------------------------------------------------------------------

    pub fn set_zoom(&self, scale_factor: f64) -> crate::Result<()> {
        self.inner
            .zoom(scale_factor)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn set_background_color(
        &self,
        color: Option<crate::types::Color>,
    ) -> crate::Result<()> {
        self.inner
            .set_background_color(
                color.map(Into::into).unwrap_or((255, 255, 255, 255)),
            )
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn clear_all_browsing_data(&self) -> crate::Result<()> {
        self.inner
            .clear_all_browsing_data()
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    // -------------------------------------------------------------------------
    // Getters
    // -------------------------------------------------------------------------

    pub fn url(&self) -> crate::Result<Url> {
        self.inner
            .url()
            .map(|url| url.parse().expect("invalid webview URL"))
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn bounds(&self) -> crate::Result<crate::types::Rect> {
        self.inner
            .bounds()
            .map(|bounds| crate::types::Rect {
                size: bounds.size,
                position: bounds.position,
            })
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn position(
        &self,
        window: &Window,
    ) -> crate::Result<PhysicalPosition<i32>> {
        self.inner
            .bounds()
            .map(|bounds| bounds.position.to_physical(window.scale_factor()))
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn size(&self, window: &Window) -> crate::Result<PhysicalSize<u32>> {
        self.inner
            .bounds()
            .map(|bounds| bounds.size.to_physical(window.scale_factor()))
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    // -------------------------------------------------------------------------
    // Focus / autoresize
    // -------------------------------------------------------------------------

    pub fn set_focus(&self) -> crate::Result<()> {
        self.inner
            .focus()
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn set_auto_resize(
        &self,
        window: &Window,
        auto_resize: bool,
    ) -> crate::Result<()> {
        let bounds = self
            .inner
            .bounds()
            .map_err(|_| crate::Error::FailedToSendMessage)?;

        let scale_factor = window.scale_factor();

        let window_size = window.inner_size().to_logical::<f32>(scale_factor);

        *self.bounds.lock().unwrap() = if auto_resize {
            let size = bounds.size.to_logical::<f32>(scale_factor);

            let position = bounds.position.to_logical::<f32>(scale_factor);

            Some(WebviewBounds {
                x_rate: position.x / window_size.width,

                y_rate: position.y / window_size.height,

                width_rate: size.width / window_size.width,

                height_rate: size.height / window_size.height,
            })
        } else {
            None
        };

        Ok(())
    }

    // -------------------------------------------------------------------------
    // DevTools
    // -------------------------------------------------------------------------

    #[cfg(any(debug_assertions, feature = "devtools"))]
    pub fn open_devtools(&self) {
        self.inner.open_devtools();
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    pub fn close_devtools(&self) {
        self.inner.close_devtools();
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    pub fn is_devtools_open(&self) -> bool {
        self.inner.is_devtools_open()
    }
    pub fn cookies(&self) -> crate::Result<Vec<Cookie<'static>>> {
        self.inner
            .cookies()
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn set_cookie(&self, cookie: &Cookie<'_>) -> crate::Result<()> {
        self.inner
            .set_cookie(cookie)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn delete_cookie(&self, cookie: &Cookie<'_>) -> crate::Result<()> {
        self.inner
            .delete_cookie(cookie)
            .map_err(|_| crate::Error::FailedToSendMessage)
    }

    pub fn cookies_for_url(
        &self,
        url: &Url,
    ) -> crate::Result<Vec<Cookie<'static>>> {
        self.inner
            .cookies_for_url(url.as_str())
            .map_err(|_| crate::Error::FailedToSendMessage)
    }
}

/*


_cookie

commands dispatcher system

[

fn make_event_handler<T: UserEvent, F: FnMut(RunEvent<T>) + 'static>(
  context: Context<T>,
  mut callback: F,
) -> impl FnMut(Event<'_, Message<T>>, &EventLoopWindowTarget<Message<T>>, &mut ControlFlow) {
  let windows = context.main_thread.windows;
  let window_id_map = context.window_id_map;
  let web_context = context.main_thread.web_context;
  let plugins = context.plugins;

  #[cfg(feature = "tracing")]
  let active_tracing_spans = context.main_thread.active_tracing_spans;
  let proxy = context.proxy;

  move |event, event_loop, control_flow| {
    for p in plugins.lock().unwrap().iter_mut() {
      let prevent_default = p.on_event(
        &event,
        event_loop,
        &proxy,
        control_flow,
        EventLoopIterationContext {
          callback: &mut callback,
          window_id_map: &window_id_map,
          windows: &windows,
          #[cfg(feature = "tracing")]
          active_tracing_spans: &active_tracing_spans,
        },
        &web_context,
      );
      if prevent_default {
        return;
      }
    }
    handle_event_loop(
      event,
      event_loop,
      control_flow,
      EventLoopIterationContext {
        callback: &mut callback,
        window_id_map: &window_id_map,
        windows: &windows,
        #[cfg(feature = "tracing")]
        active_tracing_spans: &active_tracing_spans,
      },
    );
  }
}

pub struct EventLoopIterationContext<'a, T: UserEvent> {
  pub callback: &'a mut (dyn FnMut(RunEvent<T>) + 'static),
  pub window_id_map: &'a WindowIdStore,
  pub windows: &'a WindowsStore,
  #[cfg(feature = "tracing")]
  pub active_tracing_spans: &'a ActiveTraceSpanStore,
}

struct UserMessageContext<'a> {
  windows: &'a WindowsStore,
  window_id_map: &'a WindowIdStore,
}

fn handle_user_message<T: UserEvent>(
  event_loop: &EventLoopWindowTarget<Message<T>>,
  message: Message<T>,
  context: UserMessageContext,
) {
  let UserMessageContext {
    window_id_map,
    windows,
  } = context;
  match message {
    Message::Task(task) => task(),
    #[cfg(target_os = "macos")]
    Message::SetActivationPolicy(activation_policy) => {
      event_loop.set_activation_policy_at_runtime(tao_activation_policy(activation_policy))
    }
    #[cfg(target_os = "macos")]
    Message::SetDockVisibility(visible) => event_loop.set_dock_visibility(visible),
    Message::RequestExit(_code) => panic!("cannot handle RequestExit on the main thread"),
    Message::Application(application_message) => match application_message {
      #[cfg(target_os = "macos")]
      ApplicationMessage::Show => {
        event_loop.show_application();
      }
      #[cfg(target_os = "macos")]
      ApplicationMessage::Hide => {
        event_loop.hide_application();
      }
      #[cfg(any(target_os = "macos", target_os = "ios"))]
      ApplicationMessage::FetchDataStoreIdentifiers(cb) => {
        if let Err(e) = WebView::fetch_data_store_identifiers(cb) {
          // this shouldn't ever happen because we're running on the main thread
          // but let's be safe and warn here
          log::error!("failed to fetch data store identifiers: {e}");
        }
      }
      #[cfg(any(target_os = "macos", target_os = "ios"))]
      ApplicationMessage::RemoveDataStore(uuid, cb) => {
        WebView::remove_data_store(&uuid, move |res| {
          cb(res.map_err(|_| Error::FailedToRemoveDataStore))
        })
      }
    },
    Message::Window(id, window_message) => {
      let w = windows.0.borrow().get(&id).map(|w| {
        #[cfg(windows)]
        let focused_webview = w.focused_webview.clone();
        #[cfg(not(windows))]
        let focused_webview = ();
        (
          w.inner.clone(),
          w.webviews.clone(),
          w.has_children.load(Ordering::Relaxed),
          w.window_event_listeners.clone(),
          focused_webview,
        )
      });
      if let Some((
        Some(window),
        webviews,
        has_children,
        window_event_listeners,
        _focused_webview,
      )) = w
      {
        match window_message {
          WindowMessage::AddEventListener(id, listener) => {
            window_event_listeners.lock().unwrap().insert(id, listener);
          }

          // Getters
          WindowMessage::ScaleFactor(tx) => tx.send(window.scale_factor()).unwrap(),
          WindowMessage::InnerPosition(tx) => tx
            .send(
              window
                .inner_position()
                .map_err(|_| Error::FailedToSendMessage),
            )
            .unwrap(),
          WindowMessage::OuterPosition(tx) => tx
            .send(
              window
                .outer_position()
                .map_err(|_| Error::FailedToSendMessage),
            )
            .unwrap(),
          WindowMessage::InnerSize(tx) => tx
            .send(inner_size(&window, &webviews, has_children))
            .unwrap(),
          WindowMessage::OuterSize(tx) => tx.send(window.outer_size()).unwrap(),
          WindowMessage::IsFullscreen(tx) => tx.send(window.fullscreen().is_some()).unwrap(),
          WindowMessage::IsMinimized(tx) => tx.send(window.is_minimized()).unwrap(),
          WindowMessage::IsMaximized(tx) => tx.send(window.is_maximized()).unwrap(),
          #[cfg(not(windows))]
          WindowMessage::IsFocused(tx) => tx.send(window.is_focused()).unwrap(),
          #[cfg(windows)]
          WindowMessage::IsFocused(tx) => {
            let focused = if has_children {
              // on multiwebview mode, get the focused state from cache,
              // as the window might not have direct focus
              matches!(
                *_focused_webview.lock().unwrap(),
                FocusState::WindowFocused | FocusState::WebviewFocused { .. }
              )
            } else {
              window.is_focused()
            };
            tx.send(focused).unwrap()
          }
          WindowMessage::IsDecorated(tx) => tx.send(window.is_decorated()).unwrap(),
          WindowMessage::IsResizable(tx) => tx.send(window.is_resizable()).unwrap(),
          WindowMessage::IsMaximizable(tx) => tx.send(window.is_maximizable()).unwrap(),
          WindowMessage::IsMinimizable(tx) => tx.send(window.is_minimizable()).unwrap(),
          WindowMessage::IsClosable(tx) => tx.send(window.is_closable()).unwrap(),
          WindowMessage::IsVisible(tx) => tx.send(window.is_visible()).unwrap(),
          WindowMessage::Title(tx) => tx.send(window.title()).unwrap(),
          WindowMessage::CurrentMonitor(tx) => tx
            .send(
              window
                .current_monitor()
                .map(|m| MonitorHandleWrapper(m).into()),
            )
            .unwrap(),
          WindowMessage::PrimaryMonitor(tx) => tx
            .send(
              window
                .primary_monitor()
                .map(|m| MonitorHandleWrapper(m).into()),
            )
            .unwrap(),
          WindowMessage::MonitorFromPoint(tx, (x, y)) => tx
            .send(
              window
                .monitor_from_point(x, y)
                .map(|m| MonitorHandleWrapper(m).into()),
            )
            .unwrap(),
          WindowMessage::AvailableMonitors(tx) => tx
            .send(
              window
                .available_monitors()
                .map(|m| MonitorHandleWrapper(m).into())
                .collect(),
            )
            .unwrap(),
          #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
          ))]
          WindowMessage::GtkWindow(tx) => tx.send(GtkWindow(window.gtk_window().clone())).unwrap(),
          #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
          ))]
          WindowMessage::GtkBox(tx) => tx
            .send(GtkBox(window.default_vbox().unwrap().clone()))
            .unwrap(),
          #[cfg(target_os = "android")]
          WindowMessage::ActivityName(tx) => {
            tx.send(window.activity_name()).unwrap();
          }
          #[cfg(target_os = "ios")]
          WindowMessage::SceneIdentifier(tx) => {
            tx.send(window.scene_identifier()).unwrap();
          }
          WindowMessage::RawWindowHandle(tx) => tx
            .send(
              window
                .window_handle()
                .map(|h| SendRawWindowHandle(h.as_raw())),
            )
            .unwrap(),
          WindowMessage::Theme(tx) => {
            tx.send(map_theme(&window.theme())).unwrap();
          }
          WindowMessage::IsEnabled(tx) => tx.send(window.is_enabled()).unwrap(),
          WindowMessage::IsAlwaysOnTop(tx) => tx.send(window.is_always_on_top()).unwrap(),
          // Setters
          WindowMessage::Center => window.center(),
          WindowMessage::RequestUserAttention(request_type) => {
            window.request_user_attention(request_type.map(|r| r.0));
          }
          WindowMessage::SetResizable(resizable) => {
            window.set_resizable(resizable);
            #[cfg(windows)]
            if !resizable {
              undecorated_resizing::detach_resize_handler(window.hwnd());
            } else if !window.is_decorated() {
              undecorated_resizing::attach_resize_handler(
                window.hwnd(),
                window.has_undecorated_shadow(),
              );
            }
          }
          WindowMessage::SetMaximizable(maximizable) => window.set_maximizable(maximizable),
          WindowMessage::SetMinimizable(minimizable) => window.set_minimizable(minimizable),
          WindowMessage::SetClosable(closable) => window.set_closable(closable),
          WindowMessage::SetTitle(title) => window.set_title(&title),
          WindowMessage::Maximize => window.set_maximized(true),
          WindowMessage::Unmaximize => window.set_maximized(false),
          WindowMessage::Minimize => window.set_minimized(true),
          WindowMessage::Unminimize => window.set_minimized(false),
          WindowMessage::SetEnabled(enabled) => window.set_enabled(enabled),
          WindowMessage::Show => window.set_visible(true),
          WindowMessage::Hide => window.set_visible(false),
          WindowMessage::Close => {
            panic!("cannot handle `WindowMessage::Close` on the main thread")
          }
          WindowMessage::Destroy => {
            panic!("cannot handle `WindowMessage::Destroy` on the main thread")
          }
          WindowMessage::SetDecorations(decorations) => {
            window.set_decorations(decorations);
            #[cfg(windows)]
            if decorations {
              undecorated_resizing::detach_resize_handler(window.hwnd());
            } else if window.is_resizable() {
              undecorated_resizing::attach_resize_handler(
                window.hwnd(),
                window.has_undecorated_shadow(),
              );
            }
          }
          WindowMessage::SetShadow(_enable) => {
            #[cfg(windows)]
            {
              window.set_undecorated_shadow(_enable);
              undecorated_resizing::update_drag_hwnd_rgn_for_undecorated(window.hwnd(), _enable);
            }
            #[cfg(target_os = "macos")]
            window.set_has_shadow(_enable);
          }
          WindowMessage::SetAlwaysOnBottom(always_on_bottom) => {
            window.set_always_on_bottom(always_on_bottom)
          }
          WindowMessage::SetAlwaysOnTop(always_on_top) => window.set_always_on_top(always_on_top),
          WindowMessage::SetVisibleOnAllWorkspaces(visible_on_all_workspaces) => {
            window.set_visible_on_all_workspaces(visible_on_all_workspaces)
          }
          WindowMessage::SetContentProtected(protected) => window.set_content_protection(protected),
          WindowMessage::SetSize(size) => {
            window.set_inner_size(size);
          }
          WindowMessage::SetMinSize(size) => {
            window.set_min_inner_size(size);
          }
          WindowMessage::SetMaxSize(size) => {
            window.set_max_inner_size(size);
          }
          WindowMessage::SetSizeConstraints(constraints) => {
            window.set_inner_size_constraints(tao::window::WindowSizeConstraints {
              min_width: constraints.min_width,
              min_height: constraints.min_height,
              max_width: constraints.max_width,
              max_height: constraints.max_height,
            });
          }
          WindowMessage::SetPosition(position) => window.set_outer_position(position),
          WindowMessage::SetFullscreen(fullscreen) => {
            if fullscreen {
              window.set_fullscreen(Some(Fullscreen::Borderless(None)))
            } else {
              window.set_fullscreen(None)
            }
          }
          WindowMessage::SetFullscreenOnMonitor(position) => {
            // Not `Window::monitor_from_point`: on macOS and Linux (GTK) it takes logical
            // coordinates, while callers pass physical ones (e.g. `Monitor::position`).
            if let Some(monitor) =
              find_monitor_for_position(window.available_monitors(), position.into())
            {
              window.set_fullscreen(Some(Fullscreen::Borderless(Some(monitor))))
            }
          }

          #[cfg(target_os = "macos")]
          WindowMessage::SetSimpleFullscreen(enable) => {
            window.set_simple_fullscreen(enable);
          }

          WindowMessage::SetFocus => {
            window.set_focus();
          }
          WindowMessage::SetFocusable(focusable) => {
            window.set_focusable(focusable);
          }
          WindowMessage::SetIcon(icon) => {
            window.set_window_icon(Some(icon));
          }
          #[allow(unused_variables)]
          WindowMessage::SetSkipTaskbar(skip) => {
            #[cfg(any(
              windows,
              target_os = "linux",
              target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "netbsd",
              target_os = "openbsd"
            ))]
            let _ = window.set_skip_taskbar(skip);
          }
          WindowMessage::SetCursorGrab(grab) => {
            let _ = window.set_cursor_grab(grab);
          }
          WindowMessage::SetCursorVisible(visible) => {
            window.set_cursor_visible(visible);
          }
          WindowMessage::SetCursorIcon(icon) => {
            window.set_cursor_icon(CursorIconWrapper::from(icon).0);
          }
          WindowMessage::SetCursorPosition(position) => {
            let _ = window.set_cursor_position(position);
          }
          WindowMessage::SetIgnoreCursorEvents(ignore) => {
            let _ = window.set_ignore_cursor_events(ignore);
          }
          WindowMessage::DragWindow => {
            let _ = window.drag_window();
          }
          WindowMessage::ResizeDragWindow(direction) => {
            let _ = window.drag_resize_window(match direction {
              tauri_runtime::ResizeDirection::East => tao::window::ResizeDirection::East,
              tauri_runtime::ResizeDirection::North => tao::window::ResizeDirection::North,
              tauri_runtime::ResizeDirection::NorthEast => tao::window::ResizeDirection::NorthEast,
              tauri_runtime::ResizeDirection::NorthWest => tao::window::ResizeDirection::NorthWest,
              tauri_runtime::ResizeDirection::South => tao::window::ResizeDirection::South,
              tauri_runtime::ResizeDirection::SouthEast => tao::window::ResizeDirection::SouthEast,
              tauri_runtime::ResizeDirection::SouthWest => tao::window::ResizeDirection::SouthWest,
              tauri_runtime::ResizeDirection::West => tao::window::ResizeDirection::West,
            });
          }
          WindowMessage::RequestRedraw => {
            window.request_redraw();
          }
          WindowMessage::SetBadgeCount(_count, _desktop_filename) => {
            #[cfg(target_os = "ios")]
            window.set_badge_count(
              _count.map_or(0, |x| x.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
            );

            #[cfg(target_os = "macos")]
            window.set_badge_label(_count.map(|x| x.to_string()));

            #[cfg(any(
              target_os = "linux",
              target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "netbsd",
              target_os = "openbsd"
            ))]
            window.set_badge_count(_count, _desktop_filename);
          }
          WindowMessage::SetBadgeLabel(_label) => {
            #[cfg(target_os = "macos")]
            window.set_badge_label(_label);
          }
          WindowMessage::SetOverlayIcon(_icon) => {
            #[cfg(windows)]
            window.set_overlay_icon(_icon.map(|x| x.0).as_ref());
          }
          WindowMessage::SetProgressBar(progress_state) => {
            window.set_progress_bar(ProgressBarStateWrapper::from(progress_state).0);
          }
          WindowMessage::SetTitleBarStyle(_style) => {
            #[cfg(target_os = "macos")]
            match _style {
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
              unknown => {
                #[cfg(feature = "tracing")]
                tracing::warn!("unknown title bar style applied: {unknown}");

                #[cfg(not(feature = "tracing"))]
                eprintln!("unknown title bar style applied: {unknown}");
              }
            };
          }
          WindowMessage::SetTrafficLightPosition(_position) => {
            #[cfg(target_os = "macos")]
            window.set_traffic_light_inset(_position);
          }
          WindowMessage::SetTheme(theme) => {
            window.set_theme(to_tao_theme(theme));
          }
          WindowMessage::SetBackgroundColor(color) => {
            window.set_background_color(color.map(Into::into))
          }
        }
      }
    }
    Message::Webview(window_id, webview_id, webview_message) => {
      #[cfg(any(
        target_os = "macos",
        windows,
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
      ))]
      if let WebviewMessage::Reparent(new_parent_window_id, tx) = webview_message {
        let webview_handle = windows.0.borrow_mut().get_mut(&window_id).and_then(|w| {
          w.webviews
            .iter()
            .position(|w| w.id == webview_id)
            .map(|webview_index| w.webviews.remove(webview_index))
        });

        if let Some(webview) = webview_handle {
          if let Some((Some(new_parent_window), new_parent_window_webviews)) = windows
            .0
            .borrow_mut()
            .get_mut(&new_parent_window_id)
            .map(|w| (w.inner.clone(), &mut w.webviews))
          {
            #[cfg(target_os = "macos")]
            let reparent_result = {
              use wry::WebViewExtMacOS;
              webview.inner.reparent(new_parent_window.ns_window() as _)
            };
            #[cfg(windows)]
            let reparent_result = { webview.inner.reparent(new_parent_window.hwnd()) };

            #[cfg(any(
              target_os = "linux",
              target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "netbsd",
              target_os = "openbsd"
            ))]
            let reparent_result = {
              if let Some(container) = new_parent_window.default_vbox() {
                webview.inner.reparent(container)
              } else {
                Err(wry::Error::MessageSender)
              }
            };

            match reparent_result {
              Ok(_) => {
                new_parent_window_webviews.push(webview);
                tx.send(Ok(())).unwrap();
              }
              Err(e) => {
                log::error!("failed to reparent webview: {e}");
                tx.send(Err(Error::FailedToSendMessage)).unwrap();
              }
            }
          }
        } else {
          tx.send(Err(Error::FailedToSendMessage)).unwrap();
        }

        return;
      }

      let webview_handle = windows.0.borrow().get(&window_id).map(|w| {
        (
          w.inner.clone(),
          w.webviews.iter().find(|w| w.id == webview_id).cloned(),
        )
      });
      if let Some((Some(window), Some(webview))) = webview_handle {
        match webview_message {
          WebviewMessage::WebviewEvent(_) => { /* already handled */ }
          WebviewMessage::SynthesizedWindowEvent(_) => { /* already handled */ }
          WebviewMessage::Reparent(_window_id, _tx) => { /* already handled */ }
          WebviewMessage::AddEventListener(id, listener) => {
            webview
              .webview_event_listeners
              .lock()
              .unwrap()
              .insert(id, listener);
          }

          #[cfg(all(feature = "tracing", not(target_os = "android")))]
          WebviewMessage::EvaluateScript(script, tx, span) => {
            let _span = span.entered();
            if let Err(e) = webview.evaluate_script(&script) {
              log::error!("{e}");
            }
            tx.send(()).unwrap();
          }
          #[cfg(not(all(feature = "tracing", not(target_os = "android"))))]
          WebviewMessage::EvaluateScript(script) => {
            if let Err(e) = webview.evaluate_script(&script) {
              log::error!("{e}");
            }
          }
          #[cfg(all(feature = "tracing", not(target_os = "android")))]
          WebviewMessage::EvaluateScriptWithCallback(script, callback, tx, span) => {
            let _span = span.entered();
            if let Err(e) = webview.evaluate_script_with_callback(&script, callback) {
              log::error!("{e}");
            }
            tx.send(()).unwrap();
          }
          #[cfg(not(all(feature = "tracing", not(target_os = "android"))))]
          WebviewMessage::EvaluateScriptWithCallback(script, callback) => {
            if let Err(e) = webview.evaluate_script_with_callback(&script, callback) {
              log::error!("{e}");
            }
          }
          WebviewMessage::Navigate(url) => {
            if let Err(e) = webview.load_url(url.as_str()) {
              log::error!("failed to navigate to url {}: {}", url, e);
            }
          }
          WebviewMessage::Reload => {
            if let Err(e) = webview.reload() {
              log::error!("failed to reload: {e}");
            }
          }
          WebviewMessage::Show => {
            if let Err(e) = webview.set_visible(true) {
              log::error!("failed to change webview visibility: {e}");
            }
          }
          WebviewMessage::Hide => {
            if let Err(e) = webview.set_visible(false) {
              log::error!("failed to change webview visibility: {e}");
            }
          }
          WebviewMessage::Print => {
            let _ = webview.print();
          }
          WebviewMessage::Close => {
            #[allow(unknown_lints, clippy::manual_inspect)]
            windows.0.borrow_mut().get_mut(&window_id).map(|window| {
              if let Some(i) = window.webviews.iter().position(|w| w.id == webview.id) {
                window.webviews.remove(i);
              }
              window
            });
          }
          WebviewMessage::SetBounds(bounds) => {
            let bounds: RectWrapper = bounds.into();
            let bounds = bounds.0;

            if let Some(b) = &mut *webview.bounds.lock().unwrap() {
              let scale_factor = window.scale_factor();
              let size = bounds.size.to_logical::<f32>(scale_factor);
              let position = bounds.position.to_logical::<f32>(scale_factor);
              let window_size = window.inner_size().to_logical::<f32>(scale_factor);
              b.width_rate = size.width / window_size.width;
              b.height_rate = size.height / window_size.height;
              b.x_rate = position.x / window_size.width;
              b.y_rate = position.y / window_size.height;
            }

            if let Err(e) = webview.set_bounds(bounds) {
              log::error!("failed to set webview size: {e}");
            }
          }
          WebviewMessage::SetSize(size) => match webview.bounds() {
            Ok(mut bounds) => {
              bounds.size = size;

              let scale_factor = window.scale_factor();
              let size = size.to_logical::<f32>(scale_factor);

              if let Some(b) = &mut *webview.bounds.lock().unwrap() {
                let window_size = window.inner_size().to_logical::<f32>(scale_factor);
                b.width_rate = size.width / window_size.width;
                b.height_rate = size.height / window_size.height;
              }

              if let Err(e) = webview.set_bounds(bounds) {
                log::error!("failed to set webview size: {e}");
              }
            }
            Err(e) => {
              log::error!("failed to get webview bounds: {e}");
            }
          },
          WebviewMessage::SetPosition(position) => match webview.bounds() {
            Ok(mut bounds) => {
              bounds.position = position;

              let scale_factor = window.scale_factor();
              let position = position.to_logical::<f32>(scale_factor);

              if let Some(b) = &mut *webview.bounds.lock().unwrap() {
                let window_size = window.inner_size().to_logical::<f32>(scale_factor);
                b.x_rate = position.x / window_size.width;
                b.y_rate = position.y / window_size.height;
              }

              if let Err(e) = webview.set_bounds(bounds) {
                log::error!("failed to set webview position: {e}");
              }
            }
            Err(e) => {
              log::error!("failed to get webview bounds: {e}");
            }
          },
          WebviewMessage::SetZoom(scale_factor) => {
            if let Err(e) = webview.zoom(scale_factor) {
              log::error!("failed to set webview zoom: {e}");
            }
          }
          WebviewMessage::SetBackgroundColor(color) => {
            if let Err(e) =
              webview.set_background_color(color.map(Into::into).unwrap_or((255, 255, 255, 255)))
            {
              log::error!("failed to set webview background color: {e}");
            }
          }
          WebviewMessage::ClearAllBrowsingData => {
            if let Err(e) = webview.clear_all_browsing_data() {
              log::error!("failed to clear webview browsing data: {e}");
            }
          }
          // Getters
          WebviewMessage::Url(tx) => {
            tx.send(
              webview
                .url()
                .map(|u| u.parse().expect("invalid webview URL"))
                .map_err(|_| Error::FailedToSendMessage),
            )
            .unwrap();
          }

          WebviewMessage::Cookies(tx) => {
            tx.send(webview.cookies().map_err(|_| Error::FailedToSendMessage))
              .unwrap();
          }

          WebviewMessage::SetCookie(cookie) => {
            if let Err(e) = webview.set_cookie(&cookie) {
              log::error!("failed to set webview cookie: {e}");
            }
          }

          WebviewMessage::DeleteCookie(cookie) => {
            if let Err(e) = webview.delete_cookie(&cookie) {
              log::error!("failed to delete webview cookie: {e}");
            }
          }

          WebviewMessage::CookiesForUrl(url, tx) => {
            let webview_cookies = webview
              .cookies_for_url(url.as_str())
              .map_err(|_| Error::FailedToSendMessage);
            tx.send(webview_cookies).unwrap();
          }

          WebviewMessage::Bounds(tx) => {
            tx.send(
              webview
                .bounds()
                .map(|bounds| tauri_runtime::dpi::Rect {
                  size: bounds.size,
                  position: bounds.position,
                })
                .map_err(|_| Error::FailedToSendMessage),
            )
            .unwrap();
          }
          WebviewMessage::Position(tx) => {
            tx.send(
              webview
                .bounds()
                .map(|bounds| bounds.position.to_physical(window.scale_factor()))
                .map_err(|_| Error::FailedToSendMessage),
            )
            .unwrap();
          }
          WebviewMessage::Size(tx) => {
            tx.send(
              webview
                .bounds()
                .map(|bounds| bounds.size.to_physical(window.scale_factor()))
                .map_err(|_| Error::FailedToSendMessage),
            )
            .unwrap();
          }
          WebviewMessage::SetFocus => {
            if let Err(e) = webview.focus() {
              log::error!("failed to focus webview: {e}");
            }
          }
          WebviewMessage::SetAutoResize(auto_resize) => match webview.bounds() {
            Ok(bounds) => {
              let scale_factor = window.scale_factor();
              let window_size = window.inner_size().to_logical::<f32>(scale_factor);
              *webview.bounds.lock().unwrap() = if auto_resize {
                let size = bounds.size.to_logical::<f32>(scale_factor);
                let position = bounds.position.to_logical::<f32>(scale_factor);
                Some(WebviewBounds {
                  x_rate: position.x / window_size.width,
                  y_rate: position.y / window_size.height,
                  width_rate: size.width / window_size.width,
                  height_rate: size.height / window_size.height,
                })
              } else {
                None
              };
            }
            Err(e) => {
              log::error!("failed to get webview bounds: {e}");
            }
          },
          WebviewMessage::WithWebview(f) => {
            #[cfg(any(
              target_os = "linux",
              target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "netbsd",
              target_os = "openbsd"
            ))]
            {
              f(webview.webview());
            }
            #[cfg(target_os = "macos")]
            {
              use wry::WebViewExtMacOS;
              let platform_webview = webview.webview();
              let manager = webview.manager();
              let ns_window = webview.ns_window();
              f(Webview {
                webview: Retained::as_ptr(&platform_webview).cast_mut() as *mut std::ffi::c_void,
                manager: Retained::as_ptr(&manager).cast_mut() as *mut std::ffi::c_void,
                ns_window: Retained::as_ptr(&ns_window).cast_mut() as *mut std::ffi::c_void,
              });
            }
            #[cfg(target_os = "ios")]
            {
              use wry::WebViewExtIOS;
              let platform_webview = webview.inner.webview();
              let manager = webview.inner.manager();

              f(Webview {
                webview: Retained::as_ptr(&platform_webview).cast_mut() as *mut std::ffi::c_void,
                manager: Retained::as_ptr(&manager).cast_mut() as *mut std::ffi::c_void,
                view_controller: window.ui_view_controller(),
              });
            }
            #[cfg(windows)]
            {
              f(Webview {
                controller: webview.controller(),
                environment: webview.environment(),
              });
            }
            #[cfg(target_os = "android")]
            {
              f(webview.handle())
            }
          }
          #[cfg(any(debug_assertions, feature = "devtools"))]
          WebviewMessage::OpenDevTools => {
            webview.open_devtools();
          }
          #[cfg(any(debug_assertions, feature = "devtools"))]
          WebviewMessage::CloseDevTools => {
            webview.close_devtools();
          }
          #[cfg(any(debug_assertions, feature = "devtools"))]
          WebviewMessage::IsDevToolsOpen(tx) => {
            tx.send(webview.is_devtools_open()).unwrap();
          }
        }
      }
    }
    Message::CreateWebview(window_id, handler, sender) => {
      let window = windows.0.borrow().get(&window_id).map(|w| {
        (
          w.inner.clone(),
          CreateWebviewOptions {
            #[cfg(windows)]
            focused_webview: w.focused_webview.clone(),
          },
        )
      });
      if let Some((Some(window), options)) = window {
        match handler(&window, options) {
          Ok(webview) => {
            if let Some(w) = windows.0.borrow_mut().get_mut(&window_id) {
              w.webviews.push(webview);
              w.has_children.store(true, Ordering::Relaxed);
            }
            // SAFETY: The caller calls blocking `rx.recv()` so the receiver will never be dropped before this
            sender.send(Ok(())).unwrap();
          }
          Err(e) => {
            // SAFETY: The caller calls blocking `rx.recv()` so the receiver will never be dropped before this
            sender.send(Err(e)).unwrap();
          }
        }
      }
    }
    Message::CreateWindow(window_id, handler, sender) => match handler(event_loop) {
      Ok(webview) => {
        windows.0.borrow_mut().insert(window_id, webview);
        // SAFETY: The caller calls blocking `rx.recv()` so the receiver will never be dropped before this
        sender.send(Ok(())).unwrap();
      }
      Err(e) => {
        // SAFETY: The caller calls blocking `rx.recv()` so the receiver will never be dropped before this
        sender.send(Err(e)).unwrap();
      }
    },
    Message::CreateRawWindow(window_id, handler, sender) => {
      let (label, builder) = handler();

      #[cfg(windows)]
      let background_color = builder.window.background_color;
      #[cfg(windows)]
      let is_window_transparent = builder.window.transparent;

      if let Ok(window) = builder.build(event_loop) {
        window_id_map.insert(window.id(), window_id);

        let window = Arc::new(window);

        #[cfg(windows)]
        let surface = if is_window_transparent {
          if let Ok(context) = softbuffer::Context::new(window.clone()) {
            if let Ok(mut surface) = softbuffer::Surface::new(&context, window.clone()) {
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

        windows.0.borrow_mut().insert(
          window_id,
          WindowWrapper {
            label,
            has_children: AtomicBool::new(false),
            inner: Some(window.clone()),
            window_event_listeners: Default::default(),
            webviews: Vec::new(),
            #[cfg(windows)]
            background_color,
            #[cfg(windows)]
            is_window_transparent,
            #[cfg(windows)]
            surface,
            #[cfg(windows)]
            focused_webview: Default::default(),
          },
        );
        sender.send(Ok(Arc::downgrade(&window))).unwrap();
      } else {
        sender.send(Err(Error::CreateWindow)).unwrap();
      }
    }

    Message::UserEvent(_) => (),
    Message::EventLoopWindowTarget(message) => match message {
      EventLoopWindowTargetMessage::CursorPosition(sender) => {
        let pos = event_loop
          .cursor_position()
          .map_err(|_| Error::FailedToSendMessage);
        sender.send(pos).unwrap();
      }
      EventLoopWindowTargetMessage::PrimaryMonitor(sender) => {
        sender
          .send(
            event_loop
              .primary_monitor()
              .map(|m| MonitorHandleWrapper(m).into()),
          )
          .unwrap();
      }
      EventLoopWindowTargetMessage::MonitorFromPoint(sender, (x, y)) => {
        sender
          .send(
            event_loop
              .monitor_from_point(x, y)
              .map(|m| MonitorHandleWrapper(m).into()),
          )
          .unwrap();
      }
      EventLoopWindowTargetMessage::AvailableMonitors(sender) => {
        sender
          .send(
            event_loop
              .available_monitors()
              .map(|m| MonitorHandleWrapper(m).into())
              .collect(),
          )
          .unwrap();
      }
      EventLoopWindowTargetMessage::SetTheme(theme) => {
        event_loop.set_theme(to_tao_theme(theme));
        // On macOS tao caches each window's theme and only refreshes it from the
        // system-wide appearance change notification, which the app-level
        // `NSApp.setAppearance` call above never posts, so `Window::theme()`
        // would keep reporting the previous value. tao's window-level setter
        // does update the cache, so push the theme through it as well.
        #[cfg(target_os = "macos")]
        for window in windows.0.borrow().values() {
          if let Some(inner) = &window.inner {
            inner.set_theme(to_tao_theme(theme));
          }
        }
      }
      EventLoopWindowTargetMessage::SetDeviceEventFilter(filter) => {
        event_loop.set_device_event_filter(DeviceEventFilterWrapper::from(filter).0);
      }
    },
  }
}

fn handle_event_loop<T: UserEvent>(
  event: Event<'_, Message<T>>,
  event_loop: &EventLoopWindowTarget<Message<T>>,
  control_flow: &mut ControlFlow,
  context: EventLoopIterationContext<'_, T>,
) {
  let EventLoopIterationContext {
    callback,
    window_id_map,
    windows,
    #[cfg(feature = "tracing")]
    active_tracing_spans,
  } = context;
  if *control_flow != ControlFlow::Exit {
    *control_flow = ControlFlow::Wait;
  }

  match event {
    Event::NewEvents(StartCause::Init) => {
      callback(RunEvent::Ready);
    }

    Event::NewEvents(StartCause::Poll) => {
      callback(RunEvent::Resumed);
    }

    Event::MainEventsCleared => {
      callback(RunEvent::MainEventsCleared);
    }

    Event::LoopDestroyed => {
      callback(RunEvent::Exit);
    }

    #[cfg(windows)]
    Event::RedrawRequested(id) => {
      if let Some(window_id) = window_id_map.get(&id) {
        let mut windows_ref = windows.0.borrow_mut();
        if let Some(window) = windows_ref.get_mut(&window_id) {
          if window.is_window_transparent {
            let background_color = window.background_color;
            if let Some(surface) = &mut window.surface {
              if let Some(window) = &window.inner {
                window.draw_surface(surface, background_color);
              }
            }
          }
        }
      }
    }

    #[cfg(feature = "tracing")]
    Event::RedrawEventsCleared => {
      active_tracing_spans.remove_window_draw();
    }

    Event::UserEvent(Message::Webview(
      window_id,
      webview_id,
      WebviewMessage::WebviewEvent(event),
    )) => {
      let windows_ref = windows.0.borrow();
      if let Some(window) = windows_ref.get(&window_id) {
        if let Some(webview) = window.webviews.iter().find(|w| w.id == webview_id) {
          let label = webview.label.clone();
          let webview_event_listeners = webview.webview_event_listeners.clone();

          drop(windows_ref);

          callback(RunEvent::WebviewEvent {
            label,
            event: event.clone(),
          });
          let listeners = webview_event_listeners.lock().unwrap();
          let handlers = listeners.values();
          for handler in handlers {
            handler(&event);
          }
        }
      }
    }

    Event::UserEvent(Message::Webview(
      window_id,
      _webview_id,
      WebviewMessage::SynthesizedWindowEvent(event),
    )) => {
      if let Some(event) = WindowEventWrapper::from(event).0 {
        let windows_ref = windows.0.borrow();
        let window = windows_ref.get(&window_id);
        if let Some(window) = window {
          let label = window.label.clone();
          let window_event_listeners = window.window_event_listeners.clone();

          drop(windows_ref);

          callback(RunEvent::WindowEvent {
            label,
            event: event.clone(),
          });

          let listeners = window_event_listeners.lock().unwrap();
          let handlers = listeners.values();
          for handler in handlers {
            handler(&event);
          }
        }
      }
    }

    Event::WindowEvent {
      event, window_id, ..
    } => {
      if let Some(window_id) = window_id_map.get(&window_id) {
        {
          let windows_ref = windows.0.borrow();
          if let Some(window) = windows_ref.get(&window_id) {
            if let Some(event) = WindowEventWrapper::parse(window, &event).0 {
              let label = window.label.clone();
              let window_event_listeners = window.window_event_listeners.clone();

              drop(windows_ref);

              callback(RunEvent::WindowEvent {
                label,
                event: event.clone(),
              });
              let listeners = window_event_listeners.lock().unwrap();
              let handlers = listeners.values();
              for handler in handlers {
                handler(&event);
              }
            }
          }
        }

        match event {
          #[cfg(windows)]
          TaoWindowEvent::ThemeChanged(theme) => {
            if let Some(window) = windows.0.borrow().get(&window_id) {
              for webview in &window.webviews {
                let theme = match theme {
                  TaoTheme::Dark => wry::Theme::Dark,
                  TaoTheme::Light => wry::Theme::Light,
                  _ => wry::Theme::Light,
                };
                if let Err(e) = webview.set_theme(theme) {
                  log::error!("failed to set theme: {e}");
                }
              }
            }
          }
          TaoWindowEvent::CloseRequested => {
            on_close_requested(callback, window_id, windows);
          }
          TaoWindowEvent::Destroyed => {
            let removed = windows.0.borrow_mut().remove(&window_id).is_some();
            if removed {
              let is_empty = windows.0.borrow().is_empty();
              if is_empty {
                let (tx, rx) = channel();
                callback(RunEvent::ExitRequested { code: None, tx });

                let recv = rx.try_recv();
                let should_prevent = matches!(recv, Ok(ExitRequestedEventAction::Prevent));

                if !should_prevent {
                  *control_flow = ControlFlow::Exit;
                }
              }
            }
          }
          TaoWindowEvent::Resized(size) => {
            if let Some((Some(window), webviews)) = windows
              .0
              .borrow()
              .get(&window_id)
              .map(|w| (w.inner.clone(), w.webviews.clone()))
            {
              let size = size.to_logical::<f32>(window.scale_factor());
              for webview in webviews {
                if let Some(b) = &*webview.bounds.lock().unwrap() {
                  if let Err(e) = webview.set_bounds(wry::Rect {
                    position: LogicalPosition::new(size.width * b.x_rate, size.height * b.y_rate)
                      .into(),
                    size: LogicalSize::new(size.width * b.width_rate, size.height * b.height_rate)
                      .into(),
                  }) {
                    log::error!("failed to autoresize webview: {e}");
                  }
                }
              }
            }
          }
          _ => {}
        }
      }
    }
    Event::UserEvent(message) => match message {
      Message::RequestExit(code) => {
        let (tx, rx) = channel();
        callback(RunEvent::ExitRequested {
          code: Some(code),
          tx,
        });

        let recv = rx.try_recv();
        let should_prevent = matches!(recv, Ok(ExitRequestedEventAction::Prevent));

        if !should_prevent {
          *control_flow = ControlFlow::ExitWithCode(code);
        }
      }
      Message::Window(id, WindowMessage::Close) => {
        on_close_requested(callback, id, windows);
      }
      Message::Window(id, WindowMessage::Destroy) => {
        on_window_close(id, windows);
      }
      Message::UserEvent(t) => callback(RunEvent::UserEvent(t)),
      message => {
        handle_user_message(
          event_loop,
          message,
          UserMessageContext {
            window_id_map,
            windows,
          },
        );
      }
    },
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
    Event::Opened { urls } => {
      callback(RunEvent::Opened { urls });
    }
    #[cfg(target_os = "macos")]
    Event::Reopen {
      has_visible_windows,
      ..
    } => callback(RunEvent::Reopen {
      has_visible_windows,
    }),
    #[cfg(target_os = "ios")]
    Event::SceneRequested { scene, options } => {
      callback(RunEvent::SceneRequested { scene, options });
    }
    _ => (),
  }
}

fn on_close_requested<'a, T: UserEvent>(
  callback: &'a mut (dyn FnMut(RunEvent<T>) + 'static),
  window_id: WindowId,
  windows: &WindowsStore,
) {
  let (tx, rx) = channel();
  let windows_ref = windows.0.borrow();
  if let Some(w) = windows_ref.get(&window_id) {
    let label = w.label.clone();
    let window_event_listeners = w.window_event_listeners.clone();

    drop(windows_ref);

    let listeners = window_event_listeners.lock().unwrap();
    let handlers = listeners.values();
    for handler in handlers {
      handler(&WindowEvent::CloseRequested {
        signal_tx: tx.clone(),
      });
    }
    callback(RunEvent::WindowEvent {
      label,
      event: WindowEvent::CloseRequested { signal_tx: tx },
    });
    if let Ok(true) = rx.try_recv() {
    } else {
      on_window_close(window_id, windows);
    }
  }
}

fn on_window_close(window_id: WindowId, windows: &WindowsStore) {
  if let Some(window_wrapper) = windows.0.borrow_mut().get_mut(&window_id) {
    window_wrapper.inner = None;
    #[cfg(windows)]
    window_wrapper.surface.take();
  }
}


]

parent

*/
