//! agentd-communicate service entry point.
//!
//! This is the main executable for the inter-agent communication service. It
//! initializes the storage backend, sets up the REST API server, and starts
//! listening for HTTP requests.
//!
//! # Features
//!
//! - SQLite-based persistent storage
//! - REST API on `http://127.0.0.1:17010` (dev default)
//! - Structured logging with tracing
//! - Prometheus metrics endpoint
//! - CORS support
//!
//! # Running the Service
//!
//! ```bash
//! # Run with default INFO logging
//! cargo run -p agentd-communicate
//!
//! # Run with DEBUG logging
//! RUST_LOG=debug cargo run -p agentd-communicate
//! ```
//!
//! # Environment Variables
//!
//! - `RUST_LOG` — Controls logging level (e.g., `debug`, `info`, `warn`, `error`).
//!   Defaults to `info` if not set.
//! - `AGENTD_PORT` — Override the listening port. Defaults to `17010`.
//!
//! # API Endpoints
//!
//! - `GET /health` — Health check
//! - `GET /metrics` — Prometheus metrics
//!
//! # Database Location
//!
//! The SQLite database is stored at a platform-specific location:
//! - Linux: `~/.local/share/agentd-communicate/communicate.db`
//! - macOS: `~/Library/Application Support/agentd-communicate/communicate.db`

mod api;
mod entity;
mod migration;
mod storage;
mod types;

use api::{create_router, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::env;
use std::sync::Arc;
use storage::CommunicateStorage;
use tracing::info;

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!(
        "service_info",
        "version" => env!("CARGO_PKG_VERSION"),
        "service" => "communicate"
    )
    .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

/// Main entry point for the agentd-communicate service.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-communicate service...");

    // Initialize SQLite storage
    let storage = CommunicateStorage::new().await?;
    info!("Communicate storage initialized at: {:?}", CommunicateStorage::get_db_path()?);

    let storage = Arc::new(storage);

    // Initialize Prometheus metrics
    let metrics_handle = init_metrics();

    // Create API state and router with metrics endpoint and tracing middleware
    let api_state = ApiState { storage };
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router(api_state)
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    // Bind to address (use AGENTD_PORT env var, default 17010 for dev, 7010 for production)
    let port = env::var("AGENTD_PORT").unwrap_or_else(|_| "17010".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Communicate API server listening on http://{}", addr);

    // Start the HTTP server
    axum::serve(listener, app).await?;

    Ok(())
}
