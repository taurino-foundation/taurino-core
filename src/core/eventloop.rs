use crate::{
    app_error::Result,
    context::Context,
    messages::Message,
    utils::{DispatcherMainThreadContext, UserEvent, WebContextStore, WindowIdStore, WindowsStore},
    window::Window,
    window_builder::WindowBuilder,
};
use std::{cell::RefCell, collections::BTreeMap, ops::Deref, sync::Arc, thread::current as current_thread};
use tao::event_loop::{EventLoop as TaoLoop, EventLoopBuilder};

#[derive(Debug, Clone)]
pub struct EventLoopHandle<T: UserEvent> {
    pub(crate) context: Context<T>,
}

impl<T: UserEvent> EventLoopHandle<T> {
    pub fn create_window(&self, builder: WindowBuilder) -> Result<Window<T>> {
        Ok(self.context.create_window(builder)?)
    }
}

#[allow(dead_code)]
pub struct EventLoop<T: UserEvent> {
    event_loop: TaoLoop<Message<T>>,
    context: Context<T>,
}

impl<T: UserEvent> EventLoop<T> {
    pub fn new() -> Result<Self> {
        Self::init_with_builder(EventLoopBuilder::<Message<T>>::with_user_event())
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn new_any_thread() -> Result<Self> {
        use tao::platform::unix::EventLoopBuilderExtUnix;
        let mut event_loop_builder = EventLoopBuilder::<Message<T>>::with_user_event();
        event_loop_builder.with_any_thread(true);
        Self::init_with_builder(event_loop_builder)
    }
    #[allow(dead_code)]
    #[cfg(windows)]
    fn new_any_thread() -> Result<Self> {
        use tao::platform::windows::EventLoopBuilderExtWindows;
        let mut event_loop_builder = EventLoopBuilder::<Message<T>>::with_user_event();
        event_loop_builder.with_any_thread(true);
        Self::init_with_builder(event_loop_builder)
    }

    fn init_with_builder(mut event_loop_builder: EventLoopBuilder<Message<T>>) -> Result<Self> {
        /*     #[cfg(windows)]
        let menu_manager = MenuManager::new();
        if let Some(hook) = menu_manager.msg_hook {
          use tao::platform::windows::EventLoopBuilderExtWindows;
          event_loop_builder.with_msg_hook(hook);


        }

        Self::init(event_loop_builder.build(), menu_manager)
        */

        /*     #[cfg(any(
          target_os = "linux",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "netbsd",
          target_os = "openbsd"
        ))]
        if let Some(app_id) = args.app_id {
          use tao::platform::unix::EventLoopBuilderExtUnix;
          event_loop_builder.with_app_id(app_id);
        } */
        Self::init(event_loop_builder.build())
    }

    fn init(event_loop: TaoLoop<Message<T>>) -> Result<Self> {
        let main_thread_id = current_thread().id();
        let web_context = WebContextStore::default();

        let windows = Arc::new(WindowsStore(RefCell::new(BTreeMap::default())));
        let window_id_map = WindowIdStore::default();

        let context = Context {
            window_id_map,
            main_thread_id,
            proxy: event_loop.create_proxy(),
            main_thread: DispatcherMainThreadContext {
                window_target: event_loop.deref().clone(),
                web_context,
                windows,
            },
            /* plugins: Default::default(), */
            next_window_id: Default::default(),
            next_webview_id: Default::default(),
            next_window_event_id: Default::default(),
            next_webview_event_id: Default::default(),
            webview_runtime_installed: wry::webview_version().is_ok(),
        };

        Ok(Self { context, event_loop })
    }
}
impl<T: UserEvent> EventLoop<T> {
    pub fn handle(&self) -> EventLoopHandle<T> {
        EventLoopHandle {
            context: self.context.clone(),
        }
    }
}
