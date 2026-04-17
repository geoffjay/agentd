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

impl ApiError {
    /// Map each error variant to its corresponding HTTP status code.
    ///
    /// Useful for middleware, tests, and any caller that needs to inspect
    /// the status code without constructing a full `axum::response::Response`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agentd_common::error::ApiError;
    /// use axum::http::StatusCode;
    ///
    /// assert_eq!(ApiError::NotFound.status_code(), StatusCode::NOT_FOUND);
    /// assert_eq!(
    ///     ApiError::InvalidInput("bad".into()).status_code(),
    ///     StatusCode::BAD_REQUEST,
    /// );
    /// ```
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let message = self.to_string();
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_code_not_found() {
        assert_eq!(ApiError::NotFound.status_code(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_status_code_unauthorized() {
        assert_eq!(ApiError::Unauthorized("sig".into()).status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_status_code_forbidden() {
        assert_eq!(ApiError::Forbidden("no perms".into()).status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_status_code_invalid_input() {
        assert_eq!(ApiError::InvalidInput("bad".into()).status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_status_code_conflict() {
        assert_eq!(ApiError::Conflict("dup".into()).status_code(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_status_code_service_unavailable() {
        assert_eq!(
            ApiError::ServiceUnavailable("down".into()).status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn test_status_code_internal() {
        let err: ApiError = anyhow::anyhow!("boom").into();
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

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
