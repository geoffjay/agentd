//! Error types for the agentd-index service.

use thiserror::Error;

/// Errors that can occur in index service operations.
#[derive(Error, Debug)]
pub enum IndexError {
    /// The requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// An invalid request was made.
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<IndexError> for agentd_common::error::ApiError {
    fn from(err: IndexError) -> Self {
        match err {
            IndexError::NotFound(_) => agentd_common::error::ApiError::NotFound,
            IndexError::InvalidRequest(msg) => agentd_common::error::ApiError::InvalidInput(msg),
            IndexError::Internal(msg) => {
                agentd_common::error::ApiError::Internal(anyhow::anyhow!("{}", msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_display() {
        let err = IndexError::NotFound("repo-1".to_string());
        assert_eq!(err.to_string(), "not found: repo-1");
    }

    #[test]
    fn test_invalid_request_display() {
        let err = IndexError::InvalidRequest("missing field".to_string());
        assert_eq!(err.to_string(), "invalid request: missing field");
    }

    #[test]
    fn test_internal_display() {
        let err = IndexError::Internal("db unavailable".to_string());
        assert_eq!(err.to_string(), "internal error: db unavailable");
    }

    #[test]
    fn test_not_found_converts_to_api_error() {
        let err = IndexError::NotFound("x".to_string());
        let api: agentd_common::error::ApiError = err.into();
        assert!(matches!(api, agentd_common::error::ApiError::NotFound));
    }

    #[test]
    fn test_invalid_request_converts_to_invalid_input() {
        let err = IndexError::InvalidRequest("bad".to_string());
        let api: agentd_common::error::ApiError = err.into();
        assert!(matches!(api, agentd_common::error::ApiError::InvalidInput(_)));
    }

    #[test]
    fn test_internal_converts_to_api_internal() {
        let err = IndexError::Internal("oops".to_string());
        let api: agentd_common::error::ApiError = err.into();
        assert!(matches!(api, agentd_common::error::ApiError::Internal(_)));
    }
}
