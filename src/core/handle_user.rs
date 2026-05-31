use tao::event_loop::EventLoopWindowTarget;

use crate::{
    messages::Message,
    utils::{UserEvent, UserMessageContext},
};

pub(crate) fn handle_user_message<T: UserEvent>(
    #[allow(unused_variables)] event_loop: &EventLoopWindowTarget<Message<T>>,
    message: Message<T>,
    context: UserMessageContext,
) {
    let UserMessageContext {
        window_id_map: _,
        windows: _,
    } = context;
    match message {
        _ => {}
    }
}
