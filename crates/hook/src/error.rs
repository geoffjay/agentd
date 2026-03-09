//! Error types for the hook service.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Errors that can occur in hook service API handlers.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The incoming event payload was malformed or missing required fields
    #[error("Invalid event: {0}")]
    InvalidEvent(String),

    /// The requested resource was not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// The specified shell is not supported
    #[error("Invalid shell: {0}")]
    InvalidShell(String),

    /// An internal error occurred while processing the event
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::InvalidEvent(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::InvalidShell(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_event_message() {
        let err = ApiError::InvalidEvent("missing command field".to_string());
        assert!(err.to_string().contains("missing command field"));
    }

    #[test]
    fn test_internal_error_message() {
        let err = ApiError::Internal("database unavailable".to_string());
        assert!(err.to_string().contains("database unavailable"));
    }

    #[test]
    fn test_not_found_message() {
        let err = ApiError::NotFound("Event 123 not found".to_string());
        assert!(err.to_string().contains("Event 123 not found"));
    }

    #[test]
    fn test_invalid_shell_message() {
        let err = ApiError::InvalidShell("powershell not supported".to_string());
        assert!(err.to_string().contains("powershell not supported"));
    }
}
