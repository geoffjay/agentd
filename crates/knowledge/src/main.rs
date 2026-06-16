//! agentd-knowledge service entry point.
//!
//! This is the main executable for the per-project knowledgebase service.
//! It initializes the SQLite metadata storage, sets up the REST API server,
//! and starts serving on the configured host/port.
//!
//! # Features
//!
//! - SQLite-backed document metadata + filesystem markdown store
//! - REST API on `http://127.0.0.1:17011` (dev default)
//! - Structured logging with tracing
//!
//! # Running the Service
//!
//! ```bash
//! cargo run -p knowledge
//! RUST_LOG=debug cargo run -p knowledge
//! ```
//!
//! # Environment Variables
//!
//! - `AGENTD_KNOWLEDGE_PORT` — HTTP listen port (default `17011`)
//! - `AGENTD_KNOWLEDGE_ROOT` — Document storage root directory
//! - `RUST_LOG` — Log level

mod api;
mod config;
mod entity;
mod migration;
mod storage;
mod types;
// client and error are pub in lib.rs; reference them to avoid dead-code in binary
mod client;
mod error;

use agentd_common::config::ValidateConfig;
use axum::{extract::State, response::IntoResponse, routing::get};
use config::KnowledgeConfig;
use metrics_exporter_prometheus::PrometheusHandle;
use std::path::PathBuf;
use tracing::info;

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!(
        "service_info",
        "version" => env!("CARGO_PKG_VERSION"),
        "service" => "knowledge"
    )
    .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-knowledge service...");

    let cfg = KnowledgeConfig::load();
    cfg.validate()?;

    let kb_root = PathBuf::from(&cfg.root);
    std::fs::create_dir_all(&kb_root)?;

    let db_path = storage::KnowledgeStorage::get_db_path()?;
    let storage =
        std::sync::Arc::new(storage::KnowledgeStorage::with_path(&db_path, &kb_root).await?);
    info!("Knowledge storage initialized at: {:?}", db_path);

    let metrics_handle = init_metrics();

    let api_router = api::create_router_with_state(storage);
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = api_router
        .merge(metrics_router)
        .layer(agentd_common::server::metrics_layer())
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Knowledge API server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
