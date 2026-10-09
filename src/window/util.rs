use std::{
  collections::{
    HashMap, HashSet,
    hash_map::Entry::{Occupied, Vacant},
  },
  path::PathBuf,
  sync::MutexGuard,
};
#[cfg(windows)]
use tao::platform::windows::WindowExtWindows;

use anyhow::Result;
use dpi::LogicalPosition;
#[cfg_attr(not(windows), allow(unused_imports))]
pub use imp::*;
use tao::window::Window;
use wry::{WebContext as WryContext, WebViewBuilder};

use crate::{
  schema::{
    Rect,
    webview::{InitializationScript, WebviewBounds},
  },
  tools::{
    lock_state,
    stores::{WebContext, WebContextStore},
    wrappers::RectWrapper,
  },
};
#[cfg(not(windows))]
mod imp {}
#[cfg(any(
  windows,
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "linux",
  target_os = "netbsd",
  target_os = "openbsd",
))]
use crate::window::undecorated_resizing;
#[cfg(windows)]
mod imp {
  use std::{iter::once, os::windows::ffi::OsStrExt};

  use once_cell::sync::Lazy;
  use windows::{
    Win32::{
      Foundation::*,
      Graphics::Gdi::*,
      System::LibraryLoader::{GetProcAddress, LoadLibraryW},
      UI::{HiDpi::*, WindowsAndMessaging::*},
    },
    core::{HRESULT, PCSTR, PCWSTR},
  };

  pub fn encode_wide(string: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    string.as_ref().encode_wide().chain(once(0)).collect()
  }

  // Helper function to dynamically load function pointer.
  // `library` and `function` must be zero-terminated.
  pub(super) fn get_function_impl(library: &str, function: &str) -> FARPROC {
    let library = encode_wide(library);
    assert_eq!(function.chars().last(), Some('\0'));

    // Library names we will use are ASCII so we can use the A version to avoid string conversion.
    let module = unsafe { LoadLibraryW(PCWSTR::from_raw(library.as_ptr())) }.unwrap_or_default();
    if module.is_invalid() {
      return None;
    }

    unsafe { GetProcAddress(module, PCSTR::from_raw(function.as_ptr())) }
  }

  macro_rules! get_function {
    ($lib:expr, $func:ident) => {
      $crate::window::util::get_function_impl($lib, concat!(stringify!($func), '\0'))
        .map(|f| unsafe { std::mem::transmute::<_, $func>(f) })
    };
  }

  type GetDpiForWindow = unsafe extern "system" fn(hwnd: HWND) -> u32;
  type GetDpiForMonitor = unsafe extern "system" fn(
    hmonitor: HMONITOR,
    dpi_type: MONITOR_DPI_TYPE,
    dpi_x: *mut u32,
    dpi_y: *mut u32,
  ) -> HRESULT;
  type GetSystemMetricsForDpi =
    unsafe extern "system" fn(nindex: SYSTEM_METRICS_INDEX, dpi: u32) -> i32;

  static GET_DPI_FOR_WINDOW: Lazy<Option<GetDpiForWindow>> =
    Lazy::new(|| get_function!("user32.dll", GetDpiForWindow));
  static GET_DPI_FOR_MONITOR: Lazy<Option<GetDpiForMonitor>> =
    Lazy::new(|| get_function!("shcore.dll", GetDpiForMonitor));
  static GET_SYSTEM_METRICS_FOR_DPI: Lazy<Option<GetSystemMetricsForDpi>> =
    Lazy::new(|| get_function!("user32.dll", GetSystemMetricsForDpi));

  #[allow(non_snake_case)]
  pub unsafe fn hwnd_dpi(hwnd: HWND) -> u32 {
    unsafe {
      if let Some(GetDpiForWindow) = *GET_DPI_FOR_WINDOW {
        // We are on Windows 10 Anniversary Update (1607) or later.
        match GetDpiForWindow(hwnd) {
          0 => USER_DEFAULT_SCREEN_DPI, // 0 is returned if hwnd is invalid
          dpi => dpi,
        }
      } else if let Some(GetDpiForMonitor) = *GET_DPI_FOR_MONITOR {
        // We are on Windows 8.1 or later.
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_invalid() {
          return USER_DEFAULT_SCREEN_DPI;
        }

        let mut dpi_x = 0;
        let mut dpi_y = 0;
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
          dpi_x
        } else {
          USER_DEFAULT_SCREEN_DPI
        }
      } else {
        // We are on Vista or later.
        if IsProcessDPIAware().as_bool() {
          let hdc = GetDC(Some(hwnd));
          if hdc.is_invalid() {
            return USER_DEFAULT_SCREEN_DPI;
          }
          // If the process is DPI aware, then scaling must be handled by the application using
          // this DPI value.
          let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSX) as u32;
          ReleaseDC(Some(hwnd), hdc);
          dpi
        } else {
          // If the process is DPI unaware, then scaling is performed by the OS; we thus return
          // 96 (scale factor 1.0) to prevent the window from being re-scaled by both the
          // application and the WM.
          USER_DEFAULT_SCREEN_DPI
        }
      }
    }
  }

  #[allow(non_snake_case)]
  pub unsafe fn get_system_metrics_for_dpi(nindex: SYSTEM_METRICS_INDEX, dpi: u32) -> i32 {
    unsafe {
      if let Some(GetSystemMetricsForDpi) = *GET_SYSTEM_METRICS_FOR_DPI {
        GetSystemMetricsForDpi(nindex, dpi)
      } else {
        GetSystemMetrics(nindex)
      }
    }
  }
}

pub fn all_initialization_scripts(
  window_label: &str,
  webview_label: &str,
  use_https_scheme: bool,
  scripts_capacity: Option<usize>,
) -> Result<Vec<InitializationScript>> {
  fn main_frame_script(script: impl Into<String>) -> InitializationScript {
    InitializationScript {
      script: script.into(),
      for_main_frame_only: true,
    }
  }

  let mut scripts = Vec::with_capacity(scripts_capacity.unwrap_or(3));

  // ----------------------------------------------------------
  // 1. Initialize global namespace
  // ----------------------------------------------------------
  /*
  webview.evaluate_script(
      "window.__TAURINO_INTERNALS__.runCallback(0, 30)"
  )?;
  */
  scripts.push(main_frame_script(
    r#"
        (() => {
            "use strict";

            const global = window;

            if (!Object.hasOwn(global, "__TAURINO_INTERNALS__")) {
                Object.defineProperty(global, "__TAURINO_INTERNALS__", {
                    value: Object.create(null),
                    writable: false,
                    configurable: false,
                    enumerable: false
                });
            }

            const internals = global.__TAURINO_INTERNALS__;

            if (!internals || typeof internals !== "object") {
                throw new TypeError("Invalid Taurino internals");
            }

            if (!Object.hasOwn(internals, "plugins")) {
                Object.defineProperty(internals, "plugins", {
                    value: Object.create(null),
                    writable: false,
                    configurable: false,
                    enumerable: true
                });
            }

            if (!Object.hasOwn(global, "isTaurino")) {
                Object.defineProperty(global, "isTaurino", {
                    value: true,
                    writable: false,
                    configurable: false,
                    enumerable: true
                });
            }
        })();
        "#,
  ));

  // ----------------------------------------------------------
  // 2. Initialize immutable metadata
  // ----------------------------------------------------------

  let os = std::env::consts::OS;
  let family = std::env::consts::FAMILY;
  let arch = std::env::consts::ARCH;
  let pid = std::process::id();

  let metadata = serde_json::json!({
      "currentWindow": {
          "label": window_label,
      },
      "currentWebview": {
          "label": webview_label,
      },
      "process": {
          "pid": pid,
      },
      "platform": {
          "os": os,
          "family": family,
          "arch": arch,
          "target": format!("{arch}-{os}"),
      },
  });

  let metadata_json = serde_json::to_string(&metadata)?;

  let metadata_script = r#"
        (() => {
            "use strict";

            const internals = window.__TAURINO_INTERNALS__;
            const metadata = __TAURINO_METADATA_JSON__;

            const deepFreeze = (value) => {
                if (value && typeof value === "object") {
                    for (const key of Object.keys(value)) {
                        deepFreeze(value[key]);
                    }
                    Object.freeze(value);
                }
                return value;
            };

            Object.defineProperty(internals, "metadata", {
                value: deepFreeze(metadata),
                writable: false,
                configurable: false,
                enumerable: true
            });
        })();
    "#
  .replace("__TAURINO_METADATA_JSON__", &metadata_json);

  scripts.push(main_frame_script(metadata_script));

  // ----------------------------------------------------------
  // 3. Initialize core APIs and IPC
  // ----------------------------------------------------------

  let scheme = if use_https_scheme { "https" } else { "http" };

  let core_script = r#"
        (() => {
            "use strict";

            const internals = window.__TAURINO_INTERNALS__;

            const osName = __TAURINO_OS_JSON__;
            const protocolScheme = __TAURINO_SCHEME_JSON__;

            // --------------------------------------------------
            // Callback registry
            // --------------------------------------------------

            const callbacks = new Map();
            let nextCallbackId = 0;

            function uid() {
                let id;

                do {
                    id = nextCallbackId;
                    nextCallbackId =
                        (nextCallbackId + 1) >>> 0;
                } while (callbacks.has(id));

                return id;
            }

            function unregisterCallback(id) {
                return callbacks.delete(id);
            }

            function registerCallback(callback, once = false) {
                if (typeof callback !== "function") {
                    throw new TypeError(
                        "Callback must be a function"
                    );
                }

                const id = uid();

                callbacks.set(id, (data) => {
                    if (once) {
                        unregisterCallback(id);
                    }

                    return callback(data);
                });

                return id;
            }

            function runCallback(id, data) {
                const callback = callbacks.get(id);

                if (callback) {
                    return callback(data);
                }

                console.warn(
                    `[TAURINO] Callback ${id} was not found`
                );
            }

            // --------------------------------------------------
            // File URL conversion
            // --------------------------------------------------

            function convertFileSrc(filePath, protocol = "asset") {
                if (typeof filePath !== "string") {
                    throw new TypeError(
                        "filePath must be a string"
                    );
                }

                const path = encodeURIComponent(filePath);

                if (osName === "windows" || osName === "android") {
                    return `${protocolScheme}://${protocol}.localhost/${path}`;
                }

                return `${protocol}://localhost/${path}`;
            }

            // --------------------------------------------------
            // IPC queue
            // --------------------------------------------------

            const ipcQueue = [];
            let isWaitingForIpc = false;

            function hasIpc() {
                return typeof internals.ipc === "function";
            }

            function flushIpcQueue() {
                if (!hasIpc()) {
                    return false;
                }

                isWaitingForIpc = false;

                while (ipcQueue.length > 0) {
                    const action = ipcQueue.shift();

                    try {
                        action();
                    } catch (error) {
                        console.error(
                            "[TAURINO] IPC action failed:",
                            error
                        );
                    }
                }

                return true;
            }

            function waitForIpc() {
                if (flushIpcQueue()) {
                    return;
                }

                setTimeout(waitForIpc, 50);
            }

            function enqueueIpc(action) {
                if (hasIpc() && ipcQueue.length === 0) {
                    action();
                    return;
                }

                ipcQueue.push(action);

                if (hasIpc()) {
                    flushIpcQueue();
                    return;
                }

                if (!isWaitingForIpc) {
                    isWaitingForIpc = true;
                    setTimeout(waitForIpc, 50);
                }
            }

            // --------------------------------------------------
            // Invoke API
            // --------------------------------------------------

            function invoke(cmd, payload = {}, options) {
                return new Promise((resolve, reject) => {
                    let successId;
                    let errorId;

                    successId = registerCallback((data) => {
                        unregisterCallback(errorId);
                        resolve(data);
                    }, true);

                    errorId = registerCallback((error) => {
                        unregisterCallback(successId);
                        reject(error);
                    }, true);

                    const action = () => {
                        try {
                            internals.ipc({
                                cmd,
                                callback: successId,
                                error: errorId,
                                payload,
                                options
                            });
                        } catch (error) {
                            unregisterCallback(successId);
                            unregisterCallback(errorId);
                            reject(error);
                        }
                    };

                    enqueueIpc(action);
                });
            }

            // --------------------------------------------------
            // Register internal APIs
            // --------------------------------------------------

            const defineApi = (name, value) => {
                Object.defineProperty(internals, name, {
                    value,
                    writable: false,
                    configurable: false,
                    enumerable: true
                });
            };

            defineApi("convertFileSrc", convertFileSrc);
            defineApi("transformCallback", registerCallback);
            defineApi("unregisterCallback", unregisterCallback);
            defineApi("runCallback", runCallback);
            defineApi("invoke", invoke);

            // Debug registry: do not expose in production
            // defineApi("callbacks", callbacks);
        })();
    "#
  .replace("__TAURINO_OS_JSON__", &serde_json::to_string(os)?)
  .replace("__TAURINO_SCHEME_JSON__", &serde_json::to_string(scheme)?);

  scripts.push(main_frame_script(core_script));

  Ok(scripts)
}

pub fn apply_webview_bounds<'a>(
  window: &'a Window,
  mut webview_builder: WebViewBuilder<'a>,
  bounds: Option<Rect>,
  auto_resize: bool,
  child: bool,
) -> (Option<WebviewBounds>, WebViewBuilder<'a>) {
  let webview_bounds = if let Some(bounds) = bounds {
    let bounds: RectWrapper = bounds.into();
    let bounds = bounds.0;
    let scale_factor = window.scale_factor();
    let position = bounds.position.to_logical::<f32>(scale_factor);
    let size = bounds.size.to_logical::<f32>(scale_factor);
    webview_builder = webview_builder.with_bounds(bounds);
    let window_size = window.inner_size().to_logical::<f32>(scale_factor);
    if auto_resize {
      Some(WebviewBounds {
        x_rate: position.x / window_size.width,
        y_rate: position.y / window_size.height,
        width_rate: size.width / window_size.width,
        height_rate: size.height / window_size.height,
      })
    } else {
      None
    }
  } else {
    if child {
      webview_builder = webview_builder.with_bounds(wry::Rect {
        position: LogicalPosition::new(0, 0).into(),
        size: window.inner_size().into(),
      });
      Some(WebviewBounds {
        x_rate: 0.,
        y_rate: 0.,
        width_rate: 1.,
        height_rate: 1.,
      })
    } else {
      None
    }
  };
  (webview_bounds, webview_builder)
}

pub fn apply_webview_context<'a>(
  env_var: &str,
  webview_label: String,
  browser_context: &'a WebContextStore,
  data_directory: Option<PathBuf>,
) -> Result<(
  MutexGuard<'a, HashMap<Option<PathBuf>, WebContext>>,
  Option<PathBuf>,
  Option<PathBuf>,
)> {
  let mut contexts = lock_state(browser_context, "browser_context")?;

  let is_first_context = contexts.is_empty();

  // Identisch zum Original
  let automation_enabled = std::env::var(env_var).as_deref() == Ok("true");

  let web_context_key = data_directory;

  match contexts.entry(web_context_key.clone()) {
    Occupied(occupied) => {
      let occupied = occupied.into_mut();

      occupied.referenced_by_webviews.insert(webview_label);
    }

    Vacant(vacant) => {
      let mut web_context = WryContext::new(web_context_key.clone());

      web_context.set_allows_automation(if automation_enabled {
        is_first_context
      } else {
        false
      });

      vacant.insert(WebContext {
        inner: web_context,
        referenced_by_webviews: [webview_label].into(),
        registered_custom_protocols: HashSet::new(),
      });
    }
  }

  // Exakt dieselbe Logik wie im Original
  let context_key = if automation_enabled {
    None
  } else {
    web_context_key.clone()
  };

  Ok((contexts, web_context_key, context_key))
}

pub fn apply_build_webview<'a>(
  window: &Window,
  webview_builder: wry::WebViewBuilder<'a>,
  webview_label: &str,
  child: bool,
) -> Result<wry::WebView> {
  let webview = match child {
    #[cfg(not(any(
      target_os = "windows",
      target_os = "macos",
      target_os = "ios",
      target_os = "android"
    )))]
    true => {
      let vbox = window.default_vbox().ok_or_else(|| {
        anyhow::anyhow!(
          "failed to create child WebView `{}`: \
                     window does not provide a GTK default vbox",
          options.label
        )
      })?;
      webview_builder.build_gtk(vbox)
    }
    #[cfg(any(
      target_os = "windows",
      target_os = "macos",
      target_os = "ios",
      target_os = "android"
    ))]
    true => webview_builder.build_as_child(window),
    false => {
      #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
      ))]
      let builder = webview_builder.build(window);
      #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
      )))]
      let builder = {
        let vbox = window.default_vbox().ok_or_else(|| {
          anyhow::anyhow!(
            "failed to create WebView `{}`: \
                         window does not provide a GTK default vbox",
            webview_label
          )
        })?;
        webview_builder.build_gtk(vbox)
      };
      builder
    }
  }
  .map_err(|error| anyhow::anyhow!("failed to build WebView `{}`: {error}", webview_label))?;
  if child == false {
    #[cfg(any(
      target_os = "linux",
      target_os = "dragonfly",
      target_os = "freebsd",
      target_os = "netbsd",
      target_os = "openbsd"
    ))]
    undecorated_resizing::attach_resize_handler(&webview);

    #[cfg(windows)]
    if window.is_resizable() && !window.is_decorated() {
      undecorated_resizing::attach_resize_handler(window.hwnd(), window.has_undecorated_shadow());
    }
  }

  Ok(webview)
}

/*
pub(crate) const PROCESS_IPC_MESSAGE_FN: &str = include_str!("./scripts/process-ipc-message-fn.js");
#[derive(Template)]
#[default_template("./scripts/init.js")]
struct InitJavascript<'a> {
  #[raw]
  pattern_script: &'a str,
  #[raw]
  ipc_script: &'a str,
  #[raw]
  core_script: &'a str,
  #[raw]
  event_initialization_script: &'a str,
  #[raw]
  freeze_prototype: &'a str,
}
#[derive(Template)]
#[default_template("./scripts/core.js")]
struct CoreJavascript<'a> {
  os_name: &'a str,
  protocol_scheme: &'a str,
  invoke_key: &'a str,
} */

/*
 */

/*
fn new_window_handler(
    policy: &NewWindowPolicy,
    url: Url,
    features: NewWindowFeatures,
    engine_manager: Arc<EngineManager>,
) -> Result<NewWindowResponse> {
    match policy.evaluate(&url) {
        NewWindowAction::Allow => Ok(NewWindowResponse::Allow),

        NewWindowAction::Deny => Ok(NewWindowResponse::Deny),

        NewWindowAction::Create { window } => {
            let mut window_options = *window;

            // requested URL in die Konfiguration des neuen WebViews übernehmen
            //
            // z. B.:
            // window_options.webview.url = WebviewUrl::External(url);

            // window.open()-Features ggf. übernehmen
            //
            // if let Some(size) = features.size() {
            //     window_options.width = Some(size.width);
            //     window_options.height = Some(size.height);
            // }
            //
            // if let Some(position) = features.position() {
            //     window_options.x = Some(position.x);
            //     window_options.y = Some(position.y);
            // }

            let window_id = engine_manager.create_window_from_new_window_request(
                window_options,
                features,
            )?;

            Ok(NewWindowResponse::Create { window_id })
        }
    }
}





*/

/*

new_window_handler




NewWindowAction::Create { window } => {
    let mut window_options = *window;

    // requested URL setzen
    // window_options.webview.url = WebviewUrl::External(url);

    let (tx, rx) = std::sync::mpsc::channel();

    engine_manager
        .proxy()?
        .send_event(Message::CreateWindow(CreateWindowRequest {
            options: window_options,
            response: tx,
        }))
        .map_err(|_| anyhow!("failed to send CreateWindow request"))?;

    let window_id = rx
        .recv()
        .map_err(|_| anyhow!("CreateWindow response channel closed"))??;

    Ok(NewWindowResponse::Create { window_id })
}


eventloop



match event {
    Event::UserEvent(Message::CreateWindow(request)) => {
        let result = {
            let mut window_manager = engine_manager.window_mut()?;

            window_manager.open_window(
                &request.options,
                event_loop_target,
            )
        };

        let _ = request.response.send(result);
    }

    // ...
}


*/
