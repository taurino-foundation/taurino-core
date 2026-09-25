use super::{
    context::sealed,
    metadata::{AboutMetadata, NativeIcon},
};
use crate::error::Result;
use crate::{
    menu::sealed::IsMenuItemBase,
    utils::{image::Image, resource::Resource},
};
use muda::MenuId;
use std::sync::Arc;

// -----------------------------------------------------------------------------
// wrappers
// -----------------------------------------------------------------------------

macro_rules! gen_wrapper {
    ($ty:ident, $inner:ident) => {
        pub(crate) struct $inner {
            pub(crate) inner: muda::$ty,
        }

        impl $inner {
            #[inline]
            pub(crate) fn new(inner: muda::$ty) -> Self {
                Self { inner }
            }
        }

        // Kept only because crate::Resource requires Send + Sync.
        // This relies on the caller's single-thread invariant.
        unsafe impl Send for $inner {}
        unsafe impl Sync for $inner {}

        pub struct $ty(pub(crate) Arc<$inner>);

        impl Clone for $ty {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }

        impl Resource for $ty {}
    };
}

gen_wrapper!(Menu, MenuInner);
gen_wrapper!(MenuItem, MenuItemInner);
gen_wrapper!(Submenu, SubmenuInner);
gen_wrapper!(PredefinedMenuItem, PredefinedMenuItemInner);
gen_wrapper!(CheckMenuItem, CheckMenuItemInner);
gen_wrapper!(IconMenuItem, IconMenuItemInner);

// -----------------------------------------------------------------------------
// generic menu-item facade
// -----------------------------------------------------------------------------

pub enum MenuItemKind {
    MenuItem(MenuItem),
    Submenu(Submenu),
    Predefined(PredefinedMenuItem),
    Check(CheckMenuItem),
    Icon(IconMenuItem),
}

impl Clone for MenuItemKind {
    fn clone(&self) -> Self {
        match self {
            Self::MenuItem(v) => Self::MenuItem(v.clone()),
            Self::Submenu(v) => Self::Submenu(v.clone()),
            Self::Predefined(v) => Self::Predefined(v.clone()),
            Self::Check(v) => Self::Check(v.clone()),
            Self::Icon(v) => Self::Icon(v.clone()),
        }
    }
}

impl MenuItemKind {
    // -------------------------------------------------------------------------
    // type
    // -------------------------------------------------------------------------

    pub fn as_menu_item(&self) -> Option<&MenuItem> {
        match self {
            Self::MenuItem(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_submenu(&self) -> Option<&Submenu> {
        match self {
            Self::Submenu(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_predefined(&self) -> Option<&PredefinedMenuItem> {
        match self {
            Self::Predefined(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_check(&self) -> Option<&CheckMenuItem> {
        match self {
            Self::Check(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_icon(&self) -> Option<&IconMenuItem> {
        match self {
            Self::Icon(item) => Some(item),
            _ => None,
        }
    }

    pub fn is_menu_item(&self) -> bool {
        matches!(self, Self::MenuItem(_))
    }

    pub fn is_submenu(&self) -> bool {
        matches!(self, Self::Submenu(_))
    }

    pub fn is_predefined(&self) -> bool {
        matches!(self, Self::Predefined(_))
    }

    pub fn is_check(&self) -> bool {
        matches!(self, Self::Check(_))
    }

    pub fn is_icon(&self) -> bool {
        matches!(self, Self::Icon(_))
    }

    // -------------------------------------------------------------------------
    // generic state
    // -------------------------------------------------------------------------

    pub fn text(&self) -> Result<String> {
        match self {
            Self::MenuItem(item) => item.text(),
            Self::Submenu(item) => item.text(),
            Self::Predefined(item) => item.text(),
            Self::Check(item) => item.text(),
            Self::Icon(item) => item.text(),
        }
    }

    pub fn set_text<S: AsRef<str>>(&self, text: S) -> Result<()> {
        match self {
            Self::MenuItem(item) => item.set_text(text),
            Self::Submenu(item) => item.set_text(text),
            Self::Predefined(item) => item.set_text(text),
            Self::Check(item) => item.set_text(text),
            Self::Icon(item) => item.set_text(text),
        }
    }

    pub fn is_enabled(&self) -> Option<Result<bool>> {
        match self {
            Self::MenuItem(item) => Some(item.is_enabled()),
            Self::Submenu(item) => Some(item.is_enabled()),
            Self::Check(item) => Some(item.is_enabled()),
            Self::Icon(item) => Some(item.is_enabled()),
            Self::Predefined(_) => None,
        }
    }

    pub fn set_enabled(&self, enabled: bool) -> Option<Result<()>> {
        match self {
            Self::MenuItem(item) => Some(item.set_enabled(enabled)),
            Self::Submenu(item) => Some(item.set_enabled(enabled)),
            Self::Check(item) => Some(item.set_enabled(enabled)),
            Self::Icon(item) => Some(item.set_enabled(enabled)),
            Self::Predefined(_) => None,
        }
    }

    pub fn is_checked(&self) -> Option<Result<bool>> {
        match self {
            Self::Check(item) => Some(item.is_checked()),
            _ => None,
        }
    }

    pub fn set_checked(&self, checked: bool) -> Option<Result<()>> {
        match self {
            Self::Check(item) => Some(item.set_checked(checked)),
            _ => None,
        }
    }

    // -------------------------------------------------------------------------
    // tree
    // -------------------------------------------------------------------------

    pub fn children(&self) -> Result<Vec<MenuItemKind>> {
        match self {
            Self::Submenu(submenu) => submenu.items(),
            _ => Ok(Vec::new()),
        }
    }

    pub fn has_children(&self) -> bool {
        matches!(self, Self::Submenu(_))
    }

    pub fn find(&self, id: &MenuId) -> Result<Option<MenuItemKind>> {
        if self.id() == id {
            return Ok(Some(self.clone()));
        }

        if let Self::Submenu(submenu) = self {
            for child in submenu.items()? {
                if let Some(found) = child.find(id)? {
                    return Ok(Some(found));
                }
            }
        }

        Ok(None)
    }

    pub fn descendants(&self) -> Result<Vec<MenuItemKind>> {
        let mut result = Vec::new();
        self.collect_descendants(&mut result)?;
        Ok(result)
    }

    pub(crate) fn collect_descendants(&self, result: &mut Vec<MenuItemKind>) -> Result<()> {
        if let Self::Submenu(submenu) = self {
            for child in submenu.items()? {
                result.push(child.clone());
                child.collect_descendants(result)?;
            }
        }

        Ok(())
    }

    pub fn id(&self) -> &MenuId {
        match self {
            Self::MenuItem(v) => v.id(),
            Self::Submenu(v) => v.id(),
            Self::Predefined(v) => v.id(),
            Self::Check(v) => v.id(),
            Self::Icon(v) => v.id(),
        }
    }

    pub(crate) fn inner_muda(&self) -> &dyn muda::IsMenuItem {
        match self {
            Self::MenuItem(v) => v.inner_muda(),
            Self::Submenu(v) => v.inner_muda(),
            Self::Predefined(v) => v.inner_muda(),
            Self::Check(v) => v.inner_muda(),
            Self::Icon(v) => v.inner_muda(),
        }
    }

    pub fn visit<F>(&self, visitor: &mut F) -> Result<()>
    where
        F: FnMut(&MenuItemKind) -> Result<()>,
    {
        visitor(self)?;

        if let Self::Submenu(submenu) = self {
            for child in submenu.items()? {
                child.visit(visitor)?;
            }
        }

        Ok(())
    }
}

/// A trait that defines a generic item in a menu, which may be one of [`MenuItemKind`]
///
/// # Safety
///
/// This trait is ONLY meant to be implemented internally by the crate.
pub trait IsMenuItem: sealed::IsMenuItemBase {
    /// Returns the kind of this menu item.
    fn kind(&self) -> MenuItemKind;

    /// Returns a unique identifier associated with this menu.
    fn id(&self) -> &MenuId;
}

macro_rules! impl_menu_item {
    ($ty:ident, $variant:ident) => {
        impl sealed::IsMenuItemBase for $ty {
            fn inner_muda(&self) -> &dyn muda::IsMenuItem {
                &self.0.inner
            }
        }

        impl IsMenuItem for $ty {
            fn kind(&self) -> MenuItemKind {
                MenuItemKind::$variant(self.clone())
            }

            fn id(&self) -> &MenuId {
                self.id()
            }
        }
    };
}

impl_menu_item!(MenuItem, MenuItem);
impl_menu_item!(Submenu, Submenu);
impl_menu_item!(PredefinedMenuItem, Predefined);
impl_menu_item!(CheckMenuItem, Check);
impl_menu_item!(IconMenuItem, Icon);

// -----------------------------------------------------------------------------
// normal item
// -----------------------------------------------------------------------------

impl MenuItem {
    pub fn new<T: AsRef<str>, A: AsRef<str>>(
        text: T,
        enabled: bool,
        accelerator: Option<A>,
    ) -> Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::MenuItem::new(text.as_ref(), enabled, accelerator);
        Ok(Self(Arc::new(MenuItemInner::new(inner))))
    }

    pub fn with_id<I: Into<MenuId>, T: AsRef<str>, A: AsRef<str>>(
        id: I,
        text: T,
        enabled: bool,
        accelerator: Option<A>,
    ) -> Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::MenuItem::with_id(id, text.as_ref(), enabled, accelerator);
        Ok(Self(Arc::new(MenuItemInner::new(inner))))
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }

    pub fn text(&self) -> Result<String> {
        Ok(self.0.inner.text())
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

    pub fn set_accelerator<S: AsRef<str>>(&self, accelerator: Option<S>) -> Result<()> {
        let accel = accelerator.and_then(|s| s.as_ref().parse().ok());
        self.0.inner.set_accelerator(accel).map_err(Into::into)
    }
}

// -----------------------------------------------------------------------------
// check item
// -----------------------------------------------------------------------------

impl CheckMenuItem {
    pub fn new<T: AsRef<str>, A: AsRef<str>>(
        text: T,
        enabled: bool,
        checked: bool,
        accelerator: Option<A>,
    ) -> Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::CheckMenuItem::new(text.as_ref(), enabled, checked, accelerator);
        Ok(Self(Arc::new(CheckMenuItemInner::new(inner))))
    }

    pub fn with_id<I: Into<MenuId>, T: AsRef<str>, A: AsRef<str>>(
        id: I,
        text: T,
        enabled: bool,
        checked: bool,
        accelerator: Option<A>,
    ) -> Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::CheckMenuItem::with_id(id, text.as_ref(), enabled, checked, accelerator);
        Ok(Self(Arc::new(CheckMenuItemInner::new(inner))))
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }
    pub fn text(&self) -> Result<String> {
        Ok(self.0.inner.text())
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

    pub fn set_accelerator<S: AsRef<str>>(&self, accelerator: Option<S>) -> Result<()> {
        let accel = accelerator.and_then(|s| s.as_ref().parse().ok());
        self.0.inner.set_accelerator(accel).map_err(Into::into)
    }

    pub fn is_checked(&self) -> Result<bool> {
        Ok(self.0.inner.is_checked())
    }

    pub fn set_checked(&self, checked: bool) -> Result<()> {
        self.0.inner.set_checked(checked);
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// icon item
// -----------------------------------------------------------------------------

impl IconMenuItem {
    pub fn new<T: AsRef<str>, A: AsRef<str>>(
        text: T,
        enabled: bool,
        icon: Option<Image<'_>>,
        accelerator: Option<A>,
    ) -> Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let icon = icon.map(TryInto::try_into).transpose()?;
        let inner = muda::IconMenuItem::new(text.as_ref(), enabled, icon, accelerator);
        Ok(Self(Arc::new(IconMenuItemInner::new(inner))))
    }

    pub fn with_id<I: Into<MenuId>, T: AsRef<str>, A: AsRef<str>>(
        id: I,
        text: T,
        enabled: bool,
        icon: Option<Image<'_>>,
        accelerator: Option<A>,
    ) -> Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let icon = icon.map(TryInto::try_into).transpose()?;
        let inner = muda::IconMenuItem::with_id(id, text.as_ref(), enabled, icon, accelerator);
        Ok(Self(Arc::new(IconMenuItemInner::new(inner))))
    }

    pub fn with_native_icon<T: AsRef<str>, A: AsRef<str>>(
        text: T,
        enabled: bool,
        icon: Option<NativeIcon>,
        accelerator: Option<A>,
    ) -> Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::IconMenuItem::with_native_icon(
            text.as_ref(),
            enabled,
            icon.map(Into::into),
            accelerator,
        );
        Ok(Self(Arc::new(IconMenuItemInner::new(inner))))
    }

    pub fn with_id_and_native_icon<I: Into<MenuId>, T: AsRef<str>, A: AsRef<str>>(
        id: I,
        text: T,
        enabled: bool,
        icon: Option<NativeIcon>,
        accelerator: Option<A>,
    ) -> Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::IconMenuItem::with_id_and_native_icon(
            id,
            text.as_ref(),
            enabled,
            icon.map(Into::into),
            accelerator,
        );
        Ok(Self(Arc::new(IconMenuItemInner::new(inner))))
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }
    pub fn text(&self) -> Result<String> {
        Ok(self.0.inner.text())
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

    pub fn set_accelerator<S: AsRef<str>>(&self, accelerator: Option<S>) -> Result<()> {
        let accel = accelerator.and_then(|s| s.as_ref().parse().ok());
        self.0.inner.set_accelerator(accel).map_err(Into::into)
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

impl MenuItemKind {
    pub(crate) fn from_muda(item: muda::MenuItemKind) -> Self {
        match item {
            muda::MenuItemKind::MenuItem(v) => {
                Self::MenuItem(MenuItem(Arc::new(MenuItemInner::new(v))))
            }
            muda::MenuItemKind::Submenu(v) => {
                Self::Submenu(Submenu(Arc::new(SubmenuInner::new(v))))
            }
            muda::MenuItemKind::Predefined(v) => Self::Predefined(PredefinedMenuItem(Arc::new(
                PredefinedMenuItemInner::new(v),
            ))),
            muda::MenuItemKind::Check(v) => {
                Self::Check(CheckMenuItem(Arc::new(CheckMenuItemInner::new(v))))
            }
            muda::MenuItemKind::Icon(v) => {
                Self::Icon(IconMenuItem(Arc::new(IconMenuItemInner::new(v))))
            }
        }
    }
}

// -----------------------------------------------------------------------------
// predefined items: direct muda calls, no manager and no main-thread dispatch
// -----------------------------------------------------------------------------

impl PredefinedMenuItem {
    fn wrap(inner: muda::PredefinedMenuItem) -> Result<Self> {
        Ok(Self(Arc::new(PredefinedMenuItemInner::new(inner))))
    }

    pub fn separator() -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::separator())
    }
    pub fn copy(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::copy(text))
    }
    pub fn cut(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::cut(text))
    }
    pub fn paste(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::paste(text))
    }
    pub fn select_all(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::select_all(text))
    }
    pub fn undo(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::undo(text))
    }
    pub fn redo(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::redo(text))
    }
    pub fn minimize(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::minimize(text))
    }
    pub fn maximize(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::maximize(text))
    }
    pub fn fullscreen(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::fullscreen(text))
    }
    pub fn hide(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::hide(text))
    }
    pub fn hide_others(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::hide_others(text))
    }
    pub fn show_all(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::show_all(text))
    }
    pub fn close_window(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::close_window(text))
    }
    pub fn quit(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::quit(text))
    }
    pub fn services(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::services(text))
    }
    pub fn bring_all_to_front(text: Option<&str>) -> Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::bring_all_to_front(text))
    }

    pub fn about(text: Option<&str>, metadata: Option<AboutMetadata<'_>>) -> Result<Self> {
        let metadata = metadata.map(TryInto::try_into).transpose()?;
        Self::wrap(muda::PredefinedMenuItem::about(text, metadata))
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }
    pub fn text(&self) -> Result<String> {
        Ok(self.0.inner.text())
    }

    pub fn set_text<S: AsRef<str>>(&self, text: S) -> Result<()> {
        self.0.inner.set_text(text.as_ref());
        Ok(())
    }
}

impl sealed::IsMenuItemBase for MenuItemKind {
    fn inner_muda(&self) -> &dyn muda::IsMenuItem {
        self.inner_muda()
    }
}

impl IsMenuItem for MenuItemKind {
    fn kind(&self) -> MenuItemKind {
        self.clone()
    }

    fn id(&self) -> &MenuId {
        self.id()
    }
}
