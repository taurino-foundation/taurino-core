mod error;
pub mod image;
mod menu;
mod platform;
pub mod resource;
mod tray;
mod types;
mod utils;
mod webview;
mod window;

pub mod prelude {
    // -------------------------------------------------------------------------
    // Error
    // -------------------------------------------------------------------------

    pub use super::error::{Error, Result};

    // -------------------------------------------------------------------------
    // Menu
    // -------------------------------------------------------------------------

    pub use super::menu::prelude::*;

    // -------------------------------------------------------------------------
    // Tray
    // -------------------------------------------------------------------------

    pub use super::tray::prelude::*;

    // -------------------------------------------------------------------------
    // Utils
    // -------------------------------------------------------------------------

    pub use super::utils::{
        assert_label_is_valid, find_monitor_for_position, from_wry_permission_kind, inner_size,
        is_label_valid, map_theme, parse_proxy_url, to_wry_permission_response, ArcMut,
        ArcMutHashMap, Icon, NewWindowFeatures, NewWindowOpener, RawWindow, WebContext,
        WindowWebViewMetaData,
    };

    #[cfg(target_os = "android")]
    pub use super::utils::CreationContext;

    // -------------------------------------------------------------------------
    // WebView
    // -------------------------------------------------------------------------

    pub use super::webview::{ManagedWebview, WebViewBuilder, WebViewId};

    // -------------------------------------------------------------------------
    // Window
    // -------------------------------------------------------------------------

    pub use super::window::{ManagedWindow, WindowBuilder, WindowId, WindowMenu};

    // -------------------------------------------------------------------------
    // Types
    // -------------------------------------------------------------------------

    pub use super::types::*;

    // -------------------------------------------------------------------------
    // Platform
    // -------------------------------------------------------------------------

    pub use super::platform::{
        dialog::error,
        monitor::MonitorExt,
        webview::Webview,
        window::{calculate_window_center_position, WindowExt},
    };

    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    pub use super::platform::undecorated_resizing::{
        attach_resize_handler, detach_resize_handler, update_drag_hwnd_rgn_for_undecorated,
    };

    #[cfg(windows)]
    pub use super::platform::util::{encode_wide, get_system_metrics_for_dpi, hwnd_dpi};
}
pub mod native {
    // -------------------------------------------------------------------------
    // Cross-platform
    // -------------------------------------------------------------------------

    pub use dpi;
    pub use raw_window_handle;
    pub use tao;
    pub use wry;

    // -------------------------------------------------------------------------
    // Desktop
    // -------------------------------------------------------------------------

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "windows",
        target_os = "macos",
    ))]
    pub mod desktop {
        pub use global_hotkey;
        pub use muda;
        pub use tray_icon;
    }

    // -------------------------------------------------------------------------
    // Linux / BSD
    // -------------------------------------------------------------------------

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    pub mod linux {
        pub use gtk;
        pub use percent_encoding;
        pub use webkit2gtk;
    }

    // -------------------------------------------------------------------------
    // Windows
    // -------------------------------------------------------------------------

    #[cfg(windows)]
    pub mod windows {
        pub use once_cell;
        pub use softbuffer;
        pub use webview2_com;
        pub use window_vibrancy;
        pub use windows;
        pub use windows_sys;
    }

    // -------------------------------------------------------------------------
    // Apple
    // -------------------------------------------------------------------------

    #[cfg(target_vendor = "apple")]
    pub mod apple {
        pub use objc2;
    }

    // -------------------------------------------------------------------------
    // macOS
    // -------------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    pub mod macos {
        pub use embed_plist;
        pub use objc2_app_kit;
        pub use objc2_foundation;
        pub use objc2_web_kit;
        pub use plist;
        pub use window_vibrancy;
    }

    // -------------------------------------------------------------------------
    // iOS / Apple mobile
    // -------------------------------------------------------------------------

    #[cfg(all(target_vendor = "apple", not(target_os = "macos")))]
    pub mod ios {
        pub use libc;
        pub use objc2_ui_kit;
        pub use swift_rs;
    }

    // -------------------------------------------------------------------------
    // Android
    // -------------------------------------------------------------------------

    #[cfg(target_os = "android")]
    pub mod android {
        pub use jni;
    }

    // -------------------------------------------------------------------------
    // Mobile
    // -------------------------------------------------------------------------

    #[cfg(any(
        target_os = "android",
        all(target_vendor = "apple", not(target_os = "macos")),
    ))]
    pub mod mobile {
        pub use bytes;
        pub use reqwest;

        #[cfg(feature = "rustls")]
        pub use rustls;
    }
}
