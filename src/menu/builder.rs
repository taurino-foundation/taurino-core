use super::{
    item::{
        CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem,
        Submenu,
    },
    metadata::{AboutMetadata, NativeIcon},
};
use crate::error::Result;
use crate::utils::image::Image;
use muda::MenuId;

// -----------------------------------------------------------------------------
// item builders
// -----------------------------------------------------------------------------

pub struct MenuItemBuilder {
    id: Option<MenuId>,
    text: String,
    enabled: bool,
    accelerator: Option<String>,
}

impl MenuItemBuilder {
    pub fn new<S: AsRef<str>>(text: S) -> Self {
        Self {
            id: None,
            text: text.as_ref().into(),
            enabled: true,
            accelerator: None,
        }
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S) -> Self {
        Self {
            id: Some(id.into()),
            text: text.as_ref().into(),
            enabled: true,
            accelerator: None,
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn accelerator<S: AsRef<str>>(mut self, accelerator: S) -> Self {
        self.accelerator = Some(accelerator.as_ref().into());
        self
    }

    pub fn build(self) -> Result<MenuItem> {
        match self.id {
            Some(id) => MenuItem::with_id(id, self.text, self.enabled, self.accelerator),
            None => MenuItem::new(self.text, self.enabled, self.accelerator),
        }
    }
}

pub struct CheckMenuItemBuilder {
    id: Option<MenuId>,
    text: String,
    enabled: bool,
    checked: bool,
    accelerator: Option<String>,
}

impl CheckMenuItemBuilder {
    pub fn new<S: AsRef<str>>(text: S) -> Self {
        Self {
            id: None,
            text: text.as_ref().into(),
            enabled: true,
            checked: true,
            accelerator: None,
        }
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S) -> Self {
        Self {
            id: Some(id.into()),
            text: text.as_ref().into(),
            enabled: true,
            checked: true,
            accelerator: None,
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    pub fn accelerator<S: AsRef<str>>(mut self, accelerator: S) -> Self {
        self.accelerator = Some(accelerator.as_ref().into());
        self
    }

    pub fn build(self) -> Result<CheckMenuItem> {
        match self.id {
            Some(id) => {
                CheckMenuItem::with_id(id, self.text, self.enabled, self.checked, self.accelerator)
            }
            None => CheckMenuItem::new(self.text, self.enabled, self.checked, self.accelerator),
        }
    }
}

pub struct IconMenuItemBuilder<'a> {
    id: Option<MenuId>,
    text: String,
    enabled: bool,
    icon: Option<Image<'a>>,
    native_icon: Option<NativeIcon>,
    accelerator: Option<String>,
}

impl<'a> IconMenuItemBuilder<'a> {
    pub fn new<S: AsRef<str>>(text: S) -> Self {
        Self {
            id: None,
            text: text.as_ref().into(),
            enabled: true,
            icon: None,
            native_icon: None,
            accelerator: None,
        }
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S) -> Self {
        Self {
            id: Some(id.into()),
            text: text.as_ref().into(),
            enabled: true,
            icon: None,
            native_icon: None,
            accelerator: None,
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn accelerator<S: AsRef<str>>(mut self, accelerator: S) -> Self {
        self.accelerator = Some(accelerator.as_ref().into());
        self
    }

    pub fn icon(mut self, icon: Image<'a>) -> Self {
        self.icon = Some(icon);
        self.native_icon = None;
        self
    }

    pub fn native_icon(mut self, icon: NativeIcon) -> Self {
        self.native_icon = Some(icon);
        self.icon = None;
        self
    }

    pub fn build(self) -> Result<IconMenuItem> {
        match (self.id, self.icon, self.native_icon) {
            (Some(id), Some(icon), _) => {
                IconMenuItem::with_id(id, self.text, self.enabled, Some(icon), self.accelerator)
            }
            (None, Some(icon), _) => {
                IconMenuItem::new(self.text, self.enabled, Some(icon), self.accelerator)
            }
            (Some(id), None, native) => IconMenuItem::with_id_and_native_icon(
                id,
                self.text,
                self.enabled,
                native,
                self.accelerator,
            ),
            (None, None, native) => {
                IconMenuItem::with_native_icon(self.text, self.enabled, native, self.accelerator)
            }
        }
    }
}

// -----------------------------------------------------------------------------
// fluent MenuBuilder / SubmenuBuilder
// -----------------------------------------------------------------------------

pub struct MenuBuilder {
    id: Option<MenuId>,
    items: Vec<Result<MenuItemKind>>,
}

impl MenuBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            items: Vec::new(),
        }
    }

    pub fn with_id<I: Into<MenuId>>(id: I) -> Self {
        Self {
            id: Some(id.into()),
            items: Vec::new(),
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn item(mut self, item: &dyn IsMenuItem) -> Self {
        self.items.push(Ok(item.kind()));
        self
    }

    pub fn items(mut self, items: &[&dyn IsMenuItem]) -> Self {
        for item in items {
            self = self.item(*item);
        }
        self
    }

    pub fn text<I: Into<MenuId>, S: AsRef<str>>(mut self, id: I, text: S) -> Self {
        self.items
            .push(MenuItem::with_id(id, text, true, None::<&str>).map(MenuItemKind::MenuItem));
        self
    }

    pub fn check<I: Into<MenuId>, S: AsRef<str>>(mut self, id: I, text: S) -> Self {
        self.items.push(
            CheckMenuItem::with_id(id, text, true, true, None::<&str>).map(MenuItemKind::Check),
        );
        self
    }

    pub fn icon<I: Into<MenuId>, S: AsRef<str>>(mut self, id: I, text: S, icon: Image<'_>) -> Self {
        self.items.push(
            IconMenuItem::with_id(id, text, true, Some(icon), None::<&str>).map(MenuItemKind::Icon),
        );
        self
    }

    pub fn native_icon<I: Into<MenuId>, S: AsRef<str>>(
        mut self,
        id: I,
        text: S,
        icon: NativeIcon,
    ) -> Self {
        self.items.push(
            IconMenuItem::with_id_and_native_icon(id, text, true, Some(icon), None::<&str>)
                .map(MenuItemKind::Icon),
        );
        self
    }

    pub fn separator(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::separator().map(MenuItemKind::Predefined));
        self
    }
    pub fn copy(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::copy(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn cut(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::cut(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn paste(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::paste(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn select_all(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::select_all(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn undo(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::undo(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn redo(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::redo(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn minimize(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::minimize(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn maximize(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::maximize(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn fullscreen(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::fullscreen(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn hide(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::hide(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn hide_others(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::hide_others(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn show_all(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::show_all(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn close_window(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::close_window(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn quit(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::quit(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn services(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::services(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn bring_all_to_front(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::bring_all_to_front(None).map(MenuItemKind::Predefined));
        self
    }

    pub fn about(mut self, metadata: Option<AboutMetadata<'_>>) -> Self {
        self.items
            .push(PredefinedMenuItem::about(None, metadata).map(MenuItemKind::Predefined));
        self
    }

    pub fn build(self) -> Result<Menu> {
        let menu = match self.id {
            Some(id) => Menu::with_id(id)?,
            None => Menu::new()?,
        };
        for item in self.items {
            menu.append(&item?)?;
        }
        Ok(menu)
    }
}

impl Default for MenuBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SubmenuBuilder<'a> {
    id: Option<MenuId>,
    text: String,
    enabled: bool,
    items: Vec<Result<MenuItemKind>>,
    icon: Option<Image<'a>>,
    native_icon: Option<NativeIcon>,
}

impl<'a> SubmenuBuilder<'a> {
    pub fn new<S: AsRef<str>>(text: S) -> Self {
        Self {
            id: None,
            text: text.as_ref().into(),
            enabled: true,
            items: Vec::new(),
            icon: None,
            native_icon: None,
        }
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S) -> Self {
        Self {
            id: Some(id.into()),
            text: text.as_ref().into(),
            enabled: true,
            items: Vec::new(),
            icon: None,
            native_icon: None,
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn submenu_icon(mut self, icon: Image<'a>) -> Self {
        self.icon = Some(icon);
        self.native_icon = None;
        self
    }

    pub fn submenu_native_icon(mut self, icon: NativeIcon) -> Self {
        self.native_icon = Some(icon);
        self.icon = None;
        self
    }

    pub fn item(mut self, item: &dyn IsMenuItem) -> Self {
        self.items.push(Ok(item.kind()));
        self
    }

    pub fn items(mut self, items: &[&dyn IsMenuItem]) -> Self {
        for item in items {
            self = self.item(*item);
        }
        self
    }

    pub fn text_item<I: Into<MenuId>, S: AsRef<str>>(mut self, id: I, text: S) -> Self {
        self.items
            .push(MenuItem::with_id(id, text, true, None::<&str>).map(MenuItemKind::MenuItem));
        self
    }

    pub fn check<I: Into<MenuId>, S: AsRef<str>>(mut self, id: I, text: S) -> Self {
        self.items.push(
            CheckMenuItem::with_id(id, text, true, true, None::<&str>).map(MenuItemKind::Check),
        );
        self
    }

    pub fn separator(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::separator().map(MenuItemKind::Predefined));
        self
    }

    pub fn build(self) -> Result<Submenu> {
        let submenu = match (self.id, self.icon, self.native_icon) {
            (Some(id), Some(icon), _) => {
                Submenu::with_id_and_icon(id, self.text, self.enabled, Some(icon))?
            }
            (None, Some(icon), _) => Submenu::new_with_icon(self.text, self.enabled, Some(icon))?,
            (Some(id), None, Some(icon)) => {
                Submenu::with_id_and_native_icon(id, self.text, self.enabled, Some(icon))?
            }
            (None, None, Some(icon)) => {
                Submenu::new_with_native_icon(self.text, self.enabled, Some(icon))?
            }
            (Some(id), None, None) => Submenu::with_id(id, self.text, self.enabled)?,
            (None, None, None) => Submenu::new(self.text, self.enabled)?,
        };
        for item in self.items {
            submenu.append(&item?)?;
        }
        Ok(submenu)
    }
}
