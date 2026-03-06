//! agentd-hook — Shell and git hook integration daemon.
//!
//! Receives hook events from shell and git integrations and creates
//! notifications when user intervention may be required.
//!
//! **Default port:** 17002 (dev) / 7002 (production)

use anyhow::Result;
use hook::api::{ApiState, create_router_with_tracing};
use std::net::SocketAddr;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing. Set LOG_FORMAT=json for structured JSON output.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if std::env::var("LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt().json().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().with_target(false).with_env_filter(env_filter).init();
    }

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(17002);

    info!(port, "Starting agentd-hook daemon");

    let api_state = ApiState::default();
    let router = create_router_with_tracing(api_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP server listening on http://{}", addr);

    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        warn!("Shutdown signal received, stopping daemon...");
    };

    tokio::select! {
        result = axum::serve(listener, router) => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = shutdown_signal => {
            info!("Graceful shutdown initiated");
        }
    }

    info!("agentd-hook daemon stopped");
    Ok(())
}
