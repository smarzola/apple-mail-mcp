use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("invalid request: {0}")]
    Validation(String),
    #[error("Apple Mail automation is unavailable: {0}")]
    AutomationUnavailable(String),
    #[error("Apple Mail rejected the operation: {0}")]
    AutomationFailed(String),
    #[error("Apple Mail did not respond within {0:?}")]
    AutomationTimeout(Duration),
    #[error("Apple Mail returned more data than the configured safety limit")]
    OutputTooLarge,
    #[error("Apple Mail returned an invalid response: {0}")]
    InvalidResponse(String),
    #[error("failed to start Apple Mail automation: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to encode or decode automation data: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, MailError>;
