mod builder;
mod context;
mod item;
mod menu;
mod metadata;
use crate::prelude::Theme;
use std::sync::Arc;

pub(crate) use context::sealed;


pub fn map_to_menu_theme(theme:Theme) -> muda::MenuTheme {
    match theme {
        Theme::Light => muda::MenuTheme::Light,
        Theme::Dark => muda::MenuTheme::Dark,
        _ => muda::MenuTheme::Auto,
    }
}
// -----------------------------------------------------------------------------
// muda event handler → forward to tao
//
// `set_event_handler` must be called once, before any menu is displayed.
// -----------------------------------------------------------------------------

pub fn install_menu_event_handler<F>(send: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    let send = Arc::new(send);

    muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        send(event.id.0.clone());
    }));
}

pub mod prelude {
    pub use super::builder::{
        CheckMenuItemBuilder, IconMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder,
    };
    pub use super::context::ContextMenu;
    pub use super::{install_menu_event_handler,map_to_menu_theme};
    pub use super::item::{
        CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem,
        Submenu,
    };
    pub use super::metadata::{AboutMetadata, AboutMetadataBuilder, NativeIcon};
}
