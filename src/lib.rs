mod error;
mod menu;
mod platform;
mod types;
mod utils;
mod webview;
mod window;


pub mod prelude {
    pub use crate::{
        error::*,
        menu::prelude::*,
        platform::prelude::*,
        types::*,
        webview::{ManagedWebview, WebViewBuilder},
        window::{ManagedWindow, WindowBuilder},
    };
}

pub mod  native{
    pub use tao;
    pub use wry;
    pub use tray_icon;
    pub use muda;
    pub use global_hotkey;
    #[cfg(windows)]
    pub use webview2_com;
}