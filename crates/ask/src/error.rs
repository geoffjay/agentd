//! Error types for the ask service.
//!
//! This module defines all error types used throughout the service, organized
//! into three categories: tmux errors, notification errors, and API errors.
//! All errors implement Display and Error traits via `thiserror`.
//!
//! # Error Categories
//!
//! - [`TmuxError`] - Errors from tmux operations (not installed, command failed, etc.)
//! - [`NotificationError`] - Errors from notification service communication
//! - [`ApiError`] - High-level API errors that map to HTTP responses
//!
//! # HTTP Status Mapping
//!
//! [`ApiError`] implements [`IntoResponse`] to automatically convert errors to
//! appropriate HTTP responses:
//!
//! - `QuestionNotFound` -> 404 Not Found
//! - `QuestionNotActionable` -> 410 Gone
//! - `InvalidRequest` -> 400 Bad Request
//! - `TmuxError` -> 500 Internal Server Error
//! - `NotificationError` -> 502 Bad Gateway
//! - `InternalError` -> 500 Internal Server Error
//!
//! # Examples
//!
//! ## Handling tmux errors
//!
//! ```
//! use ask::tmux_check::check_tmux_sessions;
//! use ask::error::TmuxError;
//!
//! match check_tmux_sessions() {
//!     Ok(result) => println!("Check succeeded: {:?}", result),
//!     Err(TmuxError::NotInstalled) => eprintln!("tmux is not installed"),
//!     Err(TmuxError::CommandFailed(msg)) => eprintln!("Command failed: {}", msg),
//!     Err(e) => eprintln!("Other error: {}", e),
//! }
//! ```
//!
//! ## API error conversion
//!
//! ```no_run
//! use ask::error::ApiError;
//! use axum::response::IntoResponse;
//!
//! fn example_handler() -> Result<String, ApiError> {
//!     Err(ApiError::QuestionNotFound("Question not found".to_string()))
//! }
//!
//! // Automatically converts to HTTP 404 response
//! let response = example_handler().unwrap_err().into_response();
//! ```

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;

/// Errors related to tmux operations.
///
/// These errors occur when interacting with tmux for session checking.
#[derive(Debug, Error)]
pub enum TmuxError {
    /// Tmux is not installed or not found in PATH.
    ///
    /// This error occurs when the `which tmux` command fails, indicating
    /// tmux is not available on the system.
    #[error("tmux is not installed or not found in PATH")]
    NotInstalled,

    /// Tmux command execution failed.
    ///
    /// Contains the error message or stderr from the failed command.
    #[error("tmux command failed: {0}")]
    CommandFailed(String),

    /// Failed to parse tmux command output.
    ///
    /// Occurs when tmux output is not valid UTF-8 or doesn't match expected format.
    #[error("failed to parse tmux output: {0}")]
    ParseError(String),

    /// Tmux server is not running.
    ///
    /// Indicates tmux is installed but no server process is active.
    #[error("tmux server is not running")]
    #[allow(dead_code)]
    ServerNotRunning,
}

/// Errors related to notification service communication.
///
/// These errors occur when making HTTP requests to the notification service.
#[derive(Debug, Error)]
pub enum NotificationError {
    /// Failed to send notification.
    ///
    /// Generic error for notification sending failures.
    #[error("failed to send notification: {0}")]
    #[allow(dead_code)]
    SendFailed(String),

    /// Notification service is unavailable or returned an error.
    ///
    /// Contains the HTTP status code and error message from the service.
    #[error("notification service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Invalid response from notification service.
    ///
    /// Occurs when the response body cannot be parsed as expected JSON.
    #[error("invalid notification response: {0}")]
    InvalidResponse(String),

    /// HTTP request to notification service failed.
    ///
    /// Wraps `reqwest::Error` for network-level failures.
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
}

/// High-level API errors with HTTP status code mapping.
///
/// These errors represent application-level failures and automatically
/// convert to appropriate HTTP responses via [`IntoResponse`].
#[derive(Debug, Error)]
pub enum ApiError {
    /// Question with the given ID was not found.
    ///
    /// Maps to HTTP 404 Not Found.
    #[error("question not found: {0}")]
    QuestionNotFound(String),

    /// Question is no longer actionable (already answered or expired).
    ///
    /// Maps to HTTP 410 Gone.
    #[error("question is no longer actionable: {0}")]
    QuestionNotActionable(String),

    /// Request was invalid or malformed.
    ///
    /// Maps to HTTP 400 Bad Request.
    #[error("invalid request: {0}")]
    #[allow(dead_code)]
    InvalidRequest(String),

    /// Error from tmux operations.
    ///
    /// Wraps [`TmuxError`] and maps to HTTP 500 Internal Server Error.
    #[error("tmux error: {0}")]
    TmuxError(#[from] TmuxError),

    /// Error from notification service communication.
    ///
    /// Wraps [`NotificationError`] and maps to HTTP 502 Bad Gateway.
    #[error("notification error: {0}")]
    NotificationError(#[from] NotificationError),

    /// Internal server error.
    ///
    /// Generic error for unexpected failures. Maps to HTTP 500.
    #[error("internal error: {0}")]
    InternalError(String),
}

impl From<anyhow::Error> for ApiError {
    /// Converts `anyhow::Error` to `ApiError::InternalError`.
    ///
    /// This allows using `?` operator with anyhow errors in API handlers.
    fn from(err: anyhow::Error) -> Self {
        ApiError::InternalError(err.to_string())
    }
}

impl IntoResponse for ApiError {
    /// Converts API errors to HTTP responses with appropriate status codes.
    ///
    /// Each error variant maps to a specific HTTP status code, and the error
    /// message is returned as JSON with an `"error"` field.
    ///
    /// # Status Code Mapping
    ///
    /// - `QuestionNotFound` -> 404 Not Found
    /// - `QuestionNotActionable` -> 410 Gone
    /// - `InvalidRequest` -> 400 Bad Request
    /// - `TmuxError` -> 500 Internal Server Error
    /// - `NotificationError` -> 502 Bad Gateway
    /// - `InternalError` -> 500 Internal Server Error
    ///
    /// # Response Format
    ///
    /// ```json
    /// {
    ///   "error": "error message here"
    /// }
    /// ```
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::QuestionNotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::QuestionNotActionable(msg) => (StatusCode::GONE, msg),
            ApiError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::TmuxError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ApiError::NotificationError(e) => (StatusCode::BAD_GATEWAY, e.to_string()),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmux_error_display() {
        let err = TmuxError::NotInstalled;
        assert_eq!(err.to_string(), "tmux is not installed or not found in PATH");

        let err = TmuxError::CommandFailed("test error".to_string());
        assert_eq!(err.to_string(), "tmux command failed: test error");

        let err = TmuxError::ParseError("parse failed".to_string());
        assert_eq!(err.to_string(), "failed to parse tmux output: parse failed");

        let err = TmuxError::ServerNotRunning;
        assert_eq!(err.to_string(), "tmux server is not running");
    }

    #[test]
    fn test_notification_error_display() {
        let err = NotificationError::SendFailed("network error".to_string());
        assert_eq!(err.to_string(), "failed to send notification: network error");

        let err = NotificationError::ServiceUnavailable("503".to_string());
        assert_eq!(err.to_string(), "notification service unavailable: 503");

        let err = NotificationError::InvalidResponse("bad json".to_string());
        assert_eq!(err.to_string(), "invalid notification response: bad json");
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError::QuestionNotFound("id-123".to_string());
        assert_eq!(err.to_string(), "question not found: id-123");

        let err = ApiError::QuestionNotActionable("already answered".to_string());
        assert_eq!(err.to_string(), "question is no longer actionable: already answered");

        let err = ApiError::InvalidRequest("missing field".to_string());
        assert_eq!(err.to_string(), "invalid request: missing field");

        let err = ApiError::InternalError("database error".to_string());
        assert_eq!(err.to_string(), "internal error: database error");
    }

    #[test]
    fn test_api_error_from_tmux_error() {
        let tmux_err = TmuxError::NotInstalled;
        let api_err: ApiError = tmux_err.into();
        assert!(matches!(api_err, ApiError::TmuxError(_)));
        assert_eq!(api_err.to_string(), "tmux error: tmux is not installed or not found in PATH");
    }

    #[test]
    fn test_api_error_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let api_err: ApiError = anyhow_err.into();
        assert!(matches!(api_err, ApiError::InternalError(_)));
        assert!(api_err.to_string().contains("something went wrong"));
    }

    #[test]
    fn test_api_error_into_response_status_codes() {
        let err = ApiError::QuestionNotFound("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let err = ApiError::QuestionNotActionable("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::GONE);

        let err = ApiError::InvalidRequest("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let err = ApiError::TmuxError(TmuxError::NotInstalled);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let err =
            ApiError::NotificationError(NotificationError::ServiceUnavailable("test".to_string()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

        let err = ApiError::InternalError("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_api_error_response_body() {
        use http_body_util::BodyExt;

        let err = ApiError::QuestionNotFound("test-id".to_string());
        let response = err.into_response();

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        let body_text = String::from_utf8(bytes.to_vec()).unwrap();

        // The response is JSON with an "error" field
        assert!(body_text.contains("\"error\""));
        assert!(body_text.contains("test-id"));
    }
}
