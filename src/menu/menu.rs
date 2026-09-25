use super::{
    item::{IsMenuItem, Menu, MenuInner, MenuItemKind, Submenu, SubmenuInner},
    metadata::NativeIcon,
};
use crate::error::Result;
use crate::utils::image::Image;
use muda::MenuId;
use std::sync::Arc;

// -----------------------------------------------------------------------------
// submenu
// -----------------------------------------------------------------------------


#[cfg(target_os = "macos")]
impl Submenu {
    pub fn set_as_windows_menu_for_nsapp(&self) {
        self.0.inner.set_as_windows_menu_for_nsapp();
    }

    pub fn set_as_help_menu_for_nsapp(&self) {
        self.0.inner.set_as_help_menu_for_nsapp();
    }
}


impl Menu {
    pub(crate) fn inner_muda(&self) -> &muda::Menu {
        &self.0.inner
    }
}

impl Submenu {
    pub(crate) fn inner_muda(&self) -> &muda::Submenu {
        &self.0.inner
    }
}




impl Submenu {
    pub fn new<S: AsRef<str>>(text: S, enabled: bool) -> Result<Self> {
        Ok(Self(Arc::new(SubmenuInner::new(muda::Submenu::new(
            text.as_ref(),
            enabled,
        )))))
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S, enabled: bool) -> Result<Self> {
        Ok(Self(Arc::new(SubmenuInner::new(muda::Submenu::with_id(
            id,
            text.as_ref(),
            enabled,
        )))))
    }

    pub fn new_with_icon<S: AsRef<str>>(text: S, enabled: bool, icon: Option<Image<'_>>) -> Result<Self> {
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

    pub fn new_with_native_icon<S: AsRef<str>>(text: S, enabled: bool, icon: Option<NativeIcon>) -> Result<Self> {
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

    pub fn with_items<S: AsRef<str>>(text: S, enabled: bool, items: &[&dyn IsMenuItem]) -> Result<Self> {
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

    pub fn insert(&self, item: &dyn IsMenuItem, position: usize) -> Result<()> {
        self.0.inner.insert(item.inner_muda(), position).map_err(Into::into)
    }

    pub fn remove(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.0.inner.remove(item.inner_muda()).map_err(Into::into)
    }

    pub fn items(&self) -> Result<Vec<MenuItemKind>> {
        Ok(self.0.inner.items().into_iter().map(MenuItemKind::from_muda).collect())
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

    pub fn set_native_icon(&self, icon: Option<NativeIcon>) -> Result<()> {
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

    pub fn with_id_and_items<I: Into<MenuId>>(id: I, items: &[&dyn IsMenuItem]) -> Result<Self> {
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

    pub fn insert(&self, item: &dyn IsMenuItem, position: usize) -> Result<()> {
        self.0.inner.insert(item.inner_muda(), position).map_err(Into::into)
    }

    pub fn remove(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.0.inner.remove(item.inner_muda()).map_err(Into::into)
    }

    pub fn items(&self) -> Result<Vec<MenuItemKind>> {
        Ok(self.0.inner.items().into_iter().map(MenuItemKind::from_muda).collect())
    }


}





impl Menu {
    // -------------------------------------------------------------------------
    // Windows
    // -------------------------------------------------------------------------

    #[cfg(windows)]
    pub unsafe fn init_for_hwnd(&self, hwnd: isize) -> Result<()> {
        unsafe {
            self.0
                .inner
                .init_for_hwnd(hwnd)
                .map_err(Into::into)
        }
    }

    #[cfg(windows)]
    pub unsafe fn init_for_hwnd_with_theme(
        &self,
        hwnd: isize,
        theme: muda::MenuTheme,
    ) -> Result<()> {
        unsafe {
            self.0
                .inner
                .init_for_hwnd_with_theme(hwnd, theme)
                .map_err(Into::into)
        }
    }

    #[cfg(windows)]
    pub unsafe fn set_theme_for_hwnd(
        &self,
        hwnd: isize,
        theme: muda::MenuTheme,
    ) -> Result<()> {
        unsafe {
            self.0
                .inner
                .set_theme_for_hwnd(hwnd, theme)
                .map_err(Into::into)
        }
    }

    #[cfg(windows)]
    pub fn haccel(&self) -> isize {
        self.0.inner.haccel()
    }

    #[cfg(windows)]
    pub fn hmenu(&self) -> isize {
        self.0.inner.hmenu()
    }

    // -------------------------------------------------------------------------
    // macOS
    // -------------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    pub fn init_for_nsapp(&self) {
        self.0.inner.init_for_nsapp();
    }

    // -------------------------------------------------------------------------
    // Linux / BSD — GTK
    // -------------------------------------------------------------------------

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn init_for_gtk_window<W, C>(
        &self,
        window: &W,
        container: Option<&C>,
    ) -> Result<()>
    where
        W: gtk::prelude::IsA<gtk::Window>
            + gtk::prelude::IsA<gtk::Widget>,
        C: gtk::prelude::IsA<gtk::Widget>,
    {
        self.0
            .inner
            .init_for_gtk_window(window, container)
            .map_err(Into::into)
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn remove_for_gtk_window<W>(&self, window: &W) -> Result<()>
    where
        W: gtk::prelude::IsA<gtk::Window>
            + gtk::prelude::IsA<gtk::Widget>,
    {
        self.0
            .inner
            .remove_for_gtk_window(window)
            .map_err(Into::into)
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn hide_for_gtk_window<W>(&self, window: &W) -> Result<()>
    where
        W: gtk::prelude::IsA<gtk::Window>,
    {
        self.0
            .inner
            .hide_for_gtk_window(window)
            .map_err(Into::into)
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn show_for_gtk_window<W>(&self, window: &W) -> Result<()>
    where
        W: gtk::prelude::IsA<gtk::Window>,
    {
        self.0
            .inner
            .show_for_gtk_window(window)
            .map_err(Into::into)
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn is_visible_on_gtk_window<W>(&self, window: &W) -> bool
    where
        W: gtk::prelude::IsA<gtk::Window>,
    {
        self.0.inner.is_visible_on_gtk_window(window)
    }

    #[cfg(all(
        feature = "gtk3",
        any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd"
        )
    ))]
    pub fn gtk_menubar_for_gtk_window<W>(
        self,
        window: &W,
    ) -> Option<gtk::MenuBar>
    where
        W: gtk::prelude::IsA<gtk::Window>,
    {
        self.0.inner.clone().gtk_menubar_for_gtk_window(window)
    }
}
