use std::collections::HashMap;

use crate::{
    app_error::{Error, Result},
    context::Context,
    utils::{UserEvent, WebViewId, WindowId, make_webview_label},
};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Window<T: UserEvent> {
    pub(crate) webview_label_id: HashMap<String, WebViewId>,
    pub(crate) window_id: WindowId,
    pub(crate) context: Context<T>,
}

impl<T: UserEvent> Window<T> {
    pub(crate) fn new(window_id: WindowId, context: Context<T>, webview_label_id: HashMap<String, WebViewId>) -> Self {
        Self {
            webview_label_id,
            window_id,
            context,
        }
    }
    #[allow(dead_code)]
    pub fn id(&self) -> WindowId {
        self.window_id
    }
    #[allow(dead_code)]
    pub fn contains_webview_label(&self, label: &str) -> bool {
        self.webview_label_id.contains_key(label)
    }
    #[allow(dead_code)]
    pub fn get_webview_id(&self, label: &str) -> Result<WebViewId> {
        self.webview_label_id
            .get(label)
            .copied()
            .ok_or(Error::LabelDoesNotExist)
    }
    #[allow(dead_code)]
    pub(crate) fn insert_webview_label(&mut self, label: impl Into<String>, webview_id: WebViewId) -> Result<()> {
        let label = label.into();

        if self.webview_label_id.contains_key(&label) {
            return Err(Error::LabelAlreadyExists);
        }

        self.webview_label_id.insert(label, webview_id);
        Ok(())
    }
    #[allow(dead_code)]
    pub fn internal_webview_label(&self, label: &str) -> Result<String> {
        let webview_id = self.get_webview_id(label)?;
        Ok(make_webview_label(self.window_id, label, webview_id))
    }
    #[allow(dead_code)]
    pub fn webview_labels(&self) -> impl Iterator<Item = &String> {
        self.webview_label_id.keys()
    }
}

// SAFETY: this is safe since the `Context` usage is guarded by `send_user_message`.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T: UserEvent> Sync for Window<T> {}
/* fn get_raw_window_handle<T: UserEvent>(
  dispatcher: &WryWindowDispatcher<T>,
) -> Result<std::result::Result<SendRawWindowHandle, raw_window_handle::HandleError>> {
  window_getter!(dispatcher, WindowMessage::RawWindowHandle)
}

 */
