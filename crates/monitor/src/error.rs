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
    /// Metrics collection failed
    #[error("Metrics collection failed: {0}")]
    CollectionFailed(String),

    /// No metrics available yet (collection has not run)
    #[error("No metrics available — collection has not run yet")]
    NoMetricsAvailable,

    /// Unknown named query — message lists the catalog.
    #[error("Unknown query `{0}`")]
    UnknownQuery(String),

    /// Invalid query parameter (window, step, ...).
    #[error("Invalid query parameter: {0}")]
    BadQueryParam(String),

    /// Prometheus itself is unreachable or rejected the query — distinct
    /// from a monitor failure so callers can tell which layer is down.
    #[error("Prometheus unavailable: {0}")]
    PrometheusUnavailable(String),

    /// Internal service error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::CollectionFailed(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            ApiError::NoMetricsAvailable => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ApiError::UnknownQuery(_) => {
                let names: Vec<&str> =
                    crate::queries::QUERY_CATALOG.iter().map(|q| q.name).collect();
                let message = format!("{self}. Known queries: {}", names.join(", "));
                return (StatusCode::NOT_FOUND, Json(json!({ "error": message }))).into_response();
            }
            ApiError::BadQueryParam(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::PrometheusUnavailable(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_failed_message() {
        let err = ApiError::CollectionFailed("sysinfo unavailable".to_string());
        assert!(err.to_string().contains("sysinfo unavailable"));
    }

    #[test]
    fn test_no_metrics_message() {
        let err = ApiError::NoMetricsAvailable;
        assert!(err.to_string().contains("No metrics"));
    }
}
