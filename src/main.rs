mod error;
mod menu;
mod platform;
mod tray;
mod types;
mod utils;
mod webview;
mod window;

pub mod prelude {
    pub use crate::{
        error::*,
        menu::prelude::*,
        platform::prelude::*,
        tray::*,
        types::*,
        utils::*,
        webview::{ManagedWebview, WebViewBuilder},
        window::{ManagedWindow, WindowBuilder},
    };
}

pub mod native {
    pub use global_hotkey;
    pub use muda;
    pub use tao;
    pub use tray_icon;
    #[cfg(windows)]
    pub use webview2_com;
    pub use wry;
}

fn main() {}
