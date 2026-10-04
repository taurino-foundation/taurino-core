// -----------------------------------------------------------------------------
// menu/mod.rs
// -----------------------------------------------------------------------------

mod builder;
mod context;
mod item;
mod menu;
mod metadata;

#[cfg(windows)]
use crate::schema::Theme;
use muda::MenuId;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[cfg(windows)]
use tao::window::Theme as TaoTheme;

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "linux",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use gtk;

use crate::menu::item::Menu;

// -----------------------------------------------------------------------------
// RawWindow
// -----------------------------------------------------------------------------

/// Platform-specific view of a freshly created native window.
///
/// The lifetime is tied directly to the underlying Tao window. This prevents a
/// `RawWindow` from outliving the native window from which its platform handles
/// were obtained.
///
/// A `RawWindow` is intentionally short-lived. It is created for callbacks and
/// native initialization and must not be stored beyond those operations.
pub struct RawWindow<'a> {
    #[cfg(windows)]
    pub hwnd: isize,
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub gtk_window: &'a gtk::ApplicationWindow,
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub default_vbox: Option<&'a gtk::Box>,
    pub _marker: &'a std::marker::PhantomData<()>,
}

pub struct WindowMenu {
    pub is_app_wide: bool,
    pub menu: Menu,
}

// -----------------------------------------------------------------------------
// Theme mapping
//
// `muda::MenuTheme` is primarily meaningful on Windows. The mapping functions
// remain available on every platform so callers do not need platform-specific
// cfg gates.
// -----------------------------------------------------------------------------

#[cfg(windows)]
pub fn map_theme_to_muda(theme: Theme) -> muda::MenuTheme {
    match theme {
        Theme::Light => muda::MenuTheme::Light,
        Theme::Dark => muda::MenuTheme::Dark,

        #[allow(unreachable_patterns)]
        _ => muda::MenuTheme::Auto,
    }
}
#[cfg(windows)]
pub fn map_muda_theme_from_tao(theme: TaoTheme) -> muda::MenuTheme {
    match theme {
        TaoTheme::Light => muda::MenuTheme::Light,
        TaoTheme::Dark => muda::MenuTheme::Dark,
        _ => muda::MenuTheme::Auto,
    }
}

// -----------------------------------------------------------------------------
// MenuManager
//
// Owns every live menu known to the application.
//
// A menu is either:
//
//   * application-wide
//       macOS uses one native menu bar for the entire application.
//
//   * per-window
//       Windows and desktop Unix platforms may initialize a separate native
//       menu for each window.
//
// `menus` is the authoritative stash of known menu objects and is also used by
// the Windows accelerator/message hook.
// -----------------------------------------------------------------------------

pub struct MenuManager {
    menus: HashMap<MenuId, Menu>,
    app_wide_menu: Option<Menu>,
}

impl MenuManager {
    pub fn new() -> anyhow::Result<Arc<Mutex<Self>>> {
        Ok(Arc::new(Mutex::new(Self {
            menus: HashMap::new(),
            app_wide_menu: None,
        })))
    }

    // -------------------------------------------------------------------------
    // Read access
    // -------------------------------------------------------------------------

    pub fn menus(&self) -> impl Iterator<Item = (&MenuId, &Menu)> {
        self.menus.iter()
    }

    pub fn get_menu(&self, id: &MenuId) -> Option<&Menu> {
        self.menus.get(id)
    }

    pub fn app_wide_menu(&self) -> Option<&Menu> {
        self.app_wide_menu.as_ref()
    }

    pub fn is_menu_in_use(&self, id: &MenuId) -> bool {
        self.menus.contains_key(id)
    }

    // -------------------------------------------------------------------------
    // Mutation
    // -------------------------------------------------------------------------

    pub fn update_menu(&mut self, menu: Menu) -> Option<Menu> {
        self.menus.insert(menu.id().clone(), menu)
    }

    pub fn insert_menu_into_stash(&mut self, menu: &Menu) {
        self.menus.entry(menu.id().clone()).or_insert_with(|| menu.clone());
    }

    pub fn set_app_wide_menu(&mut self, menu: Menu) -> Option<Menu> {
        let previous = self.app_wide_menu.replace(menu.clone());

        if let Some(previous) = &previous {
            self.menus.remove(previous.id());
        }

        self.insert_menu_into_stash(&menu);

        previous
    }

    pub fn remove_app_wide_menu(&mut self) -> Option<Menu> {
        let menu = self.app_wide_menu.take()?;
        let id = menu.id().clone();

        self.menus.remove(&id);

        Some(menu)
    }

    pub fn remove_menu_if_not_app_wide(&mut self, id: &MenuId) -> Option<Menu> {
        if self.app_wide_menu.as_ref().is_some_and(|menu| menu.id() == id) {
            return None;
        }

        self.menus.remove(id)
    }

    pub fn clear(&mut self) {
        self.app_wide_menu = None;
        self.menus.clear();
    }

    // -------------------------------------------------------------------------
    // Window menu creation
    //
    // Called after the native Tao window exists, because platform-specific
    // initialization may need its native window handle.
    //
    // Contract:
    //
    // macOS:
    //   * If an application-wide menu already exists, return that menu.
    //   * Otherwise, use the explicit builder menu when supplied.
    //   * If no explicit menu exists, create an empty menu.
    //   * Register the resulting menu as the application-wide menu and
    //     initialize it for NSApp.
    //
    // Windows:
    //   * Use the explicit builder menu when supplied.
    //   * Otherwise create an empty menu.
    //   * Register it in the manager.
    //   * Initialize it for this window's HWND and theme.
    //
    // Linux / BSD:
    //   * Use the explicit builder menu when supplied.
    //   * Otherwise create an empty menu.
    //   * Register it in the manager.
    //   * GTK-specific attachment may happen later when the final container
    //     hierarchy is available.
    // -------------------------------------------------------------------------

    pub fn create_window_menu(
        &mut self,
        raw: RawWindow<'_>,
        theme: Theme,
        explicit_menu: Option<Menu>,
    ) -> anyhow::Result<WindowMenu> {
        // ---------------------------------------------------------------------
        // macOS: exactly one menu bar for the application
        // ---------------------------------------------------------------------

        #[cfg(target_os = "macos")]
        {
            // Once an application-wide menu exists, every window references
            // that same menu. A per-window explicit menu cannot replace it
            // implicitly.
            if let Some(menu) = self.app_wide_menu.clone() {
                self.insert_menu_into_stash(&menu);

                let _ = (raw, theme, explicit_menu);

                return Ok(WindowMenu {
                    is_app_wide: true,
                    menu,
                });
            }

            // The first explicit builder menu becomes the application-wide
            // menu. If none was supplied, create an empty application menu.
            let menu = match explicit_menu {
                Some(menu) => menu,
                None => Menu::new()?,
            };

            self.set_app_wide_menu(menu.clone());

            menu.init_for_nsapp();

            // `raw` and `theme` are intentionally unused on macOS at present,
            // but remain part of the uniform cross-platform API.
            let _ = (raw, theme);

            return Ok(WindowMenu {
                is_app_wide: true,
                menu,
            });
        }

        // ---------------------------------------------------------------------
        // Windows / Linux / BSD: menu belongs to this window
        // ---------------------------------------------------------------------

        #[cfg(not(target_os = "macos"))]
        {
            // Crucially, use the explicit WindowBuilder menu instead of merely
            // registering it and then creating a different empty menu.
            let menu = match explicit_menu {
                Some(menu) => menu,
                None => Menu::new()?,
            };

            self.insert_menu_into_stash(&menu);

            // -----------------------------------------------------------------
            // Windows native initialization
            // -----------------------------------------------------------------

            #[cfg(windows)]
            {
                let muda_theme = map_theme_to_muda(theme);

                unsafe {
                    menu.init_for_hwnd_with_theme(raw.hwnd, muda_theme)?;
                }
            }

            // -----------------------------------------------------------------
            // Linux / BSD
            // -----------------------------------------------------------------

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                // The final GTK container may only become available after the
                // webview/container hierarchy has been assembled. Keep these
                // handles available through RawWindow, while actual GTK menu
                // attachment may be performed by the corresponding caller.
                let _ = (raw.gtk_window, raw.default_vbox, theme);
            }

            // Other non-macOS platforms currently require no native menu
            // initialization here.
            #[cfg(not(any(
                windows,
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            )))]
            {
                let _ = (raw, theme);
            }

            Ok(WindowMenu {
                is_app_wide: false,
                menu,
            })
        }
    }

    // -------------------------------------------------------------------------
    // Windows message hook
    // -------------------------------------------------------------------------

    #[cfg(windows)]
    pub fn install_msg_hook(manager: Arc<Mutex<Self>>) -> Box<dyn FnMut(*const std::ffi::c_void) -> bool + 'static> {
        Box::new(move |msg| {
            use windows::Win32::UI::WindowsAndMessaging::{HACCEL, MSG, TranslateAcceleratorW};

            unsafe {
                use crate::utils::lock_state;

                let msg = msg as *const MSG;

                if msg.is_null() {
                    return false;
                }

                let Ok(manager) = lock_state(&manager, "MenuManager") else {
                    return false;
                };

                for menu in manager.menus.values() {
                    let translated = TranslateAcceleratorW((*msg).hwnd, HACCEL(menu.inner().haccel() as _), msg);

                    if translated == 1 {
                        return true;
                    }
                }

                false
            }
        })
    }
}

// -----------------------------------------------------------------------------
// muda event handler -> forward to Tao
//
// Must be installed exactly once before any menu is displayed.
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

// -----------------------------------------------------------------------------
// Prelude
// -----------------------------------------------------------------------------

pub mod prelude {
    pub use super::builder::{CheckMenuItemBuilder, IconMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};

    pub use super::context::ContextMenu;

    pub use super::install_menu_event_handler;

    pub use super::item::{
        CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu,
    };

    pub use super::metadata::{AboutMetadata, AboutMetadataBuilder, NativeIcon};

    #[cfg(windows)]
    pub use super::{map_muda_theme_from_tao, map_theme_to_muda};
}
