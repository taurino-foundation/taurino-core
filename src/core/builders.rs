use tao::event_loop::EventLoopWindowTarget;

use crate::{
    app_error::Result,
    context::Context,
    messages::Message,
    utils::{UserEvent, WebViewId, WindowId, WindowWrapper},
    window_builder::WindowBuilder,
};
pub(crate) fn create_window<T: UserEvent>(
    #[allow(unused_variables)] window_id: WindowId,
    #[allow(unused_variables)] webview_id: WebViewId,
    #[allow(unused_variables)] event_loop: &EventLoopWindowTarget<Message<T>>,
    #[allow(unused_variables)] context: &Context<T>,
    #[allow(unused_variables)] builder: WindowBuilder,
) -> Result<WindowWrapper> {
    Ok(WindowWrapper {})
}
