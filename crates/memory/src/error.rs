//! Error types for the agentd-memory service.
//!
//! This module defines the error hierarchy for memory store operations,
//! with conversions to [`agentd_common::error::ApiError`] for HTTP responses.

use thiserror::Error;

/// Errors that can occur during vector store operations.
///
/// # Examples
///
/// ```rust
/// use memory::error::StoreError;
///
/// let err = StoreError::NotFound("mem_123_abc".to_string());
/// assert!(err.to_string().contains("mem_123_abc"));
/// ```
#[derive(Error, Debug)]
pub enum StoreError {
    /// The requested memory record was not found.
    #[error("memory not found: {0}")]
    NotFound(String),

    /// Failed to connect to the vector store backend.
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    /// A query or write operation against the store failed.
    #[error("query failed: {0}")]
    QueryFailed(String),

    /// The data is malformed or cannot be deserialized.
    #[error("invalid data: {0}")]
    InvalidData(String),

    /// The store could not be initialized (missing collection, bad schema, etc.).
    #[error("initialization failed: {0}")]
    InitializationFailed(String),

    /// The requesting actor does not have permission to access this memory.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
}

/// Convenience alias for store operation results.
pub type StoreResult<T> = std::result::Result<T, StoreError>;

impl From<StoreError> for agentd_common::error::ApiError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::NotFound(_) => agentd_common::error::ApiError::NotFound,
            StoreError::PermissionDenied(msg) => agentd_common::error::ApiError::InvalidInput(msg),
            StoreError::InvalidData(msg) => agentd_common::error::ApiError::InvalidInput(msg),
            e => agentd_common::error::ApiError::Internal(anyhow::anyhow!("{}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_display() {
        let err = StoreError::NotFound("mem_123_abc".to_string());
        assert_eq!(err.to_string(), "memory not found: mem_123_abc");
    }

    #[test]
    fn test_connection_failed_display() {
        let err = StoreError::ConnectionFailed("timeout after 30s".to_string());
        assert_eq!(err.to_string(), "connection failed: timeout after 30s");
    }

    #[test]
    fn test_query_failed_display() {
        let err = StoreError::QueryFailed("invalid vector dimension".to_string());
        assert_eq!(err.to_string(), "query failed: invalid vector dimension");
    }

    #[test]
    fn test_initialization_failed_display() {
        let err = StoreError::InitializationFailed("schema mismatch".to_string());
        assert_eq!(err.to_string(), "initialization failed: schema mismatch");
    }

    #[test]
    fn test_permission_denied_display() {
        let err = StoreError::PermissionDenied("actor=user2".to_string());
        assert_eq!(err.to_string(), "permission denied: actor=user2");
    }

    #[test]
    fn test_not_found_converts_to_api_error_not_found() {
        let store_err = StoreError::NotFound("mem_1".to_string());
        let api_err: agentd_common::error::ApiError = store_err.into();
        assert!(matches!(api_err, agentd_common::error::ApiError::NotFound));
    }

    #[test]
    fn test_permission_denied_converts_to_invalid_input() {
        let store_err = StoreError::PermissionDenied("denied".to_string());
        let api_err: agentd_common::error::ApiError = store_err.into();
        assert!(matches!(api_err, agentd_common::error::ApiError::InvalidInput(_)));
    }

    #[test]
    fn test_invalid_data_converts_to_invalid_input() {
        let store_err = StoreError::InvalidData("bad json".to_string());
        let api_err: agentd_common::error::ApiError = store_err.into();
        assert!(matches!(api_err, agentd_common::error::ApiError::InvalidInput(_)));
    }

    #[test]
    fn test_connection_failed_converts_to_internal() {
        let store_err = StoreError::ConnectionFailed("unreachable".to_string());
        let api_err: agentd_common::error::ApiError = store_err.into();
        assert!(matches!(api_err, agentd_common::error::ApiError::Internal(_)));
    }
}
