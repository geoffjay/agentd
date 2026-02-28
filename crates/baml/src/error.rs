//! Error types for the BAML client.

use thiserror::Error;

/// Errors that can occur when interacting with the BAML server
#[derive(Error, Debug)]
pub enum BamlError {
    /// The BAML server is not reachable
    #[error("BAML server unreachable at {url}: {source}")]
    ServerUnreachable { url: String, source: reqwest::Error },

    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Server returned an error response
    #[error("BAML server error (HTTP {status}): {message}")]
    ServerError { status: u16, message: String },

    /// Function not found on server
    #[error("BAML function '{function_name}' not found on server")]
    FunctionNotFound { function_name: String },

    /// Invalid response from server
    #[error("Invalid response from server: {0}")]
    InvalidResponse(String),

    /// Timeout waiting for response
    #[error("Request timed out after {timeout_secs} seconds")]
    Timeout { timeout_secs: u64 },

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result type for BAML operations
pub type Result<T> = std::result::Result<T, BamlError>;
