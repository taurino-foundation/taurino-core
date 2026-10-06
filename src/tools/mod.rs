pub mod environment;
pub mod image;
pub mod resources;
pub mod stores;
use crate::schema::FrontendDist;
use crate::schema::Theme;
use crate::schema::webview::BackgroundThrottlingPolicy;
use anyhow::{Result, anyhow};
use dpi::Position;
use http::{
  Request, Response as HttpResponse, StatusCode,
  header::{ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE},
};
pub mod logging;
use std::borrow::Cow;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use tao::monitor::MonitorHandle;
pub mod wrappers;
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
