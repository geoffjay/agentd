//! Ask service entry point.
//!
//! Initializes the agent-driven Q&A service and runs the HTTP server.
//!
//! # Environment Variables
//!
//! - `AGENTD_PORT` - Port to bind to (default: 17001)
//! - `AGENTD_ORCHESTRATOR_URL` - Orchestrator callback URL (default: http://localhost:17006)
//! - `RUST_LOG` - Logging configuration (default: info)

mod api;
mod entity;
mod error;
mod migration;
mod state;
mod storage;
mod types;

use anyhow::Result;
use api::{create_router_with_tracing, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use state::AppState;
use storage::QuestionStorage;
use tracing::{error, info, warn};

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info", "version" => env!("CARGO_PKG_VERSION"), "service" => "ask")
        .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

#[tokio::main]
async fn main() -> Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-ask service...");

    let port = std::env::var("AGENTD_PORT")
        .unwrap_or_else(|_| "17001".to_string())
        .parse::<u16>()
        .unwrap_or(17001);

    let orchestrator_url = std::env::var("AGENTD_ORCHESTRATOR_URL")
        .unwrap_or_else(|_| "http://localhost:17006".to_string());

    info!("Configuration: port={}, orchestrator={}", port, orchestrator_url);

    // Initialize persistent storage.
    let storage = match QuestionStorage::new().await {
        Ok(s) => {
            info!("Question storage initialized");
            s
        }
        Err(e) => {
            error!("Failed to initialize question storage: {}", e);
            return Err(e);
        }
    };

    let app_state = AppState::new_with_storage(storage);

    let api_state =
        ApiState { app_state: app_state.clone(), orchestrator_url: Some(orchestrator_url) };

    let metrics_handle = init_metrics();
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router_with_tracing(api_state)
        .merge(metrics_router)
        .layer(agentd_common::server::metrics_layer())
        .layer(agentd_common::server::cors_layer());

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Listening on {}", addr);

    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        warn!("Shutdown signal received, stopping service...");
    };

    // Background task to expire questions past their TTL.
    let cleanup_state = app_state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
            if let Err(e) = cleanup_state.expire_questions().await {
                error!("Question expiration failed: {}", e);
            }
        }
    });

    info!("agentd-ask service is ready");
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal).await?;
    info!("agentd-ask service stopped");

    Ok(())
}
