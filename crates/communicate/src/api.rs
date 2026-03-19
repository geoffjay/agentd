//! REST API router for the communicate service.

use axum::{routing::get, Json, Router};
use std::sync::Arc;

use crate::storage::CommunicateStorage;

/// Shared application state injected into all route handlers.
#[derive(Clone)]
pub struct ApiState {
    // Storage will be accessed by route handlers added in subsequent issues.
    #[allow(dead_code)]
    pub storage: Arc<CommunicateStorage>,
}

/// Build the Axum router with all communicate API routes.
pub fn create_router(state: ApiState) -> Router {
    Router::new().route("/health", get(health)).with_state(state)
}

/// `GET /health` — liveness check.
async fn health() -> Json<agentd_common::types::HealthResponse> {
    Json(agentd_common::types::HealthResponse::ok("agentd-communicate", env!("CARGO_PKG_VERSION")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tempfile::TempDir;
    use tower::ServiceExt;

    async fn build_test_app() -> (Router, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Arc::new(CommunicateStorage::with_path(&db_path).await.unwrap());
        let state = ApiState { storage };
        (create_router(state), temp_dir)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let (app, _temp) = build_test_app().await;

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "agentd-communicate");
    }
}
