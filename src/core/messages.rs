use tao::event_loop::EventLoopWindowTarget;

use crate::{
    app_error::Result,
    utils::{UserEvent, WindowId, WindowWrapper},
};

/*

pub type CreateWindowClosure<T> =
  Box<dyn FnOnce(&EventLoopWindowTarget<Message<T>>) -> Result<WindowWrapper> + Send>;

pub type CreateWebviewClosure =
  Box<dyn FnOnce(&Window, CreateWebviewOptions) -> Result<WebviewWrapper> + Send>;

pub struct CreateWebviewOptions {
  pub focused_webview: Arc<Mutex<Option<String>>>,
}

*/

pub type CreateWindowClosure<T> = Box<dyn FnOnce(&EventLoopWindowTarget<Message<T>>) -> Result<WindowWrapper> + Send>;

#[allow(dead_code)]
pub enum Message<T: 'static> {
    CreateWindow(WindowId, CreateWindowClosure<T>),
    UserEvent(T),
}

impl<T: UserEvent> Clone for Message<T> {
    fn clone(&self) -> Self {
        match self {
            Self::UserEvent(arg0) => Self::UserEvent(arg0.clone()),
            #[allow(unreachable_patterns)]
            _ => unimplemented!(),
        }
    }
}
