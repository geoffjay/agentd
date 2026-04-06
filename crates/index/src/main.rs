//! agentd-index service entry point.
//!
//! Initialises the HTTP server with health, metrics, and search endpoints.
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
//! | Variable                        | Default                        | Description               |
//! |---------------------------------|--------------------------------|---------------------------|
//! | `RUST_LOG`                      | `info`                         | Log level                 |
//! | `AGENTD_PORT`                   | `17012`                        | HTTP listen port          |
//! | `AGENTD_INDEX_LANCE_PATH`       | XDG data dir / `lancedb`       | LanceDB directory         |
//! | `AGENTD_INDEX_EMBEDDING_MODEL`  | `nomic-embed-code`             | Embedding model           |
//!
//! # Endpoints
//!
//! - `GET /health`  — health check
//! - `GET /metrics` — Prometheus metrics
//! - `POST /search` — semantic vector search over code chunks

use axum::{extract::State, response::IntoResponse, routing::get};
use index::api::{create_router_with_state, AppState};
use index::config::IndexConfig;
use index::store::create_store;
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

    // Ensure LanceDB directory exists.
    std::fs::create_dir_all(&config.lance.path)?;
    info!(lance_path = %config.lance.path, "LanceDB directory ready");

    // ── Vector store ─────────────────────────────────────────────────────
    let store = create_store(&config.lance, &config.embedding).await?;
    store.initialize().await?;
    info!("Vector store initialised");

    // ── Metrics ──────────────────────────────────────────────────────────
    let metrics_handle = init_metrics();

    // ── Router ───────────────────────────────────────────────────────────
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router_with_state(AppState { store })
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let addr = format!("127.0.0.1:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Index API server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
