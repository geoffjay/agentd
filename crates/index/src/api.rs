//! REST API handlers for the agentd-index service.
//!
//! # Endpoints
//!
//! | Method | Path      | Description          |
//! |--------|-----------|----------------------|
//! | `GET`  | `/health` | Service health check |

use agentd_common::types::HealthResponse;
use axum::{response::IntoResponse, routing::get, Json, Router};

/// Creates the Axum router for the index service.
pub fn create_router() -> Router {
    Router::new().route("/health", get(health_handler))
}

/// `GET /health` — returns service health status.
async fn health_handler() -> impl IntoResponse {
    Json(HealthResponse::ok("agentd-index", env!("CARGO_PKG_VERSION")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use hyper::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_200() {
        let app = create_router();
        let request = Request::builder().method("GET").uri("/health").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_returns_ok_status() {
        let app = create_router();
        let request = Request::builder().method("GET").uri("/health").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "agentd-index");
    }
}
