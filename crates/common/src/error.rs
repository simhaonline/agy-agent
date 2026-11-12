use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommonError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid encoding: {0}")]
    InvalidEncoding(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Path traversal or security violation: {0}")]
    Security(String),

    #[error("Generic error: {0}")]
    Generic(String),
}
