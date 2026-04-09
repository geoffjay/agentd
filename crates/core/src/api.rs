//! HTTP router for the core service.
//!
//! Endpoints:
//! - `GET  /health`          — liveness probe
//! - `POST /auth/register`   — create account + default personal org
//! - `POST /auth/login`      — authenticate, return bearer token
//! - `POST /auth/logout`     — invalidate bearer token
//! - `GET  /auth/me`         — return current user + active organization

pub mod auth;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

use crate::storage::Storage;

/// Shared application state threaded through all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub storage: Storage,
}

/// Build the application router.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .nest("/auth", auth::router())
        .with_state(state)
}

async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "core" }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentd_common::storage::create_test_connection;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn test_app() -> Router {
        let (conn, _tmp) = create_test_connection().await;
        let storage = Storage::new(conn).await.unwrap();
        create_router(AppState { storage })
    }

    #[tokio::test]
    async fn health_returns_200() {
        let app = test_app().await;
        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_returns_json_body() {
        let app = test_app().await;
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
