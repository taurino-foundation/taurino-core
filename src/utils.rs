#[cfg(not(target_os = "macos"))]
use crate::schema::PhysicalSize;
use crate::schema::Theme;
use anyhow::{Result, anyhow};
#[cfg_attr(not(windows), allow(unused_imports))]
pub use imp::*;
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(windows)]
use tao::platform::windows::WindowExtWindows;

#[cfg(windows)]
use crate::core::webview::WebView;
#[cfg(not(target_os = "macos"))]
use crate::core::window::Window;

#[cfg(not(target_os = "macos"))]
pub fn inner_size(window: &Window, _webviews: &[WebView], _has_children: bool) -> Result<PhysicalSize<u32>> {
    let size = window.inner_size()?;
    Ok(PhysicalSize::new(size.width, size.height))
}

// ------------------------------------------------------------
// Free functions for existing call sites
// ------------------------------------------------------------
#[cfg(target_os = "macos")]
pub fn reparent_native(webview: &WebView, target: &Arc<tao::window::Window>) -> Result<()> {
    use tao::platform::macos::WindowExtMacOS;
    use wry::WebViewExtMacOS;
    webview
        .inner()
        .reparent(target.ns_window() as _)
        .map_err(|e| anyhow!("reparent failed: {e}"))
}
#[cfg(windows)]
pub fn reparent_native(webview: &WebView, target: &Arc<tao::window::Window>) -> Result<()> {
    use wry::WebViewExtWindows;
    webview
        .inner()
        .reparent(target.hwnd())
        .map_err(|e| anyhow!("reparent failed: {e}"))
}
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub fn reparent_native(webview: &WebView, target: &Arc<tao::window::Window>) -> Result<()> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewExtUnix;
    let container = target
        .default_vbox()
        .ok_or_else(|| anyhow!("target window has no default vbox"))?;
    webview
        .inner()
        .reparent(container)
        .map_err(|e| anyhow!("reparent failed: {e}"))
}
#[cfg(target_os = "macos")]
pub fn inner_size(window: &Window, webviews: &[WebView], has_children: bool) -> PhysicalSize<u32> {
    use wry::WebViewExtMacOS;
    if !has_children {
        if let Some(webview) = webviews.first() {
            let _main_thread =
                MainThreadMarker::new().expect("native view measurement must run on the macOS main thread");
            let native_webview = webview.as_wry().webview();
            // SAFETY:
            // Wry returns its WKWebView subclass.
            // WKWebView is an NSView subclass on macOS.
            // Access occurs after verification on the main thread.
            let view = unsafe { Retained::cast_unchecked::<NSView>(native_webview) };
            let frame = view.frame();
            return LogicalSize::<f64>::new(frame.size.width, frame.size.height).to_physical(window.scale_factor());
        }
    }
    let size = window.inner_size();
    // Explicit conversion avoids a dependency on whether engine_schema
    // and Tao re-export identical types.
    PhysicalSize::new(size.width, size.height)
}

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

#[cfg(not(windows))]
mod imp {}

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
            $crate::utils::get_function_impl($lib, concat!(stringify!($func), '\0'))
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
    type GetSystemMetricsForDpi = unsafe extern "system" fn(nindex: SYSTEM_METRICS_INDEX, dpi: u32) -> i32;

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
