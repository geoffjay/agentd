//! REST API endpoints and routing for the monitor service.
//!
//! Provides the following endpoints:
//!
//! - `GET /health`   — standard health check
//! - `GET /metrics`  — latest system metrics snapshot
//! - `POST /collect` — trigger an immediate metrics collection
//! - `GET /history`  — full metrics history (ring buffer)
//! - `GET /status`   — health assessment against configured thresholds

use crate::{
    error::ApiError,
    metrics_collector,
    state::AppState,
    types::{CollectResponse, HealthResponse, SystemStatus},
};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use tracing::info;

/// Shared state passed to every API handler.
#[derive(Clone)]
pub struct ApiState {
    pub app_state: AppState,
}

/// Create the base router (no middleware).
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .route("/collect", post(collect_metrics))
        .route("/history", get(get_history))
        .route("/status", get(get_status))
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
///
/// Returns HTTP 200 with service name, version, and collection count.
async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    let count = state.app_state.metrics_count().await;
    Json(
        HealthResponse::ok("agentd-monitor", env!("CARGO_PKG_VERSION"))
            .with_detail("metrics_collected", serde_json::json!(count)),
    )
}

/// `GET /metrics` — return the latest metrics snapshot.
///
/// Returns HTTP 503 if no collection has run yet.
async fn get_metrics(State(state): State<ApiState>) -> Result<impl IntoResponse, ApiError> {
    state.app_state.latest_metrics().await.map(Json).ok_or(ApiError::NoMetricsAvailable)
}

/// `POST /collect` — trigger an immediate metrics collection.
///
/// Collects fresh metrics, stores them in state, and returns the snapshot
/// along with any threshold alerts.
async fn collect_metrics(State(state): State<ApiState>) -> impl IntoResponse {
    info!("Collecting system metrics on demand");
    let metrics = metrics_collector::collect();
    state.app_state.push_metrics(metrics.clone()).await;
    let system_status = state.app_state.evaluate_status().await;

    Json(CollectResponse { metrics, alerts: system_status.alerts })
}

/// `GET /history` — return all retained metrics snapshots.
async fn get_history(State(state): State<ApiState>) -> impl IntoResponse {
    let history = state.app_state.all_metrics().await;
    Json(history)
}

/// `GET /status` — evaluate current health against thresholds.
async fn get_status(State(state): State<ApiState>) -> Json<SystemStatus> {
    Json(state.app_state.evaluate_status().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MonitorConfig;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn make_state() -> ApiState {
        ApiState { app_state: AppState::new(MonitorConfig::default()) }
    }

    #[tokio::test]
    async fn test_health_check_returns_200() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/health").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check_contains_service_name() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/health").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["service"], "agentd-monitor");
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn test_get_metrics_returns_503_when_empty() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/metrics").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_collect_returns_200() {
        let router = create_router(make_state());
        let req = Request::builder().method("POST").uri("/collect").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_collect_then_get_metrics() {
        let state = make_state();
        let router = create_router(state.clone());

        // Collect first
        let collect_req =
            Request::builder().method("POST").uri("/collect").body(Body::empty()).unwrap();
        let collect_resp = router.clone().oneshot(collect_req).await.unwrap();
        assert_eq!(collect_resp.status(), StatusCode::OK);

        // Now GET /metrics should return data
        let metrics_req = Request::builder().uri("/metrics").body(Body::empty()).unwrap();
        let metrics_resp = router.oneshot(metrics_req).await.unwrap();
        assert_eq!(metrics_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_history_returns_empty_array_initially() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/history").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_status_returns_healthy_initially() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/status").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "healthy");
    }
}
