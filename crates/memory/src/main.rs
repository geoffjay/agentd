//! agentd-memory service entry point.
//!
//! Initialises the LanceDB vector store and HTTP server with full CRUD,
//! semantic search, health, and metrics endpoints.
//!
//! # Running the Service
//!
//! ```bash
//! cargo run -p memory
//! RUST_LOG=debug cargo run -p memory
//! ```
//!
//! # Environment Variables
//!
//! | Variable                             | Default                        | Description                     |
//! |--------------------------------------|--------------------------------|---------------------------------|
//! | `RUST_LOG`                           | `info`                         | Log level                       |
//! | `AGENTD_PORT`                        | `17008`                        | Listen port                     |
//! | `AGENTD_MEMORY_EMBEDDING_PROVIDER`   | `none`                         | `openai` or `none`              |
//! | `AGENTD_MEMORY_EMBEDDING_MODEL`      | `text-embedding-3-small`       | Model name                      |
//! | `AGENTD_MEMORY_EMBEDDING_API_KEY`    | —                              | API key for remote providers    |
//! | `AGENTD_MEMORY_EMBEDDING_ENDPOINT`   | `https://api.openai.com/v1`    | Base URL (use Ollama URL local) |
//! | `AGENTD_MEMORY_LANCE_PATH`           | XDG data dir / `lancedb`       | LanceDB directory path          |
//! | `AGENTD_MEMORY_LANCE_TABLE`          | `memories`                     | LanceDB table name              |
//!
//! # Endpoints
//!
//! - `GET  /health`                  — health check (DB + LanceDB status)
//! - `GET  /metrics`                 — Prometheus metrics
//! - `POST /memories`                — create a new memory
//! - `GET  /memories`                — list memories with filters
//! - `GET  /memories/:id`            — retrieve a memory by ID
//! - `DELETE /memories/:id`          — delete a memory
//! - `PUT  /memories/:id/visibility` — update visibility & share list
//! - `POST /memories/search`         — semantic similarity search

mod api;

use api::{create_router, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
use memory::config::{EmbeddingConfig, LanceConfig};
use metrics_exporter_prometheus::PrometheusHandle;
use std::env;
use tracing::info;

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info", "version" => env!("CARGO_PKG_VERSION"), "service" => "memory")
        .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-memory service...");

    // ── Metrics ──────────────────────────────────────────────────────────
    let metrics_handle = init_metrics();

    // ── Vector store ─────────────────────────────────────────────────────
    let lance_config = LanceConfig::from_env();
    let embedding_config = EmbeddingConfig::from_env();

    info!(
        lance_path = %lance_config.path,
        lance_table = %lance_config.table,
        embedding_provider = %embedding_config.provider,
        embedding_model = %embedding_config.model,
        "Initialising vector store"
    );

    let store = memory::store::create_store(&lance_config, &embedding_config).await?;
    store.initialize().await?;

    info!("Vector store initialised");

    // ── Router ───────────────────────────────────────────────────────────
    let api_state = ApiState { store };
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router(api_state)
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let port = env::var("AGENTD_PORT").unwrap_or_else(|_| "17008".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Memory API server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
