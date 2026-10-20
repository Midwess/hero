use thiserror::Error;

/// Result type used by API gateway operations.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
/// Failures while communicating with or interpreting an API gateway.
pub enum Error {
    /// Kong rejected or failed to reconcile a service, route, or plugin.
    #[error("failed to register API gateway resource: {0}")]
    Registration(String),
    /// A gateway operation completed with an unsuccessful response.
    #[error("API gateway operation failed: {0}")]
    Operation(String),
    /// The Admin API request could not be sent or received.
    #[error("API gateway request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// A successful HTTP response did not contain the expected JSON shape.
    #[error("invalid API gateway response: {0}")]
    InvalidResponse(#[from] serde_json::Error),
}
