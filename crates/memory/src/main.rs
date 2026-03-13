//! agentd-memory service entry point.
//!
//! Initialises the HTTP server with `/health` and `/metrics` endpoints.
//! Storage backends (SQLite via SeaORM for metadata and LanceDB for vector
//! embeddings) will be wired up in subsequent issues.
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
//! - `RUST_LOG` — log level (default: `info`)
//! - `AGENTD_PORT` — listen port (default: `17008` dev / `7008` prod)
//!
//! # Endpoints
//!
//! - `GET /health`  — health check
//! - `GET /metrics` — Prometheus metrics

mod api;
pub mod config;
pub mod error;
pub mod store;
pub mod types;

use api::{create_router, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
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

    let metrics_handle = init_metrics();

    let api_state = ApiState {};
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
