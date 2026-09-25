use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
};

use anyhow::{anyhow, Result};
use tao::window::Window;
use wry::WebView;

pub type WindowId = String;

#[derive(Clone)]
pub struct ManagedWebView {
    inner: Rc<WebView>,
}

impl ManagedWebView {
    pub fn new(webview: WebView) -> Self {
        Self {
            inner: Rc::new(webview),
        }
    }

    pub fn inner(&self) -> Rc<WebView> {
        Rc::clone(&self.inner)
    }
}

pub struct ManagedWindow {
    inner: Arc<Window>,
    webviews: Vec<ManagedWebView>,
}

impl ManagedWindow {
    pub fn new(window: Arc<Window>) -> Self {
        Self {
            inner: window,
            webviews: Vec::new(),
        }
    }

    pub fn inner(&self) -> Arc<Window> {
        Arc::clone(&self.inner)
    }

    pub fn add_webview(&mut self, webview: WebView) -> ManagedWebView {
        let webview = ManagedWebView::new(webview);

        self.webviews.push(webview.clone());

        webview
    }

    pub fn webview(&self, index: usize) -> Result<ManagedWebView> {
        self.webviews
            .get(index)
            .cloned()
            .ok_or_else(|| anyhow!("webview at index `{index}` not found"))
    }

    pub fn webviews(&self) -> Vec<ManagedWebView> {
        self.webviews.clone()
    }

    pub fn remove_webview(&mut self, index: usize) -> Result<ManagedWebView> {
        if index >= self.webviews.len() {
            return Err(anyhow!(
                "webview at index `{index}` not found"
            ));
        }

        Ok(self.webviews.remove(index))
    }

    pub fn clear_webviews(&mut self) {
        self.webviews.clear();
    }
}

pub type ManagedWindowHandle = Rc<RefCell<ManagedWindow>>;

#[derive(Clone, Default)]
pub struct WindowManager {
    windows: Rc<RefCell<BTreeMap<WindowId, ManagedWindowHandle>>>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &self,
        id: impl Into<WindowId>,
        window: Arc<Window>,
    ) -> Result<ManagedWindowHandle> {
        let id = id.into();

        let mut windows = self.windows.borrow_mut();

        if windows.contains_key(&id) {
            return Err(anyhow!(
                "window `{id}` already exists"
            ));
        }

        let managed = Rc::new(RefCell::new(
            ManagedWindow::new(window),
        ));

        windows.insert(id, Rc::clone(&managed));

        Ok(managed)
    }

    pub fn get(&self, id: &str) -> Result<ManagedWindowHandle> {
        self.windows
            .borrow()
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("window `{id}` not found"))
    }

    pub fn remove(&self, id: &str) -> Result<ManagedWindowHandle> {
        self.windows
            .borrow_mut()
            .remove(id)
            .ok_or_else(|| anyhow!("window `{id}` not found"))
    }

    pub fn contains(&self, id: &str) -> bool {
        self.windows.borrow().contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.windows.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.borrow().is_empty()
    }

    pub fn clear(&self) {
        self.windows.borrow_mut().clear();
    }

    pub fn ids(&self) -> Vec<WindowId> {
        self.windows
            .borrow()
            .keys()
            .cloned()
            .collect()
    }

    pub fn add_webview(
        &self,
        window_id: &str,
        webview: WebView,
    ) -> Result<ManagedWebView> {
        let window = self.get(window_id)?;

        let webview = window
            .borrow_mut()
            .add_webview(webview);

        Ok(webview)
    }
}