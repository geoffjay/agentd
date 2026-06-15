//! HTTP router for the core service.
//!
//! Endpoints:
//! - `GET  /health`                              — liveness probe
//! - `POST /auth/register`                       — create account + default personal org
//! - `POST /auth/login`                          — authenticate, return bearer token
//! - `POST /auth/logout`                         — invalidate bearer token
//! - `GET  /auth/me`                             — return current user + active organization
//! - `GET  /api/v1/users/me`                     — get current user profile
//! - `PUT  /api/v1/users/me`                     — update profile (display_name, email)
//! - `PUT  /api/v1/users/me/password`            — change password
//! - `GET  /api/v1/users/me/organizations`       — list user's organizations
//! - `PUT  /users/me/active-organization`        — switch active organization
//! - `POST /api/v1/organizations`                — create organization
//! - `GET  /api/v1/organizations/{id}`           — get organization
//! - `PUT  /api/v1/organizations/{id}`           — update organization (owners only)
//! - `DELETE /api/v1/organizations/{id}`         — delete organization (owners only)
//! - `GET  /api/v1/admin/users`                  — list all users (superuser only)
//! - `GET  /api/v1/admin/organizations`          — list all organizations (superuser only)
//! - `GET  /api/v1/admin/memberships`            — list all memberships (superuser only)
//! - `GET  /api/v1/admin/sessions`               — list all sessions (superuser only)
//! - `GET  /api/v1/organizations/{id}/members`   — list members
//! - `POST /api/v1/organizations/{id}/members`   — add member (owners only)
//! - `DELETE /api/v1/organizations/{id}/members/{uid}` — remove member (owners only)
//! - `GET  /api/v1/health`                       — aggregate downstream health check
//! - `ANY  /api/v1/{service}/*`                  — proxy to downstream service

pub mod admin;
pub mod auth;
pub mod gateway;
pub mod organizations;
pub mod users;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{proxy::ProxyConfig, storage::Storage};

/// Shared application state threaded through all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub storage: Storage,
}

/// Build the application router without a proxy (for testing or when proxy is disabled).
pub fn create_router(state: AppState) -> Router {
    create_router_with_proxy(state, ProxyConfig::from_env())
}

/// Build the application router with an explicit [`ProxyConfig`].
pub fn create_router_with_proxy(state: AppState, proxy: ProxyConfig) -> Router {
    let api_v1 = Router::new()
        .nest("/users", users::v1_router())
        .nest("/organizations", organizations::router())
        // Product-admin routes — superuser only, product-wide (not tenant-scoped)
        .nest("/admin", admin::router())
        // Gateway routes — /api/v1/health and /api/v1/{service}/* path
        .merge(gateway::router(proxy));

    Router::new()
        .route("/health", get(health_handler))
        .nest("/auth", auth::router())
        // Legacy active-org route (issue #216 spec)
        .nest("/users", users::router())
        .nest("/api/v1", api_v1)
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
