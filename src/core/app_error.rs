#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
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
    #[error("label already exists")]
    LabelAlreadyExists,
    #[error("label does not exist")]
    LabelDoesNotExist,
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
    /*   #[error("Invalid header name: {0}")]
    InvalidHeaderName(#[from] InvalidHeaderName),
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("Invalid status code: {0}")]
    InvalidStatusCode(#[from] InvalidStatusCode),
    #[error("Invalid method: {0}")]
    InvalidMethod(#[from] InvalidMethod), */
    #[error("Infallible error, something went really wrong: {0}")]
    Infallible(#[from] std::convert::Infallible),
    #[error("the event loop has been closed")]
    EventLoopClosed,
    #[error("Invalid proxy url")]
    InvalidProxyUrl,
    #[error("window not found")]
    WindowNotFound,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[error("failed to remove data store")]
    FailedToRemoveDataStore,
    #[error("Could not find the webview runtime, make sure it is installed")]
    WebviewRuntimeNotInstalled,
}

/// Result type.
pub type Result<T> = std::result::Result<T, Error>;
