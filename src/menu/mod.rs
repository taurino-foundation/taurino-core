mod builder;
mod context;
mod item;
mod menu;
mod metadata;

pub(crate) use context::sealed;

pub mod prelude {
    pub use super::builder::{CheckMenuItemBuilder, IconMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
    pub use super::context::ContextMenu;
    pub use super::item::{
        CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu,
    };
    pub use super::metadata::{AboutMetadata, AboutMetadataBuilder, NativeIcon};
}
