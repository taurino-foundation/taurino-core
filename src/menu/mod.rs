mod builder;
mod context;
mod item;
mod menu;
mod metadata;

use crate::native::tao::window::Theme as TaoTheme;
use crate::prelude::Theme;
use std::sync::Arc;

pub(crate) use context::sealed;

// -----------------------------------------------------------------------------
// Menu theme mapping
//
// `muda::MenuTheme` is a Windows-specific API. Do not expose/use it when
// compiling the crate for macOS or Linux/BSD.
// -----------------------------------------------------------------------------

#[cfg(windows)]
pub fn map_to_menu_theme(theme: Theme) -> muda::MenuTheme {
    match theme {
        Theme::Light => muda::MenuTheme::Light,
        Theme::Dark => muda::MenuTheme::Dark,
        #[allow(unreachable_patterns)]
        _ => muda::MenuTheme::Auto,
    }
}

#[cfg(windows)]
pub fn map_from_tao_to_menu_theme(theme: TaoTheme) -> muda::MenuTheme {
    match theme {
        TaoTheme::Light => muda::MenuTheme::Light,
        TaoTheme::Dark => muda::MenuTheme::Dark,
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
    // Builders
    pub use super::builder::{CheckMenuItemBuilder, IconMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};

    // Traits
    pub use super::context::ContextMenu;

    pub use super::item::{
        CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu,
    };

    // Metadata / enums
    pub use super::metadata::{AboutMetadata, AboutMetadataBuilder, NativeIcon};

    // Events
    pub use super::install_menu_event_handler;

    // Windows-only public API
    #[cfg(windows)]
    pub use super::{map_from_tao_to_menu_theme, map_to_menu_theme};
}
