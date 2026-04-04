//! agentd-index service entry point.
//!
//! Initialises the HTTP server with health and metrics endpoints.
//! Additional capabilities (chunking, embeddings, search) are added by
//! subsequent issues in the v0.14.0 milestone.
//!
//! # Running the Service
//!
//! ```bash
//! cargo run -p index
//! RUST_LOG=debug cargo run -p index
//! ```
//!
//! # Environment Variables
//!
//! | Variable                  | Default                        | Description         |
//! |---------------------------|--------------------------------|---------------------|
//! | `RUST_LOG`                | `info`                         | Log level           |
//! | `AGENTD_PORT`             | `17012`                        | HTTP listen port    |
//! | `AGENTD_INDEX_DATA_PATH`  | XDG data dir / `agentd-index`  | Data directory      |
//!
//! # Endpoints
//!
//! - `GET /health`  — health check
//! - `GET /metrics` — Prometheus metrics

use axum::{extract::State, response::IntoResponse, routing::get};
use index::api::create_router;
use index::config::IndexConfig;
use metrics_exporter_prometheus::PrometheusHandle;
use tracing::info;

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info", "version" => env!("CARGO_PKG_VERSION"), "service" => "index")
        .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-index service...");

    let config = IndexConfig::from_env();

    // Ensure data directory exists.
    std::fs::create_dir_all(&config.data_path)?;
    info!(data_path = %config.data_path.display(), "Data directory ready");

    // ── Metrics ──────────────────────────────────────────────────────────
    let metrics_handle = init_metrics();

    // ── Router ───────────────────────────────────────────────────────────
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router()
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let addr = format!("127.0.0.1:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Index API server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
