//! Error types for the ask service.
//!
//! # HTTP Status Mapping
//!
//! - `QuestionNotFound` -> 404 Not Found
//! - `QuestionAlreadyAnswered` -> 409 Conflict
//! - `InvalidRequest` -> 400 Bad Request
//! - `InternalError` -> 500 Internal Server Error

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;

/// High-level API errors with HTTP status code mapping.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Question with the given ID was not found. Maps to HTTP 404.
    #[error("question not found: {0}")]
    QuestionNotFound(String),

    /// Question is already answered or dismissed. Maps to HTTP 409.
    #[error("question already answered or dismissed: {0}")]
    QuestionAlreadyAnswered(String),

    /// Request was invalid or malformed. Maps to HTTP 400.
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// Internal server error. Maps to HTTP 500.
    #[error("internal error: {0}")]
    InternalError(String),
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        // Check if it's a "already answered" error from storage.
        let msg = err.to_string();
        if msg.contains("already") && msg.contains("cannot be updated") {
            ApiError::QuestionAlreadyAnswered(msg)
        } else if msg.contains("not found") {
            ApiError::QuestionNotFound(msg)
        } else {
            ApiError::InternalError(msg)
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::QuestionNotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::QuestionAlreadyAnswered(msg) => (StatusCode::CONFLICT, msg),
            ApiError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "error": error_message }));
        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_display() {
        assert_eq!(
            ApiError::QuestionNotFound("id-123".to_string()).to_string(),
            "question not found: id-123"
        );
        assert_eq!(
            ApiError::QuestionAlreadyAnswered("done".to_string()).to_string(),
            "question already answered or dismissed: done"
        );
        assert_eq!(
            ApiError::InvalidRequest("bad input".to_string()).to_string(),
            "invalid request: bad input"
        );
        assert_eq!(
            ApiError::InternalError("db error".to_string()).to_string(),
            "internal error: db error"
        );
    }

    #[test]
    fn test_api_error_status_codes() {
        let cases = [
            (ApiError::QuestionNotFound("x".into()), StatusCode::NOT_FOUND),
            (ApiError::QuestionAlreadyAnswered("x".into()), StatusCode::CONFLICT),
            (ApiError::InvalidRequest("x".into()), StatusCode::BAD_REQUEST),
            (ApiError::InternalError("x".into()), StatusCode::INTERNAL_SERVER_ERROR),
        ];
        for (err, expected_status) in cases {
            assert_eq!(err.into_response().status(), expected_status);
        }
    }

    #[test]
    fn test_from_anyhow_not_found() {
        let err: ApiError = anyhow::anyhow!("Question abc not found").into();
        assert!(matches!(err, ApiError::QuestionNotFound(_)));
    }

    #[test]
    fn test_from_anyhow_already_answered() {
        let err: ApiError =
            anyhow::anyhow!("Question abc is already Answered and cannot be updated").into();
        assert!(matches!(err, ApiError::QuestionAlreadyAnswered(_)));
    }

    #[test]
    fn test_from_anyhow_internal() {
        let err: ApiError = anyhow::anyhow!("database connection failed").into();
        assert!(matches!(err, ApiError::InternalError(_)));
    }
}
