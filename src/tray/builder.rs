use std::{path::Path, sync::Arc};

use muda::MenuEvent;

use crate::{prelude::ContextMenu, utils::image::Image};

use super::{
    /* icon::{attach_menu_handler, attach_tray_handler}, */
    TrayIcon, TrayIconEvent, TrayIconId,
};

type MenuHandler = Arc<dyn Fn(&TrayIcon, MenuEvent) + Send + Sync + 'static>;
type TrayHandler =
    Arc<dyn Fn(&TrayIcon, TrayIconEvent) + Send + Sync + 'static>;

/// [`TrayIcon`] builder without Tauri `AppHandle`, `Runtime` or `Manager`.
#[derive(Default)]
pub struct TrayIconBuilder {
    on_menu_event: Option<MenuHandler>,
    on_tray_icon_event: Option<TrayHandler>,
    inner: tray_icon::TrayIconBuilder,
}

impl TrayIconBuilder {
    pub fn new() -> Self {
        Self {
            inner: tray_icon::TrayIconBuilder::new(),
            on_menu_event: None,
            on_tray_icon_event: None,
        }
    }

    pub fn with_id<I: Into<TrayIconId>>(id: I) -> Self {
        let mut builder = Self::new();
        builder.inner = builder.inner.with_id(id);
        builder
    }

    pub fn menu<M: ContextMenu>(mut self, menu: &M) -> Self {
        self.inner = self.inner.with_menu(menu.inner_context_owned());
        self
    }

    pub fn icon(mut self, icon: Image<'_>) -> Self {
        if let Ok(icon) = icon.try_into() {
            self.inner = self.inner.with_icon(icon);
        }
        self
    }

    pub fn tooltip<S: AsRef<str>>(mut self, tooltip: S) -> Self {
        self.inner = self.inner.with_tooltip(tooltip);
        self
    }

    pub fn title<S: AsRef<str>>(mut self, title: S) -> Self {
        self.inner = self.inner.with_title(title);
        self
    }

    pub fn temp_dir_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.inner = self.inner.with_temp_dir_path(path);
        self
    }

    pub fn icon_as_template(mut self, is_template: bool) -> Self {
        self.inner = self.inner.with_icon_as_template(is_template);
        self
    }

    #[deprecated(
        since = "2.2.0",
        note = "Use `TrayIconBuilder::show_menu_on_left_click` instead."
    )]
    pub fn menu_on_left_click(mut self, enable: bool) -> Self {
        self.inner = self.inner.with_menu_on_left_click(enable);
        self
    }

    pub fn show_menu_on_left_click(mut self, enable: bool) -> Self {
        self.inner = self.inner.with_menu_on_left_click(enable);
        self
    }

    /// Same fluent shape as Tauri, but `TrayIcon` replaces `AppHandle` in the callback.
    pub fn on_menu_event<F>(mut self, f: F) -> Self
    where
        F: Fn(&TrayIcon, MenuEvent) + Sync + Send + 'static,
    {
        self.on_menu_event = Some(Arc::new(f));
        self
    }

    pub fn on_tray_icon_event<F>(mut self, f: F) -> Self
    where
        F: Fn(&TrayIcon, TrayIconEvent) + Sync + Send + 'static,
    {
        self.on_tray_icon_event = Some(Arc::new(f));
        self
    }

    pub fn id(&self) -> &TrayIconId {
        self.inner.id()
    }

    /// Build directly. No manager/app handle argument is required.
    pub fn build(self) -> crate::error::Result<TrayIcon> {
        let Self {
            on_menu_event: _,
            on_tray_icon_event: _,
            inner,
        } = self;
        let tray = TrayIcon::from_inner(inner.build()?);
        /*     attach_menu_handler(&tray, on_menu_event);
        attach_tray_handler(&tray, on_tray_icon_event); */
        Ok(tray)
    }
}
