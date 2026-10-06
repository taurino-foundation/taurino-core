use crate::{core::image::Image, menu::prelude::ContextMenu, trayicon::TrayIcon};
use anyhow;
use std::path::Path;
use tray_icon::TrayIconId;

#[derive(Default)]
pub struct TrayIconBuilder {
  inner: tray_icon::TrayIconBuilder,
}

impl TrayIconBuilder {
  pub fn new() -> Self {
    Self {
      inner: tray_icon::TrayIconBuilder::new(),
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

  #[deprecated(since = "2.2.0", note = "Use `TrayIconBuilder::show_menu_on_left_click` instead.")]
  pub fn menu_on_left_click(mut self, enable: bool) -> Self {
    self.inner = self.inner.with_menu_on_left_click(enable);
    self
  }

  pub fn show_menu_on_left_click(mut self, enable: bool) -> Self {
    self.inner = self.inner.with_menu_on_left_click(enable);
    self
  }

  pub fn id(&self) -> &TrayIconId {
    self.inner.id()
  }

  /// Build directly. No manager/app handle argument is required.
  pub fn build(self) -> anyhow::Result<TrayIcon> {
    let Self { inner } = self;
    let tray = TrayIcon::from_inner(inner.build()?);
    Ok(tray)
  }
}
