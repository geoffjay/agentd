//! Error types for the monitor service.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Errors that can occur in monitor service API handlers.
#[derive(Debug, Error)]
pub enum ApiError {
    /// No metrics are available yet (collection has not run)
    #[error("No metrics available yet")]
    NoMetricsAvailable,

    /// The requested resource was not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// An internal error occurred
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::NoMetricsAvailable => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_metrics_available_message() {
        let err = ApiError::NoMetricsAvailable;
        assert!(err.to_string().contains("No metrics"));
    }

    #[test]
    fn test_not_found_message() {
        let err = ApiError::NotFound("alert-123".to_string());
        assert!(err.to_string().contains("alert-123"));
    }

    #[test]
    fn test_internal_error_message() {
        let err = ApiError::Internal("unexpected panic".to_string());
        assert!(err.to_string().contains("unexpected panic"));
    }
}
