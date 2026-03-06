//! REST API endpoints and routing for the monitor service.
//!
//! Provides the following endpoints:
//!
//! - `GET /health`   — standard health check
//! - `GET /metrics`  — list current metric reports
//! - `GET /alerts`   — list active alerts

use crate::{
    error::ApiError,
    types::{Alert, AlertsResponse, HealthResponse, MetricReport, MetricsResponse},
};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use std::sync::{Arc, RwLock};
use tracing::debug;

/// In-memory store for the monitor service.
///
/// Holds metric reports and active alerts. Will be replaced with a more
/// sophisticated storage layer as the service matures.
#[derive(Default)]
pub struct Store {
    pub metrics: Vec<MetricReport>,
    pub alerts: Vec<Alert>,
}

/// Shared state passed to every API handler.
#[derive(Clone)]
pub struct ApiState {
    /// Service name reported in health check responses
    pub service_name: &'static str,
    /// In-memory store shared across handlers
    pub store: Arc<RwLock<Store>>,
}

impl Default for ApiState {
    fn default() -> Self {
        Self { service_name: "agentd-monitor", store: Arc::new(RwLock::new(Store::default())) }
    }
}

/// Create the base router (no middleware).
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .route("/alerts", get(get_alerts))
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
    let store = state.store.read().expect("store lock poisoned");
    let metric_count = store.metrics.len();
    let alert_count = store.alerts.len();
    drop(store);

    Json(
        HealthResponse::ok(state.service_name, env!("CARGO_PKG_VERSION"))
            .with_detail("metrics_count", serde_json::json!(metric_count))
            .with_detail("alert_count", serde_json::json!(alert_count)),
    )
}

/// `GET /metrics` — return all current metric reports.
///
/// Returns an empty list if no metrics have been collected yet.
async fn get_metrics(State(state): State<ApiState>) -> Result<impl IntoResponse, ApiError> {
    let store = state.store.read().map_err(|e| ApiError::Internal(e.to_string()))?;
    let metrics = store.metrics.clone();
    let count = metrics.len();
    debug!("Returning {} metric reports", count);
    Ok(Json(MetricsResponse { metrics, count }))
}

/// `GET /alerts` — return all active alerts.
///
/// Returns an empty list if there are no active alerts.
async fn get_alerts(State(state): State<ApiState>) -> Result<impl IntoResponse, ApiError> {
    let store = state.store.read().map_err(|e| ApiError::Internal(e.to_string()))?;
    let alerts = store.alerts.clone();
    let count = alerts.len();
    debug!("Returning {} active alerts", count);
    Ok(Json(AlertsResponse { alerts, count }))
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
        assert_eq!(json["service"], "agentd-monitor");
        assert_eq!(json["status"], "ok");
        assert_eq!(json["details"]["metrics_count"], 0);
        assert_eq!(json["details"]["alert_count"], 0);
    }

    #[tokio::test]
    async fn test_get_metrics_returns_empty_list_initially() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/metrics").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["count"], 0);
        assert!(json["metrics"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_alerts_returns_empty_list_initially() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/alerts").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["count"], 0);
        assert!(json["alerts"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_metrics_reflects_stored_data() {
        let state = make_state();
        {
            let mut store = state.store.write().unwrap();
            store.metrics.push(MetricReport {
                name: "cpu.usage".to_string(),
                value: 55.0,
                unit: Some("percent".to_string()),
                observed_at: None,
                tags: std::collections::HashMap::new(),
            });
        }
        let router = create_router(state);
        let req = Request::builder().uri("/metrics").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["count"], 1);
        assert_eq!(json["metrics"][0]["name"], "cpu.usage");
    }

    #[tokio::test]
    async fn test_get_alerts_reflects_stored_data() {
        use chrono::Utc;
        use uuid::Uuid;
        let state = make_state();
        {
            let mut store = state.store.write().unwrap();
            store.alerts.push(Alert {
                id: Uuid::new_v4(),
                metric: "memory.usage".to_string(),
                current_value: 95.0,
                threshold: 90.0,
                message: "Memory critical".to_string(),
                raised_at: Utc::now(),
            });
        }
        let router = create_router(state);
        let req = Request::builder().uri("/alerts").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["count"], 1);
        assert_eq!(json["alerts"][0]["metric"], "memory.usage");
    }
}
