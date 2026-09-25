mod dialog;
mod monitor;
mod undecorated_resizing;
mod util;
mod webview;
mod window;

pub mod prelude {
    pub use super::dialog::error;
    pub use super::monitor::MonitorExt;
    pub use super::undecorated_resizing::{
        attach_resize_handler, detach_resize_handler, update_drag_hwnd_rgn_for_undecorated,
    };
    #[cfg(windows)]
    pub use super::util::{encode_wide, get_system_metrics_for_dpi, hwnd_dpi};
    pub use super::webview::Webview;
    pub use super::window::{WindowExt, calculate_window_center_position};
}
