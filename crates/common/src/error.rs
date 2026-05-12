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
    use axum::{body::to_bytes, response::IntoResponse};

    // --- Display tests ---

    #[test]
    fn test_not_found_display() {
        let err = ApiError::NotFound;
        assert_eq!(err.to_string(), "not found");
    }

    #[test]
    fn test_unauthorized_display() {
        let err = ApiError::Unauthorized("invalid signature".to_string());
        assert_eq!(err.to_string(), "unauthorized: invalid signature");
    }

    #[test]
    fn test_forbidden_display() {
        let err = ApiError::Forbidden("read-only token".to_string());
        assert_eq!(err.to_string(), "forbidden: read-only token");
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
    fn test_service_unavailable_display() {
        let err = ApiError::ServiceUnavailable("db unreachable".to_string());
        assert_eq!(err.to_string(), "service unavailable: db unreachable");
    }

    #[test]
    fn test_internal_from_anyhow() {
        let err: ApiError = anyhow::anyhow!("db broke").into();
        assert!(matches!(err, ApiError::Internal(_)));
        assert!(err.to_string().contains("db broke"));
    }

    // --- IntoResponse: HTTP status codes ---

    #[tokio::test]
    async fn test_not_found_returns_404() {
        let response = ApiError::NotFound.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_unauthorized_returns_401() {
        let response = ApiError::Unauthorized("bad token".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_forbidden_returns_403() {
        let response = ApiError::Forbidden("no permission".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_invalid_input_returns_400() {
        let response = ApiError::InvalidInput("missing id".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_conflict_returns_409() {
        let response = ApiError::Conflict("already exists".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_service_unavailable_returns_503() {
        let response = ApiError::ServiceUnavailable("db down".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_internal_returns_500() {
        let response = ApiError::Internal(anyhow::anyhow!("unexpected failure")).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    // --- IntoResponse: JSON body shape ---

    #[tokio::test]
    async fn test_not_found_body_contains_error_key() {
        let response = ApiError::NotFound.into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "not found");
    }

    #[tokio::test]
    async fn test_unauthorized_body_contains_message() {
        let response = ApiError::Unauthorized("expired".to_string()).into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "unauthorized: expired");
    }

    #[tokio::test]
    async fn test_invalid_input_body_contains_message() {
        let response = ApiError::InvalidInput("name required".to_string()).into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "invalid input: name required");
    }

    #[tokio::test]
    async fn test_internal_body_exposes_cause_message() {
        let response = ApiError::Internal(anyhow::anyhow!("disk full")).into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "disk full");
    }

    #[tokio::test]
    async fn test_response_body_is_json_object_with_single_error_key() {
        // All variants must produce exactly {"error": "..."} with no extra keys.
        let response = ApiError::Conflict("dup".to_string()).into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.is_object());
        assert_eq!(json.as_object().unwrap().len(), 1);
        assert!(json["error"].is_string());
    }
}
