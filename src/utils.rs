use crate::schema::FrontendDist;
#[cfg(target_os = "macos")]
use crate::schema::LogicalSize;
use crate::schema::Theme;
use crate::schema::webview::BackgroundThrottlingPolicy;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::schema::window::WindowId;
use anyhow::{Result, anyhow};
use dpi::Position;
use http::{
  Request, Response as HttpResponse, StatusCode,
  header::{ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE},
};

use std::borrow::Cow;
use std::ffi::OsString;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use tao::monitor::MonitorHandle;

use url::Url;

/// Maps a Taurino theme value to Tao's native window theme.
pub fn map_theme(theme: Theme) -> tao::window::Theme {
  match theme {
    Theme::Light => tao::window::Theme::Light,

    Theme::Dark => tao::window::Theme::Dark,

    #[allow(unreachable_patterns)]
    _ => tao::window::Theme::Light,
  }
}
pub type ArcMut<T> = Arc<Mutex<T>>;
pub fn arc_mut<T>(t: T) -> ArcMut<T> {
  Arc::new(Mutex::new(t))
}

/// Converts PoisonError into a custom error without taking over its guard.

/// Locks window state and converts mutex poisoning into an engine error.
pub fn lock_state<'a, T>(mutex: &'a Mutex<T>, name: &str) -> Result<MutexGuard<'a, T>> {
  mutex.lock().map_err(|_| anyhow!("Window {name} mutex is poisoned"))
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
  pub environment: webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment,
  /// The instance of the webview that initiated the new window request.
  #[cfg(target_os = "macos")]
  pub webview: objc2::rc::Retained<objc2_web_kit::WKWebView>,
  /// Configuration of the target webview.
  ///
  /// This **MUST** be used when creating the target webview. See [`WebviewAttributes::webview_configuration`].
  #[cfg(target_os = "macos")]
  pub target_configuration: objc2::rc::Retained<objc2_web_kit::WKWebViewConfiguration>,
}

/// Window features of a window requested to open.
#[derive(Debug)]
pub struct NewWindowFeatures {
  pub(crate) size: Option<dpi::LogicalSize<f64>>,
  pub(crate) position: Option<dpi::LogicalPosition<f64>>,
  pub(crate) opener: NewWindowOpener,
}

impl NewWindowFeatures {
  pub fn new(
    size: Option<dpi::LogicalSize<f64>>,
    position: Option<dpi::LogicalPosition<f64>>,
    opener: NewWindowOpener,
  ) -> Self {
    Self { size, position, opener }
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

pub fn find_monitor_for_position(
  monitors: impl Iterator<Item = MonitorHandle>,
  window_position: Position,
) -> Option<MonitorHandle> {
  monitors.into_iter().find(|m| {
    let monitor_pos = m.position();
    let monitor_size = m.size();

    // type annotations required for 32bit targets.
    let window_position = window_position.to_physical::<i32>(m.scale_factor());

    monitor_pos.x <= window_position.x
      && window_position.x < monitor_pos.x + monitor_size.width as i32
      && monitor_pos.y <= window_position.y
      && window_position.y < monitor_pos.y + monitor_size.height as i32
  })
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
/// Permission types that can be requested by the webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PermissionKind {
  /// Microphone access permission.
  Microphone,
  /// Camera access permission.
  Camera,
  /// Geolocation access permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION`.
  /// - **Linux**: Supported via `GeolocationPermissionRequest`.
  /// - **Android**: Supported via `WebChromeClient.onGeolocationPermissionsShowPrompt`.
  /// - **macOS / iOS**: Not yet supported by platform backends.
  Geolocation,
  /// Notifications permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS`.
  /// - **Linux**: Supported via `NotificationPermissionRequest`.
  /// - **macOS / Android / iOS**: Not yet supported by platform backends.
  Notifications,
  /// Clipboard read permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ`.
  /// - **macOS / Linux / Android / iOS**: Not yet supported by platform backends.
  ClipboardRead,
  /// Display capture permission (for getDisplayMedia).
  DisplayCapture,
  /// Midi access permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_MIDI_SYSTEM_EXCLUSIVE_MESSAGES`.
  /// - **Android**: Supported via `android.webkit.resource.MIDI_SYSEX`.
  /// - **macOS / Linux / iOS**: Not yet supported by platform backends.
  Midi,
  /// Sensors (accelerometer, gyroscope, etc.) access permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_OTHER_SENSORS`.
  /// - **macOS / Linux / Android / iOS**: Not yet supported by platform backends.
  Sensors,
  /// Media key system access permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Android**: Supported via `android.webkit.resource.PROTECTED_MEDIA_ID`.
  /// - **Windows / macOS / Linux / iOS**: Not yet supported by platform backends.
  MediaKeySystemAccess,
  /// Local fonts access permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_LOCAL_FONTS`.
  /// - **macOS / Linux / Android / iOS**: Not yet supported by platform backends.
  LocalFonts,
  /// Window management permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_WINDOW_MANAGEMENT`.
  /// - **macOS / Linux / Android / iOS**: Not yet supported by platform backends.
  WindowManagement,
  /// Pointer lock permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Linux**: Supported via `PointerLockPermissionRequest`.
  /// - **Windows / macOS / Android / iOS**: Not yet supported by platform backends.
  PointerLock,
  /// Automatic downloads permission (multiple downloads without user interaction).
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_MULTIPLE_AUTOMATIC_DOWNLOADS`.
  /// - **macOS / Linux / Android / iOS**: Not yet supported by platform backends.
  AutomaticDownloads,
  /// File system access permission (read/write via File System Access API).
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_FILE_READ_WRITE`.
  /// - **macOS / Linux / Android / iOS**: Not yet supported by platform backends.
  FileSystemAccess,
  /// Media autoplay permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY`.
  /// - **macOS / Linux / Android / iOS**: Not yet supported by platform backends.
  Autoplay,
  /// Other unrecognized permission type.
  Other,
}

impl std::fmt::Display for PermissionKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Microphone => write!(f, "microphone"),
      Self::Camera => write!(f, "camera"),
      Self::Geolocation => write!(f, "geolocation"),
      Self::Notifications => write!(f, "notifications"),
      Self::ClipboardRead => write!(f, "clipboard-read"),
      Self::DisplayCapture => write!(f, "display-capture"),
      Self::Midi => write!(f, "midi"),
      Self::Sensors => write!(f, "sensors"),
      Self::MediaKeySystemAccess => write!(f, "media-key-system-access"),
      Self::LocalFonts => write!(f, "local-fonts"),
      Self::WindowManagement => write!(f, "window-management"),
      Self::PointerLock => write!(f, "pointer-lock"),
      Self::AutomaticDownloads => write!(f, "automatic-downloads"),
      Self::FileSystemAccess => write!(f, "file-system-access"),
      Self::Autoplay => write!(f, "autoplay"),
      Self::Other => write!(f, "other"),
    }
  }
}

/// Response for permission requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PermissionResponse {
  /// Grant the permission.
  ///
  /// ## Platform-specific
  ///
  /// - **Android**: Not supported for runtime permissions; the normal Android
  ///   permission flow is used instead.
  Allow,
  /// Deny the permission.
  Deny,
  /// Use the platform or browser default behavior.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows / macOS / Android**: The default behavior is to continue the
  ///   platform or browser permission flow.
  /// - **Linux**: The default behavior is [`Self::Deny`]
  #[default]
  Default,
}

impl std::fmt::Display for PermissionResponse {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Allow => write!(f, "allow"),
      Self::Deny => write!(f, "deny"),
      Self::Default => write!(f, "default"),
    }
  }
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
    wry::PermissionKind::MediaKeySystemAccess => PermissionKind::MediaKeySystemAccess,
    wry::PermissionKind::LocalFonts => PermissionKind::LocalFonts,
    wry::PermissionKind::WindowManagement => PermissionKind::WindowManagement,
    wry::PermissionKind::PointerLock => PermissionKind::PointerLock,
    wry::PermissionKind::AutomaticDownloads => PermissionKind::AutomaticDownloads,
    wry::PermissionKind::FileSystemAccess => PermissionKind::FileSystemAccess,
    wry::PermissionKind::Autoplay => PermissionKind::Autoplay,
    wry::PermissionKind::Other => PermissionKind::Other,
    _ => PermissionKind::Other,
  }
}

pub fn to_wry_permission_response(response: PermissionResponse) -> wry::PermissionResponse {
  match response {
    PermissionResponse::Allow => wry::PermissionResponse::Allow,
    PermissionResponse::Deny => wry::PermissionResponse::Deny,
    PermissionResponse::Default => wry::PermissionResponse::Default,
  }
}

/// Response for the new window request handler.
pub enum NewWindowResponse {
  /// Allow the window to be opened with the default implementation.
  Allow,
  /// Allow the window to be opened, with the given window.
  ///
  /// ## Platform-specific:
  ///
  /// **Linux**: The webview must be related to the caller webview. See [`WebviewAttributes::related_view`].
  /// **Windows**: The webview must use the same environment as the caller webview. See [`WebviewAttributes::with_environment`].
  #[cfg(not(any(target_os = "android", target_os = "ios")))]
  Create { window_id: WindowId },
  /// Deny the window from being opened.
  Deny,
}

pub fn map_background_throttling(throttling: BackgroundThrottlingPolicy) -> wry::BackgroundThrottlingPolicy {
  match throttling {
    BackgroundThrottlingPolicy::Disabled => wry::BackgroundThrottlingPolicy::Disabled,
    BackgroundThrottlingPolicy::Suspend => wry::BackgroundThrottlingPolicy::Suspend,
    BackgroundThrottlingPolicy::Throttle => wry::BackgroundThrottlingPolicy::Throttle,
  }
}

pub fn get_app_url(https: bool, frontend_dist: &FrontendDist) -> Cow<'_, Url> {
  match frontend_dist {
    FrontendDist::Url(url) => Cow::Borrowed(url),
    _ => taurino_protocol_url(https),
  }
}
#[allow(dead_code)]
pub fn taurino_protocol_url<'a>(https: bool) -> Cow<'a, Url> {
  if cfg!(windows) || cfg!(target_os = "android") {
    let scheme = if https { "https" } else { "http" };
    Cow::Owned(Url::parse(&format!("{scheme}://taurino.localhost")).unwrap())
  } else {
    Cow::Owned(Url::parse("taurino://localhost").unwrap())
  }
}
pub fn is_local_network_url(url: &url::Url) -> bool {
  match url.host() {
    Some(url::Host::Domain(s)) => s == "localhost",
    Some(url::Host::Ipv4(_)) | Some(url::Host::Ipv6(_)) => true,
    None => false,
  }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Env {
  #[cfg(target_os = "linux")]
  pub appimage: Option<std::ffi::OsString>,
  #[cfg(target_os = "linux")]
  pub appdir: Option<std::ffi::OsString>,
  pub args_os: Vec<OsString>,
}

#[allow(clippy::derivable_impls)]
impl Default for Env {
  fn default() -> Self {
    let args_os = std::env::args_os().collect();
    #[cfg(target_os = "linux")]
    {
      let env = Self {
        #[cfg(target_os = "linux")]
        appimage: std::env::var_os("APPIMAGE"),
        #[cfg(target_os = "linux")]
        appdir: std::env::var_os("APPDIR"),
        args_os,
      };
      if env.appimage.is_some() || env.appdir.is_some() {
        let is_temp = std::env::current_exe()
          .map(|p| {
            p.display()
              .to_string()
              .starts_with(&format!("{}/.mount_", std::env::temp_dir().display()))
          })
          .unwrap_or(true);

        if !is_temp {
          log::warn!(
            "`APPDIR` or `APPIMAGE` environment variable found but this application was not detected as an AppImage; this might be a security issue."
          );
        }
      }
      env
    }
    #[cfg(not(target_os = "linux"))]
    {
      Self { args_os }
    }
  }
}

/*
>> git add -A
>> git commit -m "map_background_throttling"
>> git pull --rebase origin main
>> git push

*/

pub fn request_path(request: &Request<Vec<u8>>, proxy_dev_server: bool) -> String {
  let uri = request.uri().to_string();

  let path = if proxy_dev_server {
    uri
  } else {
    uri.split(['?', '#']).next().unwrap_or_default().to_string()
  };

  strip_app_origin(&path)
}

pub fn strip_app_origin(value: &str) -> String {
  let custom_origin = format!("taurino://localhost");
  let http_origin = format!("http://taurino.localhost");
  let https_origin = format!("https://taurino.localhost");

  value
    .strip_prefix(&custom_origin)
    .or_else(|| value.strip_prefix(&http_origin))
    .or_else(|| value.strip_prefix(&https_origin))
    .unwrap_or(value)
    .to_string()
}

pub fn safe_asset_path(root: &Path, uri_path: &str) -> Option<PathBuf> {
  let clean_path = uri_path
    .trim_start_matches('/')
    .split(['?', '#'])
    .next()
    .unwrap_or_default();

  let mut output = root.to_path_buf();

  if clean_path.is_empty() {
    output.push("index.html");
    return Some(output);
  }

  for component in Path::new(clean_path).components() {
    match component {
      Component::Normal(part) => output.push(part),
      Component::CurDir => {}
      _ => return None,
    }
  }

  Some(output)
}

pub fn empty_response(status: StatusCode, window_origin: &str) -> Result<HttpResponse<Cow<'static, [u8]>>> {
  Ok(
    HttpResponse::builder()
      .status(status)
      .header(ACCESS_CONTROL_ALLOW_ORIGIN, window_origin)
      .header(ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, PUT, PATCH, DELETE, OPTIONS")
      .header(ACCESS_CONTROL_ALLOW_HEADERS, "*")
      .body(Cow::Owned(Vec::new()))?,
  )
}

pub fn error_response(status: StatusCode, window_origin: &str, message: &str) -> HttpResponse<Cow<'static, [u8]>> {
  HttpResponse::builder()
    .status(status)
    .header(CONTENT_TYPE, mime::TEXT_PLAIN.essence_str())
    .header(ACCESS_CONTROL_ALLOW_ORIGIN, window_origin)
    .body(Cow::Owned(message.as_bytes().to_vec()))
    .unwrap()
}

pub fn should_forward_request_header(name: &str) -> bool {
  !matches!(
    name.to_ascii_lowercase().as_str(),
    "host" | "connection" | "content-length" | "transfer-encoding" | "upgrade"
  )
}

pub fn should_forward_response_header(name: &str) -> bool {
  !matches!(
    name.to_ascii_lowercase().as_str(),
    "connection" | "content-length" | "transfer-encoding" | "upgrade"
  )
}
