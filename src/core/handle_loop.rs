use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopWindowTarget},
};

use crate::{
    messages::Message,
    utils::{EventLoopIterationContext, UserEvent},
};

#[allow(dead_code)]
pub(crate) fn handle_event_loop<T: UserEvent>(
    event: Event<'_, Message<T>>,
    #[allow(unused_variables)] event_loop: &EventLoopWindowTarget<Message<T>>,
    control_flow: &mut ControlFlow,
    context: EventLoopIterationContext<'_, T>,
) {
    let EventLoopIterationContext {
        callback: _,
        window_id_map: _,
        windows: _,
    } = context;
    if *control_flow != ControlFlow::Exit {
        *control_flow = ControlFlow::Wait;
    }

    match event {
        _ => {}
    }
}
