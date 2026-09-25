use dpi::PhysicalSize;
use dpi::Position;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

pub mod image;
pub mod resource;
use tao::window::Window;
/* use url::Url;
 */
use tao::window::WindowId as TaoWindowId;
use tao::{monitor::MonitorHandle, window::Theme as TaoTheme};
use wry::{/* ProxyConfig, ProxyEndpoint, */ WebContext as WryWebContext};
pub(crate) mod wrappers;
use crate::{
    types::{PermissionKind, PermissionResponse, Theme},
    webview::WebViewId,
    window::{WindowId /* WindowsStore */},
};

/* #[cfg(windows)]
use tao::platform::windows::WindowExtWindows; */

#[cfg(target_os = "macos")]
use wry::WebViewExtMacOS;
/* #[cfg(windows)]
use wry::WebViewExtWindows; */

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use wry::WebViewExtUnix;

#[cfg(target_os = "macos")]
use tao::platform::macos::WindowExtMacOS;
#[cfg(target_os = "macos")]
use wry::WebViewExtMacOS;

use crate::webview::ManagedWebview;

pub type ArcMutHashMap<K, V> = Arc<Mutex<HashMap<K, V>>>;
/* pub type ArcMutHashSet<T> = Arc<Mutex<HashSet<T>>>; */
pub type ArcMut<T> = Arc<Mutex<T>>;

// ---------- Example concrete aliases (e.g., for file paths) ----------
/*
pub type PathMap<V> = ArcMutHashMap<PathBuf, V>;
pub type PathSet = ArcMutHashSet<PathBuf>;

// ---------- HashMap helpers ----------

/// Create a new empty `ArcMutHashMap`.
pub fn new_arc_mut_hashmap<K, V>() -> ArcMutHashMap<K, V> {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Lock the mutex and return the guard (panics if poisoned).
pub fn lock_map<K, V>(
    map: &ArcMutHashMap<K, V>,
) -> MutexGuard<'_, HashMap<K, V>> {
    map.lock().expect("ArcMutHashMap mutex poisoned")
}

/// Insert a key/value pair, returning the previous value if any.
pub fn map_insert<K, V>(
    map: &ArcMutHashMap<K, V>,
    key: K,
    value: V,
) -> Option<V>
where
    K: Eq + Hash,
{
    lock_map(map).insert(key, value)
}

/// Get a cloned value for a key (requires `V: Clone`).
pub fn map_get<K, V>(map: &ArcMutHashMap<K, V>, key: &K) -> Option<V>
where
    K: Eq + Hash,
    V: Clone,
{
    lock_map(map).get(key).cloned()
}

/// Remove a key, returning the value if it existed.
pub fn map_remove<K, V>(map: &ArcMutHashMap<K, V>, key: &K) -> Option<V>
where
    K: Eq + Hash,
{
    lock_map(map).remove(key)
}

/// Check whether a key exists.
pub fn map_contains_key<K, V>(map: &ArcMutHashMap<K, V>, key: &K) -> bool
where
    K: Eq + Hash,
{
    lock_map(map).contains_key(key)
}

/// Number of entries.
pub fn map_len<K, V>(map: &ArcMutHashMap<K, V>) -> usize {
    lock_map(map).len()
}

// ---------- HashSet helpers ----------

/// Create a new empty `ArcMutHashSet`.
pub fn new_arc_mut_hashset<T>() -> ArcMutHashSet<T> {
    Arc::new(Mutex::new(HashSet::new()))
}

/// Lock the mutex and return the guard (panics if poisoned).
pub fn lock_set<T>(set: &ArcMutHashSet<T>) -> MutexGuard<'_, HashSet<T>> {
    set.lock().expect("ArcMutHashSet mutex poisoned")
}

/// Insert a value; returns `true` if it was newly inserted.
pub fn set_insert<T>(set: &ArcMutHashSet<T>, value: T) -> bool
where
    T: Eq + Hash,
{
    lock_set(set).insert(value)
}

/// Check membership.
pub fn set_contains<T>(set: &ArcMutHashSet<T>, value: &T) -> bool
where
    T: Eq + Hash,
{
    lock_set(set).contains(value)
}

/// Remove a value; returns `true` if it was present.
pub fn set_remove<T>(set: &ArcMutHashSet<T>, value: &T) -> bool
where
    T: Eq + Hash,
{
    lock_set(set).remove(value)
}

/// Number of entries.
pub fn set_len<T>(set: &ArcMutHashSet<T>) -> usize {
    lock_set(set).len()
}


/// Reconstructs a path from its components using the platform separator then converts it to String and removes UNC prefixes on Windows if it exists.
pub fn display_path<P: AsRef<Path>>(p: P) -> String {
    dunce::simplified(&p.as_ref().components().collect::<PathBuf>())
        .display()
        .to_string()
}

/// Write the file only if the content of the existing file (if any) is different.
///
/// This will always write unless the file exists with identical content.
pub fn write_if_changed<P, C>(path: P, content: C) -> std::io::Result<()>
where
    P: AsRef<Path>,
    C: AsRef<[u8]>,
{
    if std::fs::read(&path).is_ok_and(|existing| existing == content.as_ref()) {
        return Ok(());
    }

    std::fs::write(path, content)
} */

#[derive(Debug)]
pub struct WebContext {
    pub inner: WryWebContext,
    pub referenced_by_webviews: HashSet<String>,
    // on Linux the custom protocols are associated with the context
    // and you cannot register a URI scheme more than once
    pub registered_custom_protocols: HashSet<String>,
}

pub fn map_theme(theme: &TaoTheme) -> Theme {
    match theme {
        TaoTheme::Light => Theme::Light,
        TaoTheme::Dark => Theme::Dark,
        _ => Theme::Light,
    }
}

/// Window icon.
#[derive(Debug, Clone)]
pub struct Icon<'a> {
    /// RGBA bytes of the icon.
    pub rgba: Cow<'a, [u8]>,
    /// Icon width.
    pub width: u32,
    /// Icon height.
    pub height: u32,
}

pub fn find_monitor_for_position(
    monitors: impl Iterator<Item = MonitorHandle>,
    window_position: Position,
) -> Option<MonitorHandle> {
    monitors.into_iter().find(|m| {
        let monitor_pos = m.position();
        let monitor_size = m.size();

        // type annotations required for 32bit targets.
        let window_position =
            window_position.to_physical::<i32>(m.scale_factor());

        monitor_pos.x <= window_position.x
            && window_position.x < monitor_pos.x + monitor_size.width as i32
            && monitor_pos.y <= window_position.y
            && window_position.y < monitor_pos.y + monitor_size.height as i32
    })
}

pub fn from_wry_permission_kind(kind: wry::PermissionKind) -> PermissionKind {
    match kind {
        wry::PermissionKind::Microphone => PermissionKind::Microphone,
        wry::PermissionKind::Camera => PermissionKind::Camera,
        wry::PermissionKind::Geolocation => PermissionKind::Geolocation,
        wry::PermissionKind::Notifications => PermissionKind::Notifications,
        wry::PermissionKind::ClipboardRead => PermissionKind::ClipboardRead,
        wry::PermissionKind::DisplayCapture => PermissionKind::DisplayCapture,
        wry::PermissionKind::Midi => PermissionKind::Midi,
        wry::PermissionKind::Sensors => PermissionKind::Sensors,
        wry::PermissionKind::MediaKeySystemAccess => {
            PermissionKind::MediaKeySystemAccess
        }
        wry::PermissionKind::LocalFonts => PermissionKind::LocalFonts,
        wry::PermissionKind::WindowManagement => {
            PermissionKind::WindowManagement
        }
        wry::PermissionKind::PointerLock => PermissionKind::PointerLock,
        wry::PermissionKind::AutomaticDownloads => {
            PermissionKind::AutomaticDownloads
        }
        wry::PermissionKind::FileSystemAccess => {
            PermissionKind::FileSystemAccess
        }
        wry::PermissionKind::Autoplay => PermissionKind::Autoplay,
        wry::PermissionKind::Other => PermissionKind::Other,
        _ => PermissionKind::Other,
    }
}

pub fn to_wry_permission_response(
    response: PermissionResponse,
) -> wry::PermissionResponse {
    match response {
        PermissionResponse::Allow => wry::PermissionResponse::Allow,
        PermissionResponse::Deny => wry::PermissionResponse::Deny,
        PermissionResponse::Default => wry::PermissionResponse::Default,
    }
}

#[cfg(target_os = "android")]
pub struct CreationContext<'a, 'b> {
    pub env: &'a mut jni::JNIEnv<'b>,
    pub activity: &'a jni::objects::JObject<'b>,
    pub webview: &'a jni::objects::JObject<'b>,
}

/// Information about the webview that initiated a new window request.
#[derive(Debug)]
pub struct NewWindowOpener {
    /// The instance of the webview that initiated the new window request.
    ///
    /// This must be set as the related view of the new webview. See [`WebviewAttributes::related_view`].
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    pub webview: webkit2gtk::WebView,
    /// The instance of the webview that initiated the new window request.
    ///
    /// The target webview environment **MUST** match the environment of the opener webview. See [`WebviewAttributes::with_environment`].
    #[cfg(windows)]
    pub webview: webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2,
    #[cfg(windows)]
    pub environment:
        webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment,
    /// The instance of the webview that initiated the new window request.
    #[cfg(target_os = "macos")]
    pub webview: objc2::rc::Retained<objc2_web_kit::WKWebView>,
    /// Configuration of the target webview.
    ///
    /// This **MUST** be used when creating the target webview. See [`WebviewAttributes::webview_configuration`].
    #[cfg(target_os = "macos")]
    pub target_configuration:
        objc2::rc::Retained<objc2_web_kit::WKWebViewConfiguration>,
}

/// Window features of a window requested to open.
#[derive(Debug)]
pub struct NewWindowFeatures {
    pub size: Option<dpi::LogicalSize<f64>>,
    pub position: Option<dpi::LogicalPosition<f64>>,
    pub opener: NewWindowOpener,
}

impl NewWindowFeatures {
    pub fn new(
        size: Option<dpi::LogicalSize<f64>>,
        position: Option<dpi::LogicalPosition<f64>>,
        opener: NewWindowOpener,
    ) -> Self {
        Self {
            size,
            position,
            opener,
        }
    }

    /// Specifies the size of the content area
    /// as defined by the user's operating system where the new window will be generated.
    pub fn size(&self) -> Option<dpi::LogicalSize<f64>> {
        self.size
    }

    /// Specifies the position of the window relative to the work area
    /// as defined by the user's operating system where the new window will be generated.
    pub fn position(&self) -> Option<dpi::LogicalPosition<f64>> {
        self.position
    }

    /// Returns information about the webview that initiated a new window request.
    pub fn opener(&self) -> &NewWindowOpener {
        &self.opener
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowWebViewMetaData {
    /// Label des Fensters, zu dem die WebView gehört.
    pub window_label: String,

    /// Eindeutiges Label der WebView.
    pub webview_label: String,
    /// ID des Fensters, zu dem die WebView gehört.
    pub window_id: WindowId,

    /// Eindeutiges ID der WebView.
    pub webview_id: WebViewId,
    /// Native Window-ID des Host-Fensters.
    pub native_id: TaoWindowId,
}

impl WindowWebViewMetaData {
    pub fn new(
        window_id: WindowId,
        webview_id: WebViewId,
        native_id: TaoWindowId,
        window_label: impl Into<String>,
        webview_label: impl Into<String>,
    ) -> crate::error::Result<Self> {
        let window_label = window_label.into();
        let webview_label = webview_label.into();

        if window_label.trim().is_empty() {
            return Err(crate::error::Error::EmptyInitializedWindow(
                window_label,
            ));
        }
        if webview_label.trim().is_empty() {
            return Err(crate::error::Error::EmptyInitializedWebView(
                webview_label,
            ));
        }
        if !is_label_valid(&window_label) {
            return Err(crate::error::Error::InvalidWindowLabel);
        }
        if !is_label_valid(&webview_label) {
            return Err(crate::error::Error::InvalidWindowLabel);
        }

        Ok(Self {
            window_label,
            webview_label,
            window_id,
            webview_id,
            native_id,
        })
    }
}

pub fn is_label_valid(label: &str) -> bool {
    label.chars().all(|c| {
        char::is_alphanumeric(c) || c == '-' || c == '/' || c == ':' || c == '_'
    })
}
/*
pub fn assert_label_is_valid(label: &str) {
    assert!(
        is_label_valid(label),
        "Window label must include only alphanumeric characters, `-`, `/`, `:` and `_`."
    );
}

pub fn parse_proxy_url(url: &Url) -> crate::Result<ProxyConfig> {
    let host = url.host().map(|h| h.to_string()).unwrap_or_default();
    let port = url.port().map(|p| p.to_string()).unwrap_or_default();

    if url.scheme() == "http" {
        let config = ProxyConfig::Http(ProxyEndpoint { host, port });

        Ok(config)
    } else if url.scheme() == "socks5" {
        let config = ProxyConfig::Socks5(ProxyEndpoint { host, port });

        Ok(config)
    } else {
        Err(crate::Error::InvalidProxyUrl)
    }
} */

#[cfg(target_os = "macos")]
pub fn inner_size(
    window: &Window,
    webviews: &[ManagedWebview],
    has_children: bool,
) -> PhysicalSize<u32> {
    if !has_children && !webviews.is_empty() {
        use wry::WebViewExtMacOS;
        let webview = webviews.first().unwrap();
        let view = unsafe {
            Retained::cast_unchecked::<objc2_app_kit::NSView>(webview.webview())
        };
        let view_frame = view.frame();
        let logical: LogicalSize<f64> =
            (view_frame.size.width, view_frame.size.height).into();
        return logical.to_physical(window.scale_factor());
    }

    window.inner_size()
}

#[cfg(not(target_os = "macos"))]
#[allow(unused_variables)]
pub fn inner_size(
    window: &Window,
    webviews: &[ManagedWebview],
    has_children: bool,
) -> PhysicalSize<u32> {
    window.inner_size()
}

/* pub fn to_tao_theme(theme: Option<Theme>) -> Option<TaoTheme> {
    match theme {
        Some(Theme::Light) => Some(TaoTheme::Light),
        Some(Theme::Dark) => Some(TaoTheme::Dark),
        _ => None,
    }
}



#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct WindowIdStore(ArcMutHashMap<TaoWindowId, WindowId>);

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

#[allow(dead_code)]
pub struct WindowsStore(pub RefCell<BTreeMap<WindowId, ManagedWindow>>);

impl fmt::Debug for WindowsStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowsStore").finish_non_exhaustive()
    }
}
pub fn reparent_webview(
    windows: &WindowsStore,
    window_id: WindowId,
    webview_id: WebViewId,
    new_parent_window_id: WindowId,
) -> crate::Result<()> {
    let webview = windows
        .0
        .borrow_mut()
        .get_mut(&window_id)
        .and_then(|window| {
            window
                .webviews
                .iter()
                .position(|webview| webview.metadata.webview_id == webview_id)
                .map(|index| window.webviews.remove(index))
        })
        .ok_or(crate::Error::FailedToSendMessage)?;

    let result = {
        let mut windows_ref = windows.0.borrow_mut();

        let new_parent = windows_ref
            .get_mut(&new_parent_window_id)
            .ok_or(crate::Error::FailedToSendMessage)?;

        let new_parent_window = new_parent
            .inner
            .clone()
            .ok_or(crate::Error::FailedToSendMessage)?;

        #[cfg(target_os = "macos")]
        let reparent_result =
            { webview.inner.reparent(new_parent_window.ns_window() as _) };

        #[cfg(windows)]
        let reparent_result =
            { webview.inner.reparent(new_parent_window.hwnd()) };

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        let reparent_result = {
            use tao::platform::unix::WindowExtUnix;

            if let Some(container) = new_parent_window.default_vbox() {
                webview.inner.reparent(container)
            } else {
                Err(wry::Error::MessageSender)
            }
        };

        match reparent_result {
            Ok(_) => {
                new_parent.webviews.push(webview);
                Ok(())
            }

            Err(e) => {
                log::error!("failed to reparent webview: {e}");
                Err(crate::Error::FailedToSendMessage)
            }
        }
    };

    result
}


 */
