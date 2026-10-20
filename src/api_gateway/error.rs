use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to register API gateway resource: {0}")]
    Registration(String),
    #[error("API gateway operation failed: {0}")]
    Operation(String),
    #[error("API gateway request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("invalid API gateway response: {0}")]
    InvalidResponse(#[from] serde_json::Error),
}
