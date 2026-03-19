//! Shared API error types with axum `IntoResponse` implementations.
//!
//! Provides a common `ApiError` enum that all agentd services can use
//! for consistent HTTP error responses. Services with domain-specific
//! error variants can extend this via `From` impls or by wrapping.
//!
//! # HTTP Status Mapping
//!
//! | Variant | HTTP Status |
//! |---------|-------------|
//! | `NotFound` | 404 Not Found |
//! | `Unauthorized` | 401 Unauthorized |
//! | `Forbidden` | 403 Forbidden |
//! | `InvalidInput` | 400 Bad Request |
//! | `Conflict` | 409 Conflict |
//! | `ServiceUnavailable` | 503 Service Unavailable |
//! | `Internal` | 500 Internal Server Error |
//!
//! # Examples
//!
//! ```rust,ignore
//! use agentd_common::error::ApiError;
//!
//! async fn get_item(id: Uuid) -> Result<Json<Item>, ApiError> {
//!     let item = find_item(id).ok_or(ApiError::NotFound)?;
//!     Ok(Json(item))
//! }
//! ```

use axum::{http::StatusCode, response::IntoResponse, Json};

/// Shared API error type for agentd services.
///
/// Provides common HTTP error variants with consistent `IntoResponse`
/// behavior. All variants produce a JSON body: `{"error": "<message>"}`.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Resource not found (HTTP 404).
    #[error("not found")]
    NotFound,

    /// Authentication or signature verification failed (HTTP 401).
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Access denied — caller lacks permission (HTTP 403).
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// Invalid input or request (HTTP 400).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Resource conflict or invalid state transition (HTTP 409).
    #[error("conflict: {0}")]
    Conflict(String),

    /// Service temporarily unavailable (HTTP 503).
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Internal server error (HTTP 500).
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            ApiError::InvalidInput(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            ApiError::ServiceUnavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ApiError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_display() {
        let err = ApiError::NotFound;
        assert_eq!(err.to_string(), "not found");
    }

    #[test]
    fn test_invalid_input_display() {
        let err = ApiError::InvalidInput("bad field".to_string());
        assert_eq!(err.to_string(), "invalid input: bad field");
    }

    #[test]
    fn test_conflict_display() {
        let err = ApiError::Conflict("agent not running".to_string());
        assert_eq!(err.to_string(), "conflict: agent not running");
    }

    #[test]
    fn test_internal_from_anyhow() {
        let err: ApiError = anyhow::anyhow!("db broke").into();
        assert!(matches!(err, ApiError::Internal(_)));
        assert!(err.to_string().contains("db broke"));
    }
}
