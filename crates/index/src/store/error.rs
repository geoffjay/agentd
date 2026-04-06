//! Error types for the index vector store.

use thiserror::Error;

/// Errors that can occur in store operations.
#[derive(Error, Debug)]
pub enum StoreError {
    /// The backend could not be opened or initialised.
    #[error("store initialization failed: {0}")]
    InitializationFailed(String),

    /// A connection attempt to the backend failed.
    #[error("store connection failed: {0}")]
    ConnectionFailed(String),

    /// A query or write operation failed.
    #[error("store query failed: {0}")]
    QueryFailed(String),

    /// A record was expected but was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Data read from the store could not be parsed.
    #[error("invalid data: {0}")]
    InvalidData(String),
}

/// Convenience `Result` alias for store operations.
pub type StoreResult<T> = Result<T, StoreError>;
