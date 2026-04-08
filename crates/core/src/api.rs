//! HTTP router for the core service.
//!
//! Endpoints:
//! - `GET /health` — liveness probe; returns `{"status":"ok","service":"core"}`

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

/// Build the application router.
pub fn create_router() -> Router {
    Router::new().route("/health", get(health_handler))
}

async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "core" }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_200() {
        let app = create_router();
        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_returns_json_body() {
        let app = create_router();
        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "core");
    }
}
