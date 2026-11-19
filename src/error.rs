use thiserror::Error;

/// Core error type for Chronos Browser.
#[derive(Error, Debug)]
pub enum ChronosError {
    #[error("Chrome binary not found: {0}")]
    ChromeBinaryNotFound(String),

    #[error("Failed to launch Chrome process: {0}")]
    ProcessLaunchFailed(String),

    #[error("Failed to discover DevTools active port: {0}")]
    DevToolsPortNotFound(String),

    #[error("CDP connection failed: {0}")]
    ConnectionFailed(String),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CDP protocol error ({code}): {message}{}", .data.as_ref().map(|d| format!(" - {}", d)).unwrap_or_default())]
    ProtocolError {
        code: i64,
        message: String,
        data: Option<String>,
    },

    #[error("Target error: {0}")]
    TargetNotFound(String),

    #[error("Navigation failed: {0}")]
    NavigationFailed(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Element not found: {0}")]
    ElementNotFound(String),

    #[error("Operation timed out: {0}")]
    Timeout(String),

    #[error("Channel error: {0}")]
    ChannelClosed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Convenience type alias for `Result<T, ChronosError>`.
pub type Result<T, E = ChronosError> = std::result::Result<T, E>;
