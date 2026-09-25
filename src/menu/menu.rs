use muda::MenuId;
use std::sync::Arc;
use crate::error::{Result};
use super::{
    item::{IsMenuItem, Menu, MenuInner, MenuItemKind, Submenu, SubmenuInner},
    metadata::NativeIcon,
};
use crate::utils::image::Image;

// -----------------------------------------------------------------------------
// submenu
// -----------------------------------------------------------------------------

impl Submenu {
    pub fn new<S: AsRef<str>>(text: S, enabled: bool) -> Result<Self> {
        Ok(Self(Arc::new(SubmenuInner::new(muda::Submenu::new(
            text.as_ref(),
            enabled,
        )))))
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(
        id: I,
        text: S,
        enabled: bool,
    ) -> Result<Self> {
        Ok(Self(Arc::new(SubmenuInner::new(muda::Submenu::with_id(
            id,
            text.as_ref(),
            enabled,
        )))))
    }

    pub fn new_with_icon<S: AsRef<str>>(
        text: S,
        enabled: bool,
        icon: Option<Image<'_>>,
    ) -> Result<Self> {
        let submenu = muda::Submenu::new(text.as_ref(), enabled);
        if let Some(icon) = icon {
            submenu.set_icon(Some(icon.try_into()?));
        }
        Ok(Self(Arc::new(SubmenuInner::new(submenu))))
    }

    pub fn with_id_and_icon<I: Into<MenuId>, S: AsRef<str>>(
        id: I,
        text: S,
        enabled: bool,
        icon: Option<Image<'_>>,
    ) -> Result<Self> {
        let submenu = muda::Submenu::with_id(id, text.as_ref(), enabled);
        if let Some(icon) = icon {
            submenu.set_icon(Some(icon.try_into()?));
        }
        Ok(Self(Arc::new(SubmenuInner::new(submenu))))
    }

    pub fn new_with_native_icon<S: AsRef<str>>(
        text: S,
        enabled: bool,
        icon: Option<NativeIcon>,
    ) -> Result<Self> {
        let submenu = muda::Submenu::new(text.as_ref(), enabled);
        if let Some(icon) = icon {
            submenu.set_native_icon(Some(icon.into()));
        }
        Ok(Self(Arc::new(SubmenuInner::new(submenu))))
    }

    pub fn with_id_and_native_icon<I: Into<MenuId>, S: AsRef<str>>(
        id: I,
        text: S,
        enabled: bool,
        icon: Option<NativeIcon>,
    ) -> Result<Self> {
        let submenu = muda::Submenu::with_id(id, text.as_ref(), enabled);
        if let Some(icon) = icon {
            submenu.set_native_icon(Some(icon.into()));
        }
        Ok(Self(Arc::new(SubmenuInner::new(submenu))))
    }

    pub fn with_items<S: AsRef<str>>(
        text: S,
        enabled: bool,
        items: &[&dyn IsMenuItem],
    ) -> Result<Self> {
        let submenu = Self::new(text, enabled)?;
        submenu.append_items(items)?;
        Ok(submenu)
    }

    pub fn with_id_and_items<I: Into<MenuId>, S: AsRef<str>>(
        id: I,
        text: S,
        enabled: bool,
        items: &[&dyn IsMenuItem],
    ) -> Result<Self> {
        let submenu = Self::with_id(id, text, enabled)?;
        submenu.append_items(items)?;
        Ok(submenu)
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }
    pub fn text(&self) -> Result<String> {
        Ok(self.0.inner.text())
    }

    pub fn append(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.0.inner.append(item.inner_muda()).map_err(Into::into)
    }

    pub fn append_items(&self, items: &[&dyn IsMenuItem]) -> Result<()> {
        for item in items {
            self.append(*item)?;
        }
        Ok(())
    }

    pub fn prepend(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.0.inner.prepend(item.inner_muda()).map_err(Into::into)
    }

    pub fn insert(
        &self,
        item: &dyn IsMenuItem,
        position: usize,
    ) -> Result<()> {
        self.0
            .inner
            .insert(item.inner_muda(), position)
            .map_err(Into::into)
    }

    pub fn remove(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.0.inner.remove(item.inner_muda()).map_err(Into::into)
    }

    pub fn items(&self) -> Result<Vec<MenuItemKind>> {
        Ok(self
            .0
            .inner
            .items()
            .into_iter()
            .map(MenuItemKind::from_muda)
            .collect())
    }

    pub fn set_text<S: AsRef<str>>(&self, text: S) -> Result<()> {
        self.0.inner.set_text(text.as_ref());
        Ok(())
    }

    pub fn is_enabled(&self) -> Result<bool> {
        Ok(self.0.inner.is_enabled())
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        self.0.inner.set_enabled(enabled);
        Ok(())
    }

    pub fn set_icon(&self, icon: Option<Image<'_>>) -> Result<()> {
        let icon = icon.map(TryInto::try_into).transpose()?;
        self.0.inner.set_icon(icon);
        Ok(())
    }

    pub fn set_native_icon(
        &self,
        icon: Option<NativeIcon>,
    ) -> Result<()> {
        #[cfg(target_os = "macos")]
        self.0.inner.set_native_icon(icon.map(Into::into));
        let _ = icon;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// menu
// -----------------------------------------------------------------------------

impl Menu {
    pub fn new() -> Result<Self> {
        Ok(Self(Arc::new(MenuInner::new(muda::Menu::new()))))
    }

    pub fn with_id<I: Into<MenuId>>(id: I) -> Result<Self> {
        Ok(Self(Arc::new(MenuInner::new(muda::Menu::with_id(id)))))
    }

    pub fn with_items(items: &[&dyn IsMenuItem]) -> Result<Self> {
        let menu = Self::new()?;
        menu.append_items(items)?;
        Ok(menu)
    }

    pub fn with_id_and_items<I: Into<MenuId>>(
        id: I,
        items: &[&dyn IsMenuItem],
    ) -> Result<Self> {
        let menu = Self::with_id(id)?;
        menu.append_items(items)?;
        Ok(menu)
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }

    pub fn append(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.0.inner.append(item.inner_muda()).map_err(Into::into)
    }

    pub fn append_items(&self, items: &[&dyn IsMenuItem]) -> Result<()> {
        for item in items {
            self.append(*item)?;
        }
        Ok(())
    }

    pub fn prepend(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.0.inner.prepend(item.inner_muda()).map_err(Into::into)
    }

    pub fn insert(
        &self,
        item: &dyn IsMenuItem,
        position: usize,
    ) -> Result<()> {
        self.0
            .inner
            .insert(item.inner_muda(), position)
            .map_err(Into::into)
    }

    pub fn remove(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.0.inner.remove(item.inner_muda()).map_err(Into::into)
    }

    pub fn items(&self) -> Result<Vec<MenuItemKind>> {
        Ok(self
            .0
            .inner
            .items()
            .into_iter()
            .map(MenuItemKind::from_muda)
            .collect())
    }
}
