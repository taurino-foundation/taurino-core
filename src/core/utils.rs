use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    fmt::{self, Debug},
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};

use tao::{event_loop::EventLoopWindowTarget, window::WindowId as TaoWindowId};
use wry::WebContext as WryWebContext;

use crate::messages::Message;

/// A type that can be used as a user event.
pub trait UserEvent: Debug + Clone + Send + 'static {}

impl<T: Debug + Clone + Send + 'static> UserEvent for T {}

/// Identifier of a window.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct WindowId(u32);

impl From<u32> for WindowId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl FromStr for WindowId {
    type Err = std::num::ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.parse::<u32>()?))
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier of a webview.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct WebViewId(u32);

impl From<u32> for WebViewId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl FromStr for WebViewId {
    type Err = std::num::ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.parse::<u32>()?))
    }
}

impl fmt::Display for WebViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Creates a stable internal webview label.
///
/// Example:
///
/// ```text
/// 1:main:2
/// ```
///
///
#[allow(dead_code)]
pub(crate) fn make_webview_label(window_id: WindowId, label: &str, webview_id: WebViewId) -> String {
    format!("{}:{}:{}", window_id, label, webview_id)
}

/// Extracts the `WebViewId` from an internal webview label.
///
/// Expected format:
///
/// ```text
/// window_id:label:webview_id
/// ```
///
///
#[allow(dead_code)]
pub(crate) fn extract_webview_id_from_label(webview_label: &str) -> crate::app_error::Result<WebViewId> {
    let id = webview_label
        .rsplit_once(':')
        .map(|(_, id)| id)
        .ok_or(crate::app_error::Error::InvalidWindowLabel)?;

    id.parse::<WebViewId>()
        .map_err(|_| crate::app_error::Error::InvalidWindowLabel)
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct WebContext {
    pub inner: WryWebContext,
    pub referenced_by_webviews: HashSet<String>,
    pub registered_custom_protocols: HashSet<String>,
}

pub type WebContextStore = Arc<Mutex<HashMap<Option<PathBuf>, WebContext>>>;

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct WindowIdStore(Arc<Mutex<HashMap<TaoWindowId, WindowId>>>);

impl WindowIdStore {
    #[allow(dead_code)]
    pub fn insert(&self, w: TaoWindowId, id: WindowId) {
        self.0.lock().unwrap().insert(w, id);
    }
    #[allow(dead_code)]
    pub fn get(&self, w: &TaoWindowId) -> Option<WindowId> {
        self.0.lock().unwrap().get(w).copied()
    }
}

#[derive(Debug)]
pub struct WindowWrapper {}

#[allow(dead_code)]
#[derive(Debug)]
pub struct WindowsStore(pub RefCell<BTreeMap<WindowId, WindowWrapper>>);

// SAFETY: this type is only used on the main thread.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for WindowsStore {}

// SAFETY: this type is only used on the main thread.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Sync for WindowsStore {}

#[allow(dead_code)]
pub enum RunEvent<T: UserEvent> {
    UserEvent(T),
}

#[allow(dead_code)]
pub struct EventLoopIterationContext<'a, T: UserEvent> {
    pub callback: &'a mut (dyn FnMut(RunEvent<T>) + 'static),
    pub window_id_map: WindowIdStore,
    pub windows: Arc<WindowsStore>,
}

#[allow(dead_code)]
pub(crate) struct UserMessageContext {
    pub(crate) windows: Arc<WindowsStore>,
    pub(crate) window_id_map: WindowIdStore,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DispatcherMainThreadContext<T: UserEvent> {
    pub window_target: EventLoopWindowTarget<Message<T>>,
    pub web_context: WebContextStore,
    pub windows: Arc<WindowsStore>,
}

// SAFETY: this type is only used on the main thread.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T: UserEvent> Send for DispatcherMainThreadContext<T> {}

// SAFETY: this type is only used on the main thread.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T: UserEvent> Sync for DispatcherMainThreadContext<T> {}
