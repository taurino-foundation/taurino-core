use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, Once, OnceLock, Weak},
};

use muda::MenuEvent;

use crate::{
    prelude::ContextMenu,
    types::Rect,
    utils::{image::Image, resource::Resource},
};

use super::{TrayIconEvent, TrayIconId};

type MenuHandler = Arc<dyn Fn(&TrayIcon, MenuEvent) + Send + Sync + 'static>;
type TrayHandler = Arc<dyn Fn(&TrayIcon, TrayIconEvent) + Send + Sync + 'static>;

struct HandlerEntry {
    tray: Weak<TrayIconInner>,
    menu: Vec<MenuHandler>,
    tray_event: Vec<TrayHandler>,
}

static HANDLERS: OnceLock<Mutex<HashMap<TrayIconId, HandlerEntry>>> = OnceLock::new();
static INSTALL_DISPATCH: Once = Once::new();

fn handlers() -> &'static Mutex<HashMap<TrayIconId, HandlerEntry>> {
    HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn install_global_dispatch() {
    INSTALL_DISPATCH.call_once(|| {
        tray_icon::TrayIconEvent::set_event_handler(Some(|event: tray_icon::TrayIconEvent| {
            let event: TrayIconEvent = event.into();
            let id = event.id().clone();
            let (tray, callbacks) = {
                let map = handlers().lock().unwrap();
                let Some(entry) = map.get(&id) else { return };
                (entry.tray.upgrade(), entry.tray_event.clone())
            };
            if let Some(inner) = tray {
                let tray = TrayIcon { inner };
                for callback in callbacks {
                    callback(&tray, event.clone());
                }
            }
        }));

        muda::MenuEvent::set_event_handler(Some(|event: muda::MenuEvent| {
            let targets = {
                let map = handlers().lock().unwrap();
                map.values()
                    .filter_map(|entry| entry.tray.upgrade().map(|tray| (tray, entry.menu.clone())))
                    .collect::<Vec<_>>()
            };
            let event: MenuEvent = event.into();
            for (inner, callbacks) in targets {
                let tray = TrayIcon { inner };
                for callback in callbacks {
                    callback(&tray, event.clone());
                }
            }
        }));
    });
}

pub(super) struct TrayIconInner {
    pub(super) id: TrayIconId,
    pub(super) inner: tray_icon::TrayIcon,
}

// Same invariant as the standalone menu wrappers: creation and native mutation
// are expected to happen on the GUI/main thread.
unsafe impl Send for TrayIconInner {}
unsafe impl Sync for TrayIconInner {}

impl Drop for TrayIconInner {
    fn drop(&mut self) {
        if let Some(map) = HANDLERS.get() {
            map.lock().unwrap().remove(&self.id);
        }
    }
}

/// Standalone tray icon wrapper.
///
/// This type is reference-counted. The native icon is removed when the last
/// `TrayIcon` clone is dropped.
#[derive(Clone)]
pub struct TrayIcon {
    pub(super) inner: Arc<TrayIconInner>,
}

impl TrayIcon {
    pub(super) fn from_inner(inner: tray_icon::TrayIcon) -> Self {
        install_global_dispatch();
        let id = inner.id().clone();
        let this = Self {
            inner: Arc::new(TrayIconInner { id: id.clone(), inner }),
        };
        handlers().lock().unwrap().insert(
            id,
            HandlerEntry {
                tray: Arc::downgrade(&this.inner),
                menu: Vec::new(),
                tray_event: Vec::new(),
            },
        );
        this
    }

    pub fn id(&self) -> &TrayIconId {
        &self.inner.id
    }

    pub fn on_menu_event<F>(&self, f: F)
    where
        F: Fn(&TrayIcon, MenuEvent) + Sync + Send + 'static,
    {
        if let Some(entry) = handlers().lock().unwrap().get_mut(self.id()) {
            entry.menu.push(Arc::new(f));
        }
    }

    pub fn on_tray_icon_event<F>(&self, f: F)
    where
        F: Fn(&TrayIcon, TrayIconEvent) + Sync + Send + 'static,
    {
        if let Some(entry) = handlers().lock().unwrap().get_mut(self.id()) {
            entry.tray_event.push(Arc::new(f));
        }
    }

    pub fn set_icon(&self, icon: Option<Image<'_>>) -> crate::error::Result<()> {
        let icon = icon.map(TryInto::try_into).transpose()?;
        self.inner.inner.set_icon(icon).map_err(Into::into)
    }

    pub fn set_menu<M: ContextMenu + 'static>(&self, menu: Option<M>) -> crate::error::Result<()> {
        self.inner.inner.set_menu(menu.map(|m| m.inner_context_owned()));
        Ok(())
    }

    pub fn set_tooltip<S: AsRef<str>>(&self, tooltip: Option<S>) -> crate::error::Result<()> {
        self.inner
            .inner
            .set_tooltip(tooltip.map(|s| s.as_ref().to_string()))
            .map_err(Into::into)
    }

    pub fn set_title<S: AsRef<str>>(&self, title: Option<S>) -> crate::error::Result<()> {
        self.inner.inner.set_title(title.map(|s| s.as_ref().to_string()));
        Ok(())
    }

    pub fn set_visible(&self, visible: bool) -> crate::error::Result<()> {
        self.inner.inner.set_visible(visible).map_err(Into::into)
    }

    pub fn set_temp_dir_path<P: AsRef<Path>>(&self, path: Option<P>) -> crate::error::Result<()> {
        #[cfg(target_os = "linux")]
        self.inner
            .inner
            .set_temp_dir_path(path.map(|p| p.as_ref().to_path_buf()));
        let _ = path;
        Ok(())
    }

    pub fn set_icon_as_template(&self, #[allow(unused)] is_template: bool) -> crate::error::Result<()> {
        #[cfg(target_os = "macos")]
        self.inner.inner.set_icon_as_template(is_template);
        Ok(())
    }

    pub fn set_icon_with_as_template(
        &self,
        icon: Option<Image<'_>>,
        #[allow(unused)] is_template: bool,
    ) -> crate::error::Result<()> {
        #[cfg(target_os = "macos")]
        {
            let icon = icon.map(TryInto::try_into).transpose()?;
            self.inner
                .inner
                .set_icon_with_as_template(icon, is_template)
                .map_err(Into::into)?;
            return Ok(());
        }
        #[cfg(not(target_os = "macos"))]
        self.set_icon(icon)
    }

    pub fn set_show_menu_on_left_click(&self, #[allow(unused)] enable: bool) -> crate::error::Result<()> {
        #[cfg(any(target_os = "macos", windows))]
        self.inner.inner.set_show_menu_on_left_click(enable);
        Ok(())
    }

    pub fn rect(&self) -> crate::error::Result<Option<Rect>> {
        Ok(self.inner.inner.rect().map(|rect| Rect {
            position: rect.position.into(),
            size: rect.size.into(),
        }))
    }

    pub fn with_inner_tray_icon<F, T>(&self, f: F) -> crate::error::Result<T>
    where
        F: FnOnce(&tray_icon::TrayIcon) -> T,
    {
        Ok(f(&self.inner.inner))
    }
}

impl Resource for TrayIcon {}

/* pub(super) fn attach_menu_handler(tray: &TrayIcon, handler: Option<MenuHandler>) {
  if let Some(handler) = handler {
    if let Some(entry) = handlers().lock().unwrap().get_mut(tray.id()) {
      entry.menu.push(handler);
    }
  }
}

pub(super) fn attach_tray_handler(tray: &TrayIcon, handler: Option<TrayHandler>) {
  if let Some(handler) = handler {
    if let Some(entry) = handlers().lock().unwrap().get_mut(tray.id()) {
      entry.tray_event.push(handler);
    }
  }
}
 */
