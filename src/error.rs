use http::{
    header::{InvalidHeaderName, InvalidHeaderValue},
    method::InvalidMethod,
    status::InvalidStatusCode,
};

use crate::utils::resource::ResourceId;

/// The result type of `tauri-utils`.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// IO error.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// Failed to create webview.
    #[error("failed to create webview: {0}")]
    CreateWebview(Box<dyn std::error::Error + Send + Sync>),
    // TODO: Make it take an error like `CreateWebview` in v3
    /// Failed to create window.
    #[error("failed to create window")]
    CreateWindow,
    /// The given window label is invalid.
    #[error("Window labels must only include alphanumeric characters, `-`, `/`, `:` and `_`.")]
    InvalidWindowLabel,
    /// Failed to send message to webview.
    #[error("failed to send message to the webview")]
    FailedToSendMessage,
    /// Failed to receive message from webview.
    #[error("failed to receive message from webview")]
    FailedToReceiveMessage,
    /// Failed to serialize/deserialize.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Failed to load window icon.
    #[error("invalid icon: {0}")]
    InvalidIcon(Box<dyn std::error::Error + Send + Sync>),
    /// Failed to get monitor on window operation.
    #[error("failed to get monitor")]
    FailedToGetMonitor,
    /// Failed to get cursor position.
    #[error("failed to get cursor position")]
    FailedToGetCursorPosition,
    #[error("Invalid header name: {0}")]
    InvalidHeaderName(#[from] InvalidHeaderName),
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("Invalid status code: {0}")]
    InvalidStatusCode(#[from] InvalidStatusCode),
    #[error("Invalid method: {0}")]
    InvalidMethod(#[from] InvalidMethod),
    #[error("Infallible error, something went really wrong: {0}")]
    Infallible(#[from] std::convert::Infallible),
    #[error("the event loop has been closed")]
    EventLoopClosed,
    #[error("Invalid proxy url")]
    InvalidProxyUrl,
    #[error("window not found")]
    WindowNotFound,
    #[error("webview `{0}` not found")]
    WebviewNotFound(String),
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[error("failed to remove data store")]
    FailedToRemoveDataStore,
    #[error("Could not find the webview runtime, make sure it is installed")]
    WebviewRuntimeNotInstalled,
    /// The window's native handle has not been initialized yet.
    #[error("window `{0}` has not been initialized")]
    WindowNotInitialized(String),
    #[error("window `{0}` label must not be empty")]
    EmptyInitializedWindow(String),
    #[error("webview `{0}` label must not be empty")]
    EmptyInitializedWebView(String),
    /// Menu operation failed.
    #[error("menu error: {0}")]
    Menu(#[from] muda::Error),

    /// Invalid menu icon.
    #[error("bad menu icon: {0}")]
    BadMenuIcon(#[from] muda::BadIcon),
    #[error("failed to process image: {0}")]
    Image(#[from] image::error::ImageError),
    // JNI error.
    #[cfg(target_os = "android")]
    #[error("jni error: {0}")]
    Jni(#[from] jni::errors::Error),

    /// Tray icon error.
    #[error("tray icon error: {0}")]
    Tray(#[from] tray_icon::Error),
    /// Bad tray icon error.
    #[error(transparent)]
    BadTrayIcon(#[from] tray_icon::BadIcon),
    /// The resource id is invalid.
    #[error("The resource id {0} is invalid.")]
    BadResourceId(ResourceId),
    /// The anyhow crate error.
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    /// [`crate::image::Image::from_icon_resource`] failed
    #[cfg(windows)]
    #[error("Can not load Image from icon resources: {0}")]
    ImageFromResource(windows::core::Error),
    /// The Window's raw handle is invalid for the platform.
    #[error("Unexpected `raw_window_handle` for the current platform")]
    InvalidWindowHandle,
}
