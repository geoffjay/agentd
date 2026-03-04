//! agentd-wrap service entry point.
//!
//! This is the main executable for the wrap service. It provides a REST API
//! for launching and managing agent sessions in tmux environments.
//!
//! # Features
//!
//! - REST API on `http://127.0.0.1:17005` (dev) or port from PORT env var
//! - Tmux session management for agent workflows
//! - Support for multiple agent types (claude-code, opencode, gemini)
//! - Structured logging with tracing
//! - Graceful shutdown support
//!
//! # Running the Service
//!
//! ```bash
//! # Run with default INFO logging
//! cargo run -p agentd-wrap
//!
//! # Run with DEBUG logging
//! RUST_LOG=debug cargo run -p agentd-wrap
//!
//! # Run on a different port
//! PORT=8080 cargo run -p agentd-wrap
//! ```
//!
//! # Environment Variables
//!
//! - `RUST_LOG` - Controls logging level (e.g., `debug`, `info`, `warn`, `error`)
//!   Defaults to `info` if not set.
//! - `PORT` - Port to listen on. Defaults to `17005` for development.
//!
//! # API Endpoints
//!
//! Once running, the service exposes the following endpoints:
//!
//! - `GET /health` - Health check
//! - `POST /launch` - Launch an agent session in tmux
//!
//! # Examples
//!
//! ```bash
//! # Start the service
//! cargo run -p agentd-wrap
//!
//! # In another terminal, test the API
//! curl http://localhost:17005/health
//!
//! # Launch a session
//! curl -X POST http://localhost:17005/launch \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "project_name": "my-project",
//!     "project_path": "/path/to/project",
//!     "agent_type": "claude-code",
//!     "model_provider": "anthropic",
//!     "model_name": "claude-sonnet-4.5"
//!   }'
//! ```

mod api;
mod tmux;
mod types;

use api::create_router;
use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::env;
use tracing::info;

/// Initialize Prometheus metrics recorder and return a handle for rendering.
fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");

    // Register service metadata gauge
    metrics::gauge!("service_info", "version" => env!("CARGO_PKG_VERSION"), "service" => "wrap")
        .set(1.0);

    handle
}

/// GET /metrics — render Prometheus text format.
async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

/// Main entry point for the agentd-wrap service.
///
/// This function performs the following initialization steps:
/// 1. Sets up structured logging with tracing
/// 2. Creates and configures the Axum HTTP router
/// 3. Starts the HTTP server on the configured port
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

    info!("Starting agentd-wrap service...");

    // Initialize Prometheus metrics
    let metrics_handle = init_metrics();

    // Create API router with metrics endpoint and tracing middleware
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router()
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer());

    // Bind to address (use PORT env var, default 17005 for dev)
    let port = env::var("PORT").unwrap_or_else(|_| "17005".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Wrap API server listening on http://{}", addr);

    // Start the HTTP server
    axum::serve(listener, app).await?;

    Ok(())
}
