use std::{
  fmt::{Debug, Formatter},
  ops::Deref,
  pin::Pin,
};
use tao::{
  event::Event,
  event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopClosed, EventLoopProxy, EventLoopWindowTarget},
};

use crate::schema::{
  PhysicalRect,
  event::{SynthesizedWindowEvent, WebViewEvent},
  webview::WebViewId,
  window::WindowId,
};
pub mod aio;
pub mod config;
pub mod logging;
pub mod menu;
pub mod schema;
pub mod tools;
pub mod trayicon;
pub mod utils;
pub mod webview;
pub mod window;
#[cfg(windows)]
pub mod windows;
pub mod wrappers;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]
pub mod linux;

pub type EngineLoop = EventLoop<EventLoopMessage>;
pub type EngineLoopBuilder = EventLoopBuilder<EventLoopMessage>;
pub type EngineWindowTarget = EventLoopWindowTarget<EventLoopMessage>;
pub type EngineLoopProxy = EventLoopProxy<EventLoopMessage>;
pub type EngineLoopClosed = EventLoopClosed<EventLoopMessage>;
pub type EngineLoopEvent<'a> = Event<'a, EventLoopMessage>;
pub type EngineCallback = Pin<Box<dyn Fn(&EngineWindowTarget, &mut ControlFlow) -> anyhow::Result<()> + Send>>;

// ─────────────────────────────────────────────
// Monitor extensions
// ─────────────────────────────────────────────

pub trait MonitorExt {
  /// Get the work area of this monitor.
  ///
  /// ## Platform-specific
  ///
  /// - **Android / iOS**: Unsupported.
  fn work_area(&self) -> PhysicalRect<i32, u32>;
}

#[cfg(mobile)]
impl MonitorExt for tao::monitor::MonitorHandle {
  fn work_area(&self) -> PhysicalRect<i32, u32> {
    PhysicalRect {
      size: self.size(),
      position: self.position(),
    }
  }
}

// ─────────────────────────────────────────────
// Window extensions
// ─────────────────────────────────────────────

pub trait WindowExt {
  /// Enable or disable the window.
  ///
  /// ## Platform-specific
  ///
  /// - **Android / iOS**: Unsupported.
  fn set_enabled(&self, enabled: bool);

  /// Whether the window is enabled or disabled.
  ///
  /// ## Platform-specific
  ///
  /// - **Android / iOS**: Unsupported, always returns `true`.
  fn is_enabled(&self) -> bool;

  /// Center the window.
  ///
  /// ## Platform-specific
  ///
  /// - **Android / iOS**: Unsupported.
  #[cfg(not(any(target_os = "ios", target_os = "android")))]
  fn center(&self) {}

  /// Clears the window surface, i.e. makes it transparent.
  #[cfg(windows)]
  fn draw_surface(
    &self,
    surface: &mut softbuffer::Surface<std::sync::Arc<tao::window::Window>, std::sync::Arc<tao::window::Window>>,
    background_color: Option<tao::window::RGBA>,
  );
}

#[cfg(desktop)]
pub fn calculate_window_center_position(
  window_size: tao::dpi::PhysicalSize<u32>,
  target_monitor: tao::monitor::MonitorHandle,
) -> tao::dpi::PhysicalPosition<i32> {
  let work_area = target_monitor.work_area();

  tao::dpi::PhysicalPosition::new(
    (work_area.size.width as i32 - window_size.width as i32) / 2 + work_area.position.x,
    (work_area.size.height as i32 - window_size.height as i32) / 2 + work_area.position.y,
  )
}

#[cfg(mobile)]
impl WindowExt for tao::window::Window {
  fn set_enabled(&self, _: bool) {}

  fn is_enabled(&self) -> bool {
    true
  }
}

/// Internal messages delivered through the Tao user-event channel.
///
/// These messages provide a thread-safe mechanism for scheduling engine work on
/// the event-loop thread.
///
/// Code executing outside the GUI event loop should generally send one of these
/// messages instead of manipulating Tao state directly.
pub enum EventLoopMessage {
  /// Requests graceful termination of the entire application.
  ///
  /// The request is processed by [`EngineEventHandler`] on the event-loop
  /// thread.
  Shutdown,

  /// Executes a dynamically supplied Taurino event-loop operation.
  ///
  /// This variant allows engine subsystems to schedule arbitrary operations
  /// that require access to the Tao event-loop target and its
  /// [`ControlFlow`].
  TaskWithTarget(EngineEvent),
  /// Executes a dynamically supplied event-loop operation.
  ///
  /// This variant allows engine subsystems to schedule arbitrary operations
  /// that require to run on the Tao event-loop target thread
  Task(Box<dyn FnOnce() + Send>),

  #[cfg(target_os = "macos")]
  SetDockVisibility(bool),
  RequestExit(i32),
  SynthesizedWindowEvent(WindowId, WebViewId, SynthesizedWindowEvent),
  WebviewEvent(WindowId, WebViewId, WebViewEvent),
  ContainsFullScreenElementChanged(WindowId, bool),
}

/// Type-erased operation scheduled for execution on the Tao event-loop thread.
///
/// The callback receives:
///
/// - the current [`EngineWindowTarget`], allowing creation or manipulation of
///   event-loop-bound objects;
/// - mutable [`ControlFlow`], allowing the callback to influence future
///   event-loop execution.
///
/// The callback is `Send` because it may be created on another thread before
/// being transported through the Tao user-event channel.
///
/// It is boxed and pinned to provide one stable, type-erased representation for
/// arbitrary callback implementations.
///
pub struct EngineEvent(EngineCallback);

impl Debug for EngineEvent {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EngineEvent").finish()
  }
}

impl EngineEvent {
  pub fn new<F: Fn(&EngineWindowTarget, &mut ControlFlow) -> anyhow::Result<()> + Send + 'static>(f: F) -> Self {
    Self(Box::pin(f))
  }
}

impl Deref for EngineEvent {
  type Target = Pin<Box<dyn Fn(&EngineWindowTarget, &mut ControlFlow) -> anyhow::Result<()> + Send>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub mod native {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "windows",
    target_os = "macos",
  ))]
  pub use muda;

  pub use tao;
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "windows",
    target_os = "macos",
  ))]
  pub use tray_icon;
  pub use wry;
}

// ─────────────────────────────────────────────
// Synchronization helpers
// ─────────────────────────────────────────────
#[macro_export]
macro_rules! unsafe_impl_sync_send {
  ($type:ty) => {
    unsafe impl Send for $type {}
    unsafe impl Sync for $type {}
  };
}

#[macro_export]
macro_rules! set_property_some {
  ($builder:ident, $property:ident, &$value:expr) => {
    if let Some(value) = &$value {
      $builder = $builder.$property(value);
    }
  };
  ($builder:ident, $property:ident, $value:expr) => {
    if let Some(value) = $value {
      $builder = $builder.$property(value.clone());
    }
  };
}

#[macro_export]
macro_rules! set_property {
  ($builder:ident, $property:ident, $value:expr) => {
    $builder = $builder.$property($value);
  };
}

#[macro_export]
macro_rules! lock {
  ($value:expr) => {
    $value
      .lock()
      .map_err(|_| anyhow::anyhow!("Failed to lock {}.", stringify!($value)))
  };
}

#[macro_export]
macro_rules! lock_force {
  ($value:expr) => {
    $value.lock().unwrap()
  };
}
