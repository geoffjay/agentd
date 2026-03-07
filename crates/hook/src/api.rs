//! REST API endpoints and routing for the hook service.
//!
//! Provides the following endpoints:
//!
//! - `GET /health`          — standard health check
//! - `POST /events`         — receive and record a hook event
//! - `GET /events`          — list recent events (query param: `?limit=N`)
//! - `GET /events/:id`      — fetch a single event by UUID
//! - `GET /shell/:shell`    — generate shell integration script

use crate::{
    config::HookConfig,
    error::ApiError,
    shell::{self, Shell},
    state::AppState,
    types::{EventResponse, HealthResponse, HookEvent, RecordedEvent},
};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use tracing::info;
use uuid::Uuid;

/// Shared state passed to every API handler.
#[derive(Clone)]
pub struct ApiState {
    /// Thread-safe event store
    pub app_state: AppState,
}

impl ApiState {
    /// Create a new `ApiState` with the given configuration.
    pub fn new(config: HookConfig) -> Self {
        Self { app_state: AppState::new(config) }
    }
}

impl Default for ApiState {
    fn default() -> Self {
        Self::new(HookConfig::default())
    }
}

/// Create the base router (no middleware).
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/events", get(list_events).post(receive_event))
        .route("/events/{id}", get(get_event))
        .route("/shell/{shell}", get(shell_integration))
        .with_state(state)
}

/// Create the router with HTTP tracing middleware.
pub fn create_router_with_tracing(state: ApiState) -> Router {
    create_router(state).layer(agentd_common::server::trace_layer())
}

/// `GET /health` — standard health check.
async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    let count = state.app_state.event_count().await;
    let config = state.app_state.config().await;
    Json(
        HealthResponse::ok("agentd-hook", env!("CARGO_PKG_VERSION"))
            .with_detail("events_recorded", serde_json::json!(count))
            .with_detail("notify_on_failure", serde_json::json!(config.notify_on_failure))
            .with_detail(
                "notify_on_long_running",
                serde_json::json!(config.notify_on_long_running),
            ),
    )
}

/// Query parameters for `GET /events`.
#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    /// Maximum number of events to return (newest first, default: 50, max: 500)
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// `GET /events` — list recent events, newest first.
async fn list_events(
    State(state): State<ApiState>,
    Query(params): Query<ListEventsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.clamp(1, 500);
    let events = state.app_state.recent_events(limit).await;
    Json(events)
}

/// `GET /events/:id` — fetch a single event by UUID.
async fn get_event(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RecordedEvent>, ApiError> {
    state
        .app_state
        .get_event(&id)
        .await
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Event {id} not found")))
}

/// `POST /events` — receive and record a hook event.
///
/// Accepts a [`HookEvent`] JSON body, assigns it a UUID, timestamps it, stores
/// it in the ring buffer, and logs notable events.
async fn receive_event(
    State(state): State<ApiState>,
    Json(event): Json<HookEvent>,
) -> Result<Json<EventResponse>, ApiError> {
    if event.command.trim().is_empty() {
        return Err(ApiError::InvalidEvent("command must not be empty".to_string()));
    }

    let event_id = Uuid::new_v4();
    let config = state.app_state.config().await;

    info!(
        event_id = %event_id,
        kind = ?event.kind,
        command = %event.command,
        exit_code = event.exit_code,
        duration_ms = event.duration_ms,
        "Received hook event"
    );

    let is_failure = event.exit_code != 0;
    let is_long_running = event.duration_ms >= config.long_running_threshold_ms;

    if is_failure && config.notify_on_failure {
        info!(
            command = %event.command,
            exit_code = event.exit_code,
            "Command failed — notification eligible"
        );
    }
    if is_long_running && config.notify_on_long_running {
        info!(
            command = %event.command,
            duration_ms = event.duration_ms,
            "Long-running command completed — notification eligible"
        );
    }

    let recorded = RecordedEvent { id: event_id, received_at: Utc::now(), event };
    state.app_state.push_event(recorded).await;

    Ok(Json(EventResponse {
        success: true,
        event_id,
        message: format!("Event {event_id} recorded"),
    }))
}

/// `GET /shell/:shell` — return a shell integration script.
///
/// Supports `zsh`, `bash`, and `fish`.
async fn shell_integration(
    State(state): State<ApiState>,
    Path(shell_name): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let config = state.app_state.config().await;
    let hook_url = format!("http://localhost:{}", config.port);

    let shell = Shell::from_str(&shell_name).ok_or_else(|| {
        ApiError::InvalidShell(format!(
            "Unknown shell '{}'. Supported: zsh, bash, fish",
            shell_name
        ))
    })?;

    let script = shell::generate_integration(shell, &hook_url);
    Ok(script)
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
        assert!(json["details"]["events_recorded"].is_number());
    }

    #[tokio::test]
    async fn test_list_events_returns_empty_initially() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/events").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_receive_event_returns_200() {
        let router = create_router(make_state());
        let payload = serde_json::json!({
            "kind": "shell", "command": "cargo build", "exit_code": 0, "duration_ms": 1200
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
        let payload = serde_json::json!({"kind": "git", "command": "pre-commit", "exit_code": 0});
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
    async fn test_receive_then_list_events() {
        let state = make_state();
        let router = create_router(state);
        let payload = serde_json::json!({"kind": "shell", "command": "ls -la", "exit_code": 0});
        let post_req = Request::builder()
            .method("POST")
            .uri("/events")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();
        router.clone().oneshot(post_req).await.unwrap();

        let get_req = Request::builder().uri("/events").body(Body::empty()).unwrap();
        let resp = router.oneshot(get_req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let events: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(events.as_array().unwrap().len(), 1);
        assert_eq!(events[0]["command"], "ls -la");
    }

    #[tokio::test]
    async fn test_receive_event_rejects_empty_command() {
        let router = create_router(make_state());
        let payload = serde_json::json!({"kind": "shell", "command": "   ", "exit_code": 1});
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
        let payload = serde_json::json!({"exit_code": 0});
        let req = Request::builder()
            .method("POST")
            .uri("/events")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_get_event_by_id() {
        let state = make_state();
        let router = create_router(state);
        let payload = serde_json::json!({"kind": "shell", "command": "echo hi", "exit_code": 0});
        let post_req = Request::builder()
            .method("POST")
            .uri("/events")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();
        let post_resp = router.clone().oneshot(post_req).await.unwrap();
        let body = post_resp.into_body().collect().await.unwrap().to_bytes();
        let post_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let event_id = post_json["event_id"].as_str().unwrap();

        let get_req =
            Request::builder().uri(format!("/events/{event_id}")).body(Body::empty()).unwrap();
        let get_resp = router.oneshot(get_req).await.unwrap();
        assert_eq!(get_resp.status(), StatusCode::OK);
        let get_body = get_resp.into_body().collect().await.unwrap().to_bytes();
        let get_json: serde_json::Value = serde_json::from_slice(&get_body).unwrap();
        assert_eq!(get_json["command"], "echo hi");
    }

    #[tokio::test]
    async fn test_get_event_not_found() {
        let router = create_router(make_state());
        let fake_id = Uuid::new_v4();
        let req = Request::builder().uri(format!("/events/{fake_id}")).body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_shell_integration_zsh() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/shell/zsh").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("agentd_hook_preexec"));
    }

    #[tokio::test]
    async fn test_shell_integration_bash() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/shell/bash").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_shell_integration_unknown_shell() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/shell/powershell").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
