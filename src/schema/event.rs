use std::{path::PathBuf, sync::atomic::Ordering};

use crate::{
  core::window::{Window, map_theme_from_tao},
  schema::{Rect, Theme},
  utils::inner_size,
};
use dpi::PhysicalPosition;
use serde;
use serde::Serialize;
use tao::event::WindowEvent as TaoWindowEvent;
use tray_icon::TrayIconId;

/// Describes the mouse button state.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum MouseButtonState {
  /// Mouse button released.
  #[default]
  Up,

  /// Mouse button pressed.
  Down,
}

impl From<tray_icon::MouseButtonState> for MouseButtonState {
  fn from(value: tray_icon::MouseButtonState) -> Self {
    match value {
      tray_icon::MouseButtonState::Up => Self::Up,
      tray_icon::MouseButtonState::Down => Self::Down,
    }
  }
}

/// Describes which mouse button triggered the event.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Default)]
pub enum MouseButton {
  #[default]
  Left,
  Right,
  Middle,
}

impl From<tray_icon::MouseButton> for MouseButton {
  fn from(value: tray_icon::MouseButton) -> Self {
    match value {
      tray_icon::MouseButton::Left => Self::Left,
      tray_icon::MouseButton::Right => Self::Right,
      tray_icon::MouseButton::Middle => Self::Middle,
    }
  }
}

/// Describes a tray icon event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum TrayIconEvent {
  #[serde(rename_all = "camelCase")]
  Click {
    id: TrayIconId,
    position: PhysicalPosition<f64>,
    rect: Rect,
    button: MouseButton,
    button_state: MouseButtonState,
  },

  DoubleClick {
    id: TrayIconId,
    position: PhysicalPosition<f64>,
    rect: Rect,
    button: MouseButton,
  },

  Enter {
    id: TrayIconId,
    position: PhysicalPosition<f64>,
    rect: Rect,
  },

  Move {
    id: TrayIconId,
    position: PhysicalPosition<f64>,
    rect: Rect,
  },

  Leave {
    id: TrayIconId,
    position: PhysicalPosition<f64>,
    rect: Rect,
  },
}

impl TrayIconEvent {
  pub fn id(&self) -> &TrayIconId {
    match self {
      Self::Click { id, .. }
      | Self::DoubleClick { id, .. }
      | Self::Enter { id, .. }
      | Self::Move { id, .. }
      | Self::Leave { id, .. } => id,
    }
  }
}

impl From<tray_icon::TrayIconEvent> for TrayIconEvent {
  fn from(value: tray_icon::TrayIconEvent) -> Self {
    match value {
      tray_icon::TrayIconEvent::Click {
        id,
        position,
        rect,
        button,
        button_state,
      } => Self::Click {
        id,
        position,
        rect: Rect {
          position: rect.position.into(),
          size: rect.size.into(),
        },
        button: button.into(),
        button_state: button_state.into(),
      },

      tray_icon::TrayIconEvent::DoubleClick {
        id,
        position,
        rect,
        button,
      } => Self::DoubleClick {
        id,
        position,
        rect: Rect {
          position: rect.position.into(),
          size: rect.size.into(),
        },
        button: button.into(),
      },

      tray_icon::TrayIconEvent::Enter { id, position, rect } => Self::Enter {
        id,
        position,
        rect: Rect {
          position: rect.position.into(),
          size: rect.size.into(),
        },
      },

      tray_icon::TrayIconEvent::Move { id, position, rect } => Self::Move {
        id,
        position,
        rect: Rect {
          position: rect.position.into(),
          size: rect.size.into(),
        },
      },

      tray_icon::TrayIconEvent::Leave { id, position, rect } => Self::Leave {
        id,
        position,
        rect: Rect {
          position: rect.position.into(),
          size: rect.size.into(),
        },
      },

      // tray_icon::TrayIconEvent is #[non_exhaustive].
      // With the currently supported tray-icon version these are all
      // known variants.
      _ => unreachable!("unsupported tray-icon event variant"),
    }
  }
}

/// An event from a window.
#[derive(Debug, Clone)]
pub enum WindowEvent {
  /// The size of the window has changed. Contains the client area's new dimensions.
  Resized(dpi::PhysicalSize<u32>),
  /// The position of the window has changed. Contains the window's new position.
  Moved(dpi::PhysicalPosition<i32>),
  /// The window has been requested to close.
  CloseRequested { window_label: String },
  /// The window has been destroyed.
  Destroyed,
  /// The window gained or lost focus.
  ///
  /// The parameter is true if the window has gained focus, and false if it has lost focus.
  Focused(bool),
  /// The window's scale factor has changed.
  ///
  /// The following user actions can cause DPI changes:
  ///
  /// - Changing the display's resolution.
  /// - Changing the display's scale factor (e.g. in Control Panel on Windows).
  /// - Moving the window to a display with a different scale factor.
  ScaleFactorChanged {
    /// The new scale factor.
    scale_factor: f64,
    /// The window inner size.
    new_inner_size: dpi::PhysicalSize<u32>,
  },
  /// An event associated with the drag and drop action.
  DragDrop(DragDropEvent),
  /// The system window theme has changed.
  ///
  /// Applications might wish to react to this to change the theme of the content of the window when the system changes the window theme.
  ThemeChanged(Theme),

  /// Emitted when the application has been suspended.
  ///
  /// ## Platform-specific
  ///
  /// - **Android**: This is triggered by `onPause` method of the Activity.
  /// - **iOS**: This is triggered by `applicationWillResignActive` method of the UIApplicationDelegate.
  /// - **Linux / macOS / Windows**: Unsupported.
  #[cfg(mobile)]
  #[cfg_attr(docsrs, doc(cfg(any(target_os = "android", target_os = "ios"))))]
  Suspended,

  /// Emitted when the application has been resumed.
  ///
  /// ## Platform-specific
  ///
  /// - **Android**: This is triggered by `onResume` method of the Activity. The first onResume() is ignored to match the iOS implementation, since that is called on activity creation.
  /// - **iOS**: This is triggered by `applicationWillEnterForeground` method of the UIApplicationDelegate.
  /// - **Linux / macOS / Windows**: Unsupported.
  #[cfg(mobile)]
  #[cfg_attr(docsrs, doc(cfg(any(target_os = "android", target_os = "ios"))))]
  Resumed,
}

/// An event from a window.
#[derive(Debug, Clone)]
pub enum WebviewEvent {
  /// An event associated with the drag and drop action.
  DragDrop(DragDropEvent),
}

/// The drag drop event payload.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DragDropEvent {
  /// A drag operation has entered the webview.
  Enter {
    /// List of paths that are being dragged onto the webview.
    paths: Vec<PathBuf>,
    /// The position of the mouse cursor.
    position: dpi::PhysicalPosition<f64>,
  },
  /// A drag operation is moving over the webview.
  Over {
    /// The position of the mouse cursor.
    position: dpi::PhysicalPosition<f64>,
  },
  /// The file(s) have been dropped onto the webview.
  Drop {
    /// List of paths that are being dropped onto the window.
    paths: Vec<PathBuf>,
    /// The position of the mouse cursor.
    position: dpi::PhysicalPosition<f64>,
  },
  /// The drag operation has been cancelled or left the window.
  Leave,
}

#[derive(Debug, Clone)]
pub enum SynthesizedWindowEvent {
  Focused(bool),
  DragDrop(DragDropEvent),
}

pub struct WindowEventWrapper(pub Option<WindowEvent>);

impl WindowEventWrapper {
  pub fn map_from_tao(event: &TaoWindowEvent<'_>, #[cfg(windows)] window: &Window) -> Self {
    let event = match event {
      TaoWindowEvent::Resized(size) => WindowEvent::Resized(*size),
      TaoWindowEvent::Moved(position) => WindowEvent::Moved(*position),
      TaoWindowEvent::Destroyed => WindowEvent::Destroyed,
      TaoWindowEvent::ScaleFactorChanged {
        scale_factor,
        new_inner_size,
      } => WindowEvent::ScaleFactorChanged {
        scale_factor: *scale_factor,
        new_inner_size: **new_inner_size,
      },
      TaoWindowEvent::Focused(focused) => {
        #[cfg(not(windows))]
        return Self(Some(WindowEvent::Focused(*focused)));
        // on multiwebview mode, if there's no focused webview, it means we're receiving a direct window focus change
        // (without receiving a webview focus, such as when clicking the taskbar app icon or using Alt + Tab)
        // in this case we must send the focus change event here
        #[cfg(windows)]
        if window.has_children.load(Ordering::Relaxed) {
          use crate::schema::FocusState;

          if !*focused {
            // Blur events are handled in the webview side (add_LostFocus)
            return Self(None);
          }

          let mut focused_webview = window.focused_webview.lock().unwrap();
          if let FocusState::Blured {
            last_focused_webview_label,
          } = &*focused_webview
          {
            let should_focus_webview = last_focused_webview_label
              .as_deref()
              .and_then(|last_focused_webview_label| {
                window
                  .webviews()
                  .iter()
                  .find(|w| w.label() == last_focused_webview_label)
              });
            *focused_webview = FocusState::WindowFocused;
            if let Some(should_focus_webview) = should_focus_webview {
              drop(focused_webview);
              let _ = should_focus_webview.focus();
            }
            WindowEvent::Focused(true)
          } else {
            // Already focused
            return Self(None);
          }
        } else if window.webviews().is_empty() {
          // Raw tao window without webviews, forward the event
          WindowEvent::Focused(*focused)
        } else {
          // when not on multiwebview mode, wry will set focus to the webview,
          // and we will handle focus change events on the webview (add_GotFocus and add_LostFocus)
          return Self(None);
        }
      }
      TaoWindowEvent::ThemeChanged(theme) => WindowEvent::ThemeChanged(map_theme_from_tao(*theme)),
      #[cfg(mobile)]
      TaoWindowEvent::Suspended => WindowEvent::Suspended,
      #[cfg(mobile)]
      TaoWindowEvent::Resumed => WindowEvent::Resumed,
      _ => return Self(None),
    };
    Self(Some(event))
  }

  pub fn parse(window: &Window, event: &TaoWindowEvent<'_>) -> anyhow::Result<Self> {
    match event {
      // resized event from tao doesn't include a reliable size on macOS
      // because wry replaces the NSView
      TaoWindowEvent::Resized(_) => {
        if let Some(w) = &window.inner {
          let size = inner_size(w, &window.webviews(), window.has_children.load(Ordering::Relaxed))?;
          Ok(Self(Some(WindowEvent::Resized(size))))
        } else {
          Ok(Self(None))
        }
      }
      e => Ok(Self::map_from_tao(
        e,
        #[cfg(windows)]
        window,
      )),
    }
  }
}

impl From<SynthesizedWindowEvent> for WindowEventWrapper {
  fn from(event: SynthesizedWindowEvent) -> Self {
    let event = match event {
      SynthesizedWindowEvent::Focused(focused) => WindowEvent::Focused(focused),
      SynthesizedWindowEvent::DragDrop(event) => WindowEvent::DragDrop(event),
    };
    Self(Some(event))
  }
}
