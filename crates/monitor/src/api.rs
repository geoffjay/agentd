//! REST API endpoints and routing for the monitor service.
//!
//! Provides the following endpoints:
//!
//! - `GET /health`            — standard health check
//! - `GET /metrics`           — latest system metrics snapshot
//! - `POST /collect`          — trigger an immediate metrics collection
//! - `GET /history`           — full metrics history (ring buffer)
//! - `GET /status`            — health assessment against configured thresholds
//! - `GET /queries`           — the curated named-query catalog
//! - `GET /queries/{name}`    — execute a named query against Prometheus
//! - `GET /query`             — raw PromQL passthrough (read-only escape hatch)

use crate::{
    error::ApiError,
    metrics_collector,
    prometheus::{PromClient, PromData, PromError},
    queries,
    state::AppState,
    types::{CollectResponse, HealthResponse, SystemStatus},
};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// Shared state passed to every API handler.
#[derive(Clone)]
pub struct ApiState {
    pub app_state: AppState,
    pub prom: Arc<PromClient>,
}

/// Create the base router (no middleware).
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .route("/collect", post(collect_metrics))
        .route("/history", get(get_history))
        .route("/status", get(get_status))
        .route("/queries", get(list_queries))
        .route("/queries/{name}", get(run_named_query))
        .route("/query", get(run_raw_query))
        .with_state(state)
}

/// Create the router with HTTP tracing middleware.
pub fn create_router_with_tracing(state: ApiState) -> Router {
    create_router(state).layer(agentd_common::server::trace_layer())
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

// ---------------------------------------------------------------------------
// Prometheus named queries
// ---------------------------------------------------------------------------

/// Query-string parameters for query execution endpoints.
#[derive(Debug, Default, Deserialize)]
struct QueryParams {
    /// `$__window` substitution (e.g. `15m`, `1h`). Defaults per query.
    window: Option<String>,
    /// `instant` (default) or `range`.
    mode: Option<String>,
    /// Range mode: how far back from now the range starts (default: 360).
    range_minutes: Option<u64>,
    /// Range mode: resolution step in seconds (default: 60).
    step_secs: Option<u64>,
    /// Raw passthrough only: the PromQL expression.
    promql: Option<String>,
}

/// Response body for query execution endpoints.
#[derive(Debug, Serialize)]
struct QueryResult {
    /// Catalog name, or `"raw"` for the passthrough endpoint.
    name: String,
    /// The executed PromQL after window substitution.
    promql: String,
    /// `instant` or `range`.
    mode: String,
    executed_at: DateTime<Utc>,
    data: PromData,
}

fn map_prom_error(e: PromError) -> ApiError {
    match e {
        PromError::Query { .. } => ApiError::BadQueryParam(e.to_string()),
        _ => ApiError::PrometheusUnavailable(e.to_string()),
    }
}

/// Execute resolved PromQL in the requested mode.
async fn execute_query(
    state: &ApiState,
    name: &str,
    promql: String,
    params: &QueryParams,
) -> Result<Json<QueryResult>, ApiError> {
    let mode = params.mode.as_deref().unwrap_or("instant");
    let data = match mode {
        "instant" => state.prom.query(&promql).await.map_err(map_prom_error)?,
        "range" => {
            let end = Utc::now();
            let minutes = params.range_minutes.unwrap_or(360).min(7 * 24 * 60);
            let step = params.step_secs.unwrap_or(60).max(1);
            let start = end - Duration::minutes(minutes as i64);
            state.prom.query_range(&promql, start, end, step).await.map_err(map_prom_error)?
        }
        other => {
            return Err(ApiError::BadQueryParam(format!(
                "invalid mode `{other}` — expected `instant` or `range`"
            )));
        }
    };

    Ok(Json(QueryResult {
        name: name.to_string(),
        promql,
        mode: mode.to_string(),
        executed_at: Utc::now(),
        data,
    }))
}

/// `GET /queries` — the catalog with descriptions. Always 200; never touches
/// Prometheus.
async fn list_queries() -> Json<&'static [queries::NamedQuery]> {
    Json(queries::QUERY_CATALOG)
}

/// `GET /queries/{name}?window=1h&mode=instant|range` — run a named query.
async fn run_named_query(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    Query(params): Query<QueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let query = queries::find(&name).ok_or_else(|| ApiError::UnknownQuery(name.clone()))?;
    let promql =
        queries::resolve(query, params.window.as_deref()).map_err(ApiError::BadQueryParam)?;
    execute_query(&state, &name, promql, &params).await
}

/// `GET /query?promql=...` — raw PromQL passthrough.
///
/// Read-only escape hatch for ad-hoc analysis; Prometheus itself listens on
/// loopback only. Prefer the named catalog for anything recurring.
async fn run_raw_query(
    State(state): State<ApiState>,
    Query(params): Query<QueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let promql = params
        .promql
        .clone()
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| ApiError::BadQueryParam("missing `promql` parameter".to_string()))?;
    execute_query(&state, "raw", promql, &params).await
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
        // Port 1 — connection refused; only the Prometheus-backed endpoints
        // touch it, and those tests use stub_prometheus() instead.
        make_state_with_prometheus("http://127.0.0.1:1")
    }

    fn make_state_with_prometheus(prometheus_url: &str) -> ApiState {
        ApiState {
            app_state: AppState::new(MonitorConfig::default()),
            prom: Arc::new(PromClient::new(prometheus_url.to_string())),
        }
    }

    /// Spawn a stub Prometheus answering /api/v1/query and /api/v1/query_range
    /// with canned vector/matrix envelopes. Returns its base URL.
    async fn stub_prometheus() -> String {
        let router = Router::new()
            .route(
                "/api/v1/query",
                get(|| async {
                    Json(serde_json::json!({
                        "status": "success",
                        "data": {
                            "resultType": "vector",
                            "result": [
                                {"metric": {"service": "orchestrator"},
                                 "value": [1718100000.0, "3"]}
                            ]
                        }
                    }))
                }),
            )
            .route(
                "/api/v1/query_range",
                get(|| async {
                    Json(serde_json::json!({
                        "status": "success",
                        "data": {
                            "resultType": "matrix",
                            "result": [
                                {"metric": {"service": "orchestrator"},
                                 "values": [[1718100000.0, "1"], [1718100060.0, "2"]]}
                            ]
                        }
                    }))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://{addr}")
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

    // -----------------------------------------------------------------------
    // Named-query endpoints
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_queries_returns_catalog_without_prometheus() {
        // Prometheus deliberately unreachable — the catalog is static.
        let router = create_router(make_state());
        let req = Request::builder().uri("/queries").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let entries = json.as_array().unwrap();
        assert_eq!(entries.len(), crate::queries::QUERY_CATALOG.len());
        assert!(entries.iter().any(|e| e["name"] == "dispatch-success-rate"));
    }

    #[tokio::test]
    async fn test_named_query_instant_happy_path() {
        let prom_url = stub_prometheus().await;
        let router = create_router(make_state_with_prometheus(&prom_url));
        let req = Request::builder()
            .uri("/queries/dispatch-success-rate?window=15m")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["name"], "dispatch-success-rate");
        assert_eq!(json["mode"], "instant");
        assert!(json["promql"].as_str().unwrap().contains("[15m]"));
        assert_eq!(json["data"]["resultType"], "vector");
    }

    #[tokio::test]
    async fn test_named_query_range_mode() {
        let prom_url = stub_prometheus().await;
        let router = create_router(make_state_with_prometheus(&prom_url));
        let req = Request::builder()
            .uri("/queries/dispatch-throughput?mode=range&range_minutes=60&step_secs=60")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["mode"], "range");
        assert_eq!(json["data"]["resultType"], "matrix");
    }

    #[tokio::test]
    async fn test_unknown_query_returns_404_with_catalog() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/queries/bogus").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json["error"].as_str().unwrap().contains("dispatch-success-rate"),
            "404 should list the catalog: {json}"
        );
    }

    #[tokio::test]
    async fn test_bad_window_returns_400() {
        let router = create_router(make_state());
        let req = Request::builder()
            .uri("/queries/dispatch-success-rate?window=1h)%20or%20vector(1)")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_prometheus_down_returns_502() {
        // make_state points the client at a refused port.
        let router = create_router(make_state());
        let req = Request::builder().uri("/queries/agents-active").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("unreachable"), "{json}");
    }

    #[tokio::test]
    async fn test_raw_query_requires_promql() {
        let router = create_router(make_state());
        let req = Request::builder().uri("/query").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_raw_query_passthrough() {
        let prom_url = stub_prometheus().await;
        let router = create_router(make_state_with_prometheus(&prom_url));
        let req = Request::builder().uri("/query?promql=up").body(Body::empty()).unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["name"], "raw");
        assert_eq!(json["promql"], "up");
    }
}
