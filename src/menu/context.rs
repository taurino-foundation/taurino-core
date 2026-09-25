use dpi::Position;
use muda::ContextMenu as MudaContextMenu;
use tao::window::Window;
use crate::error::{Result};
use crate::menu::sealed::ContextMenuBase;

use super::item::{Menu, Submenu};

/// A helper trait with methods to help creating a context menu.
///
/// # Safety
///
/// This trait is ONLY meant to be implemented internally by the crate.
pub trait ContextMenu: sealed::ContextMenuBase + Send + Sync {
    /// Get the popup [`HMENU`] for this menu.
    ///
    /// The returned [`HMENU`] is valid as long as the [`ContextMenu`] is.
    ///
    /// [`HMENU`]: https://learn.microsoft.com/en-us/windows/win32/winprog/windows-data-types#HMENU
    #[cfg(windows)]
    #[cfg_attr(docsrs, doc(cfg(windows)))]
    fn hpopupmenu(&self) -> Result<isize>;

    /// Popup this menu as a context menu on the specified window at the cursor position.
    fn popup(&self, window: &Window) -> Result<()>;

    /// Popup this menu as a context menu on the specified window at the specified position.
    ///
    /// The position is relative to the window's top-left corner.
    fn popup_at<P: Into<Position>>(
        &self,
        window: &Window,
        position: P,
    ) -> Result<()>;
}

pub(crate) mod sealed {
    use dpi::Position;
    use tao::window::Window;
    use crate::error::{Result};

    pub trait IsMenuItemBase {
        fn inner_muda(&self) -> &dyn muda::IsMenuItem;
    }

    pub trait ContextMenuBase {
        fn inner_context(&self) -> &dyn muda::ContextMenu;
        fn inner_context_owned(&self) -> Box<dyn muda::ContextMenu>;
        fn popup_inner<P: Into<Position>>(
            &self,
            window: &Window,
            position: Option<P>,
        ) -> Result<()>;
    }
}

fn show_context_menu(
    menu: &dyn MudaContextMenu,
    window: &Window,
    position: Option<Position>,
) -> Result<()> {
    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;

        unsafe {
            menu.show_context_menu_for_hwnd(window.hwnd(), position);
        }
    }

    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::WindowExtMacOS;

        unsafe {
            menu.show_context_menu_for_nsview(window.ns_view() as _, position);
        }
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        use tao::platform::unix::WindowExtUnix;

        menu.show_context_menu_for_gtk_window(
            window.gtk_window().as_ref(),
            position,
        );
    }

    Ok(())
}

impl ContextMenu for Menu {
    #[cfg(windows)]
    fn hpopupmenu(&self) -> Result<isize> {
        Ok(self.0.inner.hpopupmenu())
    }

    fn popup(&self, window: &Window) -> Result<()> {
        self.popup_inner(window, None::<Position>)
    }

    fn popup_at<P: Into<Position>>(
        &self,
        window: &Window,
        position: P,
    ) -> Result<()> {
        self.popup_inner(window, Some(position))
    }
}

impl sealed::ContextMenuBase for Menu {
    fn inner_context(&self) -> &dyn muda::ContextMenu {
        &self.0.inner
    }

    fn inner_context_owned(&self) -> Box<dyn muda::ContextMenu> {
        Box::new(self.0.inner.clone())
    }

    fn popup_inner<P: Into<Position>>(
        &self,
        window: &Window,
        position: Option<P>,
    ) -> Result<()> {
        show_context_menu(&self.0.inner, window, position.map(Into::into))
    }
}

impl ContextMenu for Submenu {
    #[cfg(windows)]
    fn hpopupmenu(&self) -> Result<isize> {
        Ok(self.0.inner.hpopupmenu())
    }

    fn popup(&self, window: &Window) -> Result<()> {
        self.popup_inner(window, None::<Position>)
    }

    fn popup_at<P: Into<Position>>(
        &self,
        window: &Window,
        position: P,
    ) -> Result<()> {
        self.popup_inner(window, Some(position))
    }
}

impl sealed::ContextMenuBase for Submenu {
    fn inner_context(&self) -> &dyn muda::ContextMenu {
        &self.0.inner
    }

    fn inner_context_owned(&self) -> Box<dyn muda::ContextMenu> {
        Box::new(self.0.inner.clone())
    }

    fn popup_inner<P: Into<Position>>(
        &self,
        window: &Window,
        position: Option<P>,
    ) -> Result<()> {
        show_context_menu(&self.0.inner, window, position.map(Into::into))
    }
}
