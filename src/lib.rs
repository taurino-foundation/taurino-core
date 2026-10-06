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
pub mod async_runtime;
pub mod menu;
pub mod schema;
pub mod tools;
pub mod trayicon;
pub mod webview;
pub mod window;

pub mod platforms;
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

use anyhow::Result;
use url::Url;

use crate::{
  schema::{FrontendDist, window::WindowConfig},
  tools::environment::Env,
};

pub type EngineLoop = EventLoop<EventLoopMessage>;
pub type EngineLoopBuilder = EventLoopBuilder<EventLoopMessage>;
pub type EngineWindowTarget = EventLoopWindowTarget<EventLoopMessage>;
pub type EngineLoopProxy = EventLoopProxy<EventLoopMessage>;
pub type EngineLoopClosed = EventLoopClosed<EventLoopMessage>;
pub type EngineLoopEvent<'a> = Event<'a, EventLoopMessage>;
pub type EngineCallback = Pin<Box<dyn Fn(&EngineWindowTarget, &mut ControlFlow) -> anyhow::Result<()> + Send>>;

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

#[derive(Debug, Clone)]
pub struct Config {
  env: Env,
  frontend_dist: FrontendDist,
  windows: Vec<WindowConfig>,
}

impl Config {
  pub fn new(_config: &str) -> Result<Self> {
    let env = Env::default();

    let frontend_dist = FrontendDist::Url(Url::parse("https://tauri.app")?); // Directory("dist".into());

    Ok(Self {
      env,
      frontend_dist,
      windows: Vec::new(),
    })
  }
  pub fn add_window_config(&mut self, config: WindowConfig) {
    self.windows.push(config);
  }

  pub fn get_env(&self) -> Env {
    self.env.clone()
  }
  pub fn get_windows(&self) -> &[WindowConfig] {
    &self.windows
  }
  pub fn frontend_dist(&self) -> FrontendDist {
    self.frontend_dist.clone()
  }
}
