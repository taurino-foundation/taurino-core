use std::sync::{Arc, Mutex};

use crate::{
    context::Context,
    utils::{UserEvent, WebViewId, WindowId},
};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WebView<T: UserEvent> {
    window_id: Arc<Mutex<WindowId>>,
    webview_id: WebViewId,
    context: Context<T>,
}

impl<T: UserEvent> WebView<T> {}
