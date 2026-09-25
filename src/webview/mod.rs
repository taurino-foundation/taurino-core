use dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use std::{ops::Deref, path::PathBuf, rc::Rc};
use tao::window::Window;
use url::Url;
use wry::{WebView, cookie::Cookie};
mod builder;
pub(crate) mod factory;
use crate::{
    types::{WebContextStore, WebviewBounds},
    utils::{ArcMut, WindowWebViewMetaData, wrappers::RectWrapper},
};

pub use self::builder::WebViewBuilder;
#[derive(Clone)]
pub struct ManagedWebview {
    pub(crate) metadata: WindowWebViewMetaData,
    pub inner: Rc<WebView>,
    pub context_store: WebContextStore,
    pub context_key: Option<PathBuf>,
    pub bounds: ArcMut<Option<WebviewBounds>>,
}

impl Deref for ManagedWebview {
    type Target = WebView;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Drop for ManagedWebview {
    fn drop(&mut self) {
        if Rc::get_mut(&mut self.inner).is_some() {
            let mut context_store = self.context_store.lock().unwrap();

            if let Some(web_context) = context_store.get_mut(&self.context_key)
            {
                web_context
                    .referenced_by_webviews
                    .remove(&self.metadata.webview_label);

                // https://github.com/tauri-apps/tauri/issues/14626
                // Because WebKit does not close its network process even when no webviews are running,
                // we need to ensure to re-use the existing process on Linux by keeping the WebContext
                // alive for the lifetime of the app.
                // WebKit on macOS handles this itself.
                #[cfg(not(any(
                    target_os = "linux",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd"
                )))]
                if web_context.referenced_by_webviews.is_empty() {
                    context_store.remove(&self.context_key);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct WebViewId(u32);

impl From<u32> for WebViewId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl ManagedWebview {
    // -------------------------------------------------------------------------
    // Commands
    // -------------------------------------------------------------------------

    pub fn evaluate_script(&self, script: &str) -> crate::error::Result<()> {
        self.inner
            .evaluate_script(script)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn evaluate_script_with_callback<F>(
        &self,
        script: &str,
        callback: F,
    ) -> crate::error::Result<()>
    where
        F: Fn(String) + Send + 'static,
    {
        self.inner
            .evaluate_script_with_callback(script, callback)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn navigate(&self, url: &Url) -> crate::error::Result<()> {
        self.inner
            .load_url(url.as_str())
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn reload(&self) -> crate::error::Result<()> {
        self.inner
            .reload()
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn show(&self) -> crate::error::Result<()> {
        self.inner
            .set_visible(true)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn hide(&self) -> crate::error::Result<()> {
        self.inner
            .set_visible(false)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn print(&self) -> crate::error::Result<()> {
        self.inner
            .print()
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    // -------------------------------------------------------------------------
    // Bounds
    // -------------------------------------------------------------------------

    pub fn set_bounds(
        &self,
        window: &Window,
        bounds: crate::types::Rect,
    ) -> crate::error::Result<()> {
        let bounds: RectWrapper = bounds.into();
        let bounds = bounds.0;

        if let Some(b) = &mut *self.bounds.lock().unwrap() {
            let scale_factor = window.scale_factor();

            let size = bounds.size.to_logical::<f32>(scale_factor);

            let position = bounds.position.to_logical::<f32>(scale_factor);

            let window_size =
                window.inner_size().to_logical::<f32>(scale_factor);

            b.width_rate = size.width / window_size.width;

            b.height_rate = size.height / window_size.height;

            b.x_rate = position.x / window_size.width;

            b.y_rate = position.y / window_size.height;
        }

        self.inner
            .set_bounds(bounds)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn set_size(
        &self,
        window: &Window,
        size: Size,
    ) -> crate::error::Result<()> {
        let mut bounds = self
            .inner
            .bounds()
            .map_err(|_| crate::error::Error::FailedToSendMessage)?;

        bounds.size = size;

        let scale_factor = window.scale_factor();
        let size = size.to_logical::<f32>(scale_factor);

        if let Some(b) = &mut *self.bounds.lock().unwrap() {
            let window_size =
                window.inner_size().to_logical::<f32>(scale_factor);

            b.width_rate = size.width / window_size.width;

            b.height_rate = size.height / window_size.height;
        }

        self.inner
            .set_bounds(bounds)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn set_position(
        &self,
        window: &Window,
        position: Position,
    ) -> crate::error::Result<()> {
        let mut bounds = self
            .inner
            .bounds()
            .map_err(|_| crate::error::Error::FailedToSendMessage)?;

        bounds.position = position;

        let scale_factor = window.scale_factor();

        let position = position.to_logical::<f32>(scale_factor);

        if let Some(b) = &mut *self.bounds.lock().unwrap() {
            let window_size =
                window.inner_size().to_logical::<f32>(scale_factor);

            b.x_rate = position.x / window_size.width;

            b.y_rate = position.y / window_size.height;
        }

        self.inner
            .set_bounds(bounds)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    // -------------------------------------------------------------------------
    // Appearance / state
    // -------------------------------------------------------------------------

    pub fn set_zoom(&self, scale_factor: f64) -> crate::error::Result<()> {
        self.inner
            .zoom(scale_factor)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn set_background_color(
        &self,
        color: Option<crate::types::Color>,
    ) -> crate::error::Result<()> {
        self.inner
            .set_background_color(
                color.map(Into::into).unwrap_or((255, 255, 255, 255)),
            )
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn clear_all_browsing_data(&self) -> crate::error::Result<()> {
        self.inner
            .clear_all_browsing_data()
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    // -------------------------------------------------------------------------
    // Getters
    // -------------------------------------------------------------------------

    pub fn url(&self) -> crate::error::Result<Url> {
        self.inner
            .url()
            .map(|url| url.parse().expect("invalid webview URL"))
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn bounds(&self) -> crate::error::Result<crate::types::Rect> {
        self.inner
            .bounds()
            .map(|bounds| crate::types::Rect {
                size: bounds.size,
                position: bounds.position,
            })
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn position(
        &self,
        window: &Window,
    ) -> crate::error::Result<PhysicalPosition<i32>> {
        self.inner
            .bounds()
            .map(|bounds| bounds.position.to_physical(window.scale_factor()))
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn size(
        &self,
        window: &Window,
    ) -> crate::error::Result<PhysicalSize<u32>> {
        self.inner
            .bounds()
            .map(|bounds| bounds.size.to_physical(window.scale_factor()))
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    // -------------------------------------------------------------------------
    // Focus / autoresize
    // -------------------------------------------------------------------------

    pub fn set_focus(&self) -> crate::error::Result<()> {
        self.inner
            .focus()
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn set_auto_resize(
        &self,
        window: &Window,
        auto_resize: bool,
    ) -> crate::error::Result<()> {
        let bounds = self
            .inner
            .bounds()
            .map_err(|_| crate::error::Error::FailedToSendMessage)?;

        let scale_factor = window.scale_factor();

        let window_size = window.inner_size().to_logical::<f32>(scale_factor);

        *self.bounds.lock().unwrap() = if auto_resize {
            let size = bounds.size.to_logical::<f32>(scale_factor);

            let position = bounds.position.to_logical::<f32>(scale_factor);

            Some(WebviewBounds {
                x_rate: position.x / window_size.width,

                y_rate: position.y / window_size.height,

                width_rate: size.width / window_size.width,

                height_rate: size.height / window_size.height,
            })
        } else {
            None
        };

        Ok(())
    }

    // -------------------------------------------------------------------------
    // DevTools
    // -------------------------------------------------------------------------

    #[cfg(any(debug_assertions, feature = "devtools"))]
    pub fn open_devtools(&self) {
        self.inner.open_devtools();
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    pub fn close_devtools(&self) {
        self.inner.close_devtools();
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    pub fn is_devtools_open(&self) -> bool {
        self.inner.is_devtools_open()
    }
    pub fn cookies(&self) -> crate::error::Result<Vec<Cookie<'static>>> {
        self.inner
            .cookies()
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn set_cookie(&self, cookie: &Cookie<'_>) -> crate::error::Result<()> {
        self.inner
            .set_cookie(cookie)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn delete_cookie(
        &self,
        cookie: &Cookie<'_>,
    ) -> crate::error::Result<()> {
        self.inner
            .delete_cookie(cookie)
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }

    pub fn cookies_for_url(
        &self,
        url: &Url,
    ) -> crate::error::Result<Vec<Cookie<'static>>> {
        self.inner
            .cookies_for_url(url.as_str())
            .map_err(|_| crate::error::Error::FailedToSendMessage)
    }
}
