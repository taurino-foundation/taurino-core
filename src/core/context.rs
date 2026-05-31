use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    thread::{ThreadId, current as current_thread},
};
use tao::event_loop::EventLoopProxy;

use crate::{
    app_error::{Error, Result},
    builders::create_window,
    handle_user::handle_user_message,
    utils::{DispatcherMainThreadContext, UserMessageContext, WebViewId, WindowId, WindowIdStore},
    window::Window,
    window_builder::WindowBuilder,
};
use crate::{messages::Message, utils::UserEvent};

#[macro_export]
macro_rules! getter {
    ($self: ident, $rx: expr, $message: expr) => {{
        $crate::send_user_message(&$self.context, $message)?;
        $rx.recv().map_err(|_| $crate::Error::FailedToReceiveMessage)
    }};
}

#[allow(unused_macros)]
macro_rules! window_getter {
    ($self: ident, $message: expr) => {{
        let (tx, rx) = channel();
        getter!($self, rx, Message::Window($self.window_id, $message(tx)))
    }};
}

#[allow(unused_macros)]
macro_rules! event_loop_window_getter {
    ($self: ident, $message: expr) => {{
        let (tx, rx) = channel();
        getter!($self, rx, Message::EventLoopWindowTarget($message(tx)))
    }};
}

#[allow(unused_macros)]
macro_rules! webview_getter {
    ($self: ident, $message: expr) => {{
        let (tx, rx) = channel();
        getter!(
            $self,
            rx,
            Message::Webview(*$self.window_id.lock().unwrap(), $self.webview_id, $message(tx))
        )
    }};
}

pub(crate) fn send_user_message<T: UserEvent>(context: &Context<T>, message: Message<T>) -> Result<()> {
    if current_thread().id() == context.main_thread_id {
        handle_user_message(
            &context.main_thread.window_target,
            message,
            UserMessageContext {
                window_id_map: context.window_id_map.clone(),
                windows: context.main_thread.windows.clone(),
            },
        );
        Ok(())
    } else {
        context
            .proxy
            .send_event(message)
            .map_err(|_| Error::FailedToSendMessage)
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct Context<T: UserEvent> {
    pub window_id_map: WindowIdStore,
    pub(crate) main_thread_id: ThreadId,
    pub proxy: EventLoopProxy<Message<T>>,
    pub(crate) main_thread: DispatcherMainThreadContext<T>,
    /* plugins: Arc<Mutex<Vec<Box<dyn Plugin<T> + Send>>>>, */
    pub(crate) next_window_id: Arc<AtomicU32>,
    pub(crate) next_webview_id: Arc<AtomicU32>,
    pub(crate) next_window_event_id: Arc<AtomicU32>,
    pub(crate) next_webview_event_id: Arc<AtomicU32>,
    pub(crate) webview_runtime_installed: bool,
}

impl<T: UserEvent> fmt::Debug for Context<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("main_thread_id", &self.main_thread_id)
            .field("proxy", &self.proxy)
            .field("main_thread", &self.main_thread)
            .finish()
    }
}

impl<T: UserEvent> Context<T> {
    #[allow(dead_code)]
    pub fn run_threaded<R, F>(&self, f: F) -> R
    where
        F: FnOnce(Option<&DispatcherMainThreadContext<T>>) -> R,
    {
        f(if current_thread().id() == self.main_thread_id {
            Some(&self.main_thread)
        } else {
            None
        })
    }

    pub(crate) fn next_window_id(&self) -> WindowId {
        self.next_window_id.fetch_add(1, Ordering::Relaxed).into()
    }

    pub(crate) fn next_webview_id(&self) -> WebViewId {
        self.next_webview_id.fetch_add(1, Ordering::Relaxed).into()
    }
}

impl<T: UserEvent> Context<T> {
    pub fn create_window(&self, builder: WindowBuilder) -> Result<Window<T>> {
        let mut webview_label_id = HashMap::new();

        let window_id = self.next_window_id();
        let webview_id = self.next_webview_id();

        let label = builder.label.as_deref().unwrap_or("main").to_string();

        if webview_label_id.contains_key(&label) {
            return Err(Error::LabelAlreadyExists);
        }

        webview_label_id.insert(label, webview_id);

        let closure_context = self.clone();
        send_user_message(
            self,
            Message::CreateWindow(
                window_id,
                Box::new(move |event_loop| create_window(window_id, webview_id, event_loop, &closure_context, builder)),
            ),
        )?;

        Ok(Window::new(window_id, self.clone(), webview_label_id))
    }
}

/*

pub(crate) fn create_window<T>(
    window_id: WindowId,
    webview_id: WebViewId,
    event_loop: &EventLoopWindowTarget<Message<T>>,
    context: &Context<T>,
    builder: WindowBuilder


fn create_window<F: Fn(RawWindow) + Send + 'static>(
    &self,
    pending: PendingWindow<T, Wry<T>>,
    after_window_creation: Option<F>,
  ) -> Result<DetachedWindow<T, Wry<T>>> {
    let label = pending.label.clone();
    let context = self.clone();
    let window_id = self.next_window_id();
    let (webview_id, use_https_scheme) = pending
      .webview
      .as_ref()
      .map(|w| {
        (
          Some(context.next_webview_id()),
          w.webview_attributes.use_https_scheme,
        )
      })
      .unwrap_or((None, false));

    send_user_message(
      self,
      Message::CreateWindow(
        window_id,
        Box::new(move |event_loop| {
          create_window(
            window_id,
            webview_id.unwrap_or_default(),
            event_loop,
            &context,
            pending,
            after_window_creation,
          )
        }),
      ),
    )?;

    let dispatcher = WryWindowDispatcher {
      window_id,
      context: self.clone(),
    };

    let detached_webview = webview_id.map(|id| {
      let webview = DetachedWebview {
        label: label.clone(),
        dispatcher: WryWebviewDispatcher {
          window_id: Arc::new(Mutex::new(window_id)),
          webview_id: id,
          context: self.clone(),
        },
      };
      DetachedWindowWebview {
        webview,
        use_https_scheme,
      }
    });

    Ok(DetachedWindow {
      id: window_id,
      label,
      dispatcher,
      webview: detached_webview,
    })
  }

  fn create_webview(
    &self,
    window_id: WindowId,
    pending: PendingWebview<T, Wry<T>>,
  ) -> Result<DetachedWebview<T, Wry<T>>> {
    let label = pending.label.clone();
    let context = self.clone();

    let webview_id = self.next_webview_id();

    let window_id_wrapper = Arc::new(Mutex::new(window_id));
    let window_id_wrapper_ = window_id_wrapper.clone();

    send_user_message(
      self,
      Message::CreateWebview(
        window_id,
        Box::new(move |window, options| {
          create_webview(
            WebviewKind::WindowChild,
            window,
            window_id_wrapper_,
            webview_id,
            &context,
            pending,
            options.focused_webview,
          )
        }),
      ),
    )?;

    let dispatcher = WryWebviewDispatcher {
      window_id: window_id_wrapper,
      webview_id,
      context: self.clone(),
    };

    Ok(DetachedWebview { label, dispatcher })
  }
}



*/
