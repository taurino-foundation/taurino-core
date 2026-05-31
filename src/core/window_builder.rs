use crate::{eventloop::EventLoopHandle, utils::UserEvent, window::Window};

#[derive(Debug, Clone, Default)]
pub struct WindowBuilder {
    pub label: Option<String>,
}

impl WindowBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    #[allow(dead_code)]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn build<T: UserEvent>(self, handle: &EventLoopHandle<T>) -> crate::app_error::Result<Window<T>> {
        handle.create_window(self)
    }
}
