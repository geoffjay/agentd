//! REST API endpoints and routing for the hook service.
//!
//! Provides the following endpoints:
//!
//! - `GET /health`   — standard health check
//! - `POST /events`  — receive and record a hook event

use crate::{
    error::ApiError,
    types::{EventResponse, HealthResponse, HookEvent, RecordedEvent},
};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use tracing::info;
use uuid::Uuid;

/// Shared state passed to every API handler.
///
/// Currently stateless; will be extended with storage and notification client
/// when full functionality is implemented.
#[derive(Clone)]
pub struct ApiState {
    /// Service name reported in health check responses
    pub service_name: &'static str,
}

impl Default for ApiState {
    fn default() -> Self {
        Self { service_name: "agentd-hook" }
    }
}

/// Create the base router (no middleware).
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/events", post(receive_event))
        .with_state(state)
}

/// Create the router with HTTP tracing middleware.
pub fn create_router_with_tracing(state: ApiState) -> Router {
    use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};

    create_router(state).layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
            .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
    )
}

/// `GET /health` — standard health check.
async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    Json(HealthResponse::ok(state.service_name, env!("CARGO_PKG_VERSION")))
}

/// `POST /events` — receive and record a hook event.
///
/// Accepts a [`HookEvent`] JSON body, assigns it a unique ID, timestamps it,
/// and returns an [`EventResponse`] confirmation.
///
/// # Errors
///
/// Returns 400 if the event payload is missing the `command` field.
async fn receive_event(
    State(_state): State<ApiState>,
    Json(event): Json<HookEvent>,
) -> Result<Json<EventResponse>, ApiError> {
    if event.command.trim().is_empty() {
        return Err(ApiError::InvalidEvent("command must not be empty".to_string()));
    }

    let event_id = Uuid::new_v4();
    info!(
        event_id = %event_id,
        kind = ?event.kind,
        command = %event.command,
        exit_code = event.exit_code,
        "Received hook event"
    );

    let _recorded = RecordedEvent { id: event_id, received_at: Utc::now(), event };

    // TODO: persist event and optionally forward to notify service

    Ok(Json(EventResponse {
        success: true,
        event_id,
        message: format!("Event {event_id} recorded"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn make_state() -> ApiState {
        ApiState::default()
    }

    #[tokio::test]
    async fn test_health_check_returns_200() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/health").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check_body() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/health").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["service"], "agentd-hook");
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn test_receive_event_returns_200() {
        let router = create_router(make_state());
        let payload = serde_json::json!({
            "kind": "shell",
            "command": "cargo build",
            "exit_code": 0,
            "duration_ms": 1200
        });
        let req = Request::builder()
            .method("POST")
            .uri("/events")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_receive_event_response_body() {
        let router = create_router(make_state());
        let payload = serde_json::json!({
            "kind": "git",
            "command": "pre-commit",
            "exit_code": 0
        });
        let req = Request::builder()
            .method("POST")
            .uri("/events")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert!(json["event_id"].is_string());
        assert!(json["message"].as_str().unwrap().contains("recorded"));
    }

    #[tokio::test]
    async fn test_receive_event_rejects_empty_command() {
        let router = create_router(make_state());
        let payload = serde_json::json!({
            "kind": "shell",
            "command": "   ",
            "exit_code": 1
        });
        let req = Request::builder()
            .method("POST")
            .uri("/events")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_receive_event_rejects_missing_fields() {
        let router = create_router(make_state());
        // Missing required fields
        let payload = serde_json::json!({ "exit_code": 0 });
        let req = Request::builder()
            .method("POST")
            .uri("/events")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        // axum returns 422 for deserialization errors
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
