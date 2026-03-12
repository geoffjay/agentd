//! agentd-memory service entry point.
//!
//! This is the main executable for the memory service. It initializes the
//! HTTP server with health and metrics endpoints. Storage backends (SQLite
//! via SeaORM for metadata and LanceDB for vector embeddings) will be
//! initialized in subsequent issues.
//!
//! # Features
//!
//! - REST API on `http://127.0.0.1:17008` (dev default)
//! - Structured logging with tracing
//! - Prometheus metrics endpoint
//!
//! # Running the Service
//!
//! ```bash
//! # Run with default INFO logging
//! cargo run -p memory
//!
//! # Run with DEBUG logging
//! RUST_LOG=debug cargo run -p memory
//!
//! # Run the release build
//! cargo run -p memory --release
//! ```
//!
//! # Environment Variables
//!
//! - `RUST_LOG` - Controls logging level (e.g., `debug`, `info`, `warn`, `error`)
//!   Defaults to `info` if not set.
//! - `AGENTD_PORT` - Override the listen port (default `17008` dev / `7008` prod)
//!
//! # API Endpoints
//!
//! Once running, the service exposes the following endpoints:
//!
//! - `GET /health` - Health check
//! - `GET /metrics` - Prometheus metrics

mod api;
mod types;

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

/// Main entry point for the agentd-memory service.
///
/// This function performs the following initialization steps:
/// 1. Sets up structured logging with tracing
/// 2. Initializes Prometheus metrics
/// 3. Creates and configures the Axum HTTP router
/// 4. Starts the HTTP server on `127.0.0.1:17008` (dev default)
///
/// # Returns
///
/// Returns `Ok(())` on successful shutdown, or an error if initialization
/// or server startup fails.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to bind to the network address
/// - The HTTP server encounters a fatal error
///
/// # Panics
///
/// Does not panic under normal operation.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-memory service...");

    // Initialize Prometheus metrics
    let metrics_handle = init_metrics();

    // Create API state and router with metrics endpoint and tracing middleware
    let api_state = ApiState {};
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router(api_state)
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    // Bind to address (use AGENTD_PORT env var, default 17008 for dev, 7008 for production)
    let port = env::var("AGENTD_PORT").unwrap_or_else(|_| "17008".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Memory API server listening on http://{}", addr);

    // Start the HTTP server
    axum::serve(listener, app).await?;

    Ok(())
}
