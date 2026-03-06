//! agentd-monitor — System monitoring and alerting service.
//!
//! Watches system metrics (CPU, memory, disk, load average) and exposes a
//! REST API for querying current state and triggering on-demand collection.
//!
//! **Default port:** 17003 (dev) / 7003 (production)
//!
//! # Usage
//!
//! ```bash
//! # Start with defaults (port 17003, 30-second collection interval)
//! agentd-monitor
//!
//! # Override port and interval via environment variables
//! PORT=7003 COLLECTION_INTERVAL_SECS=60 agentd-monitor
//!
//! # JSON structured logging
//! LOG_FORMAT=json agentd-monitor
//! ```

use anyhow::Result;
use monitor::{
    api::{ApiState, create_router_with_tracing},
    config::MonitorConfig,
    metrics_collector,
    state::AppState,
};
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

    let config = MonitorConfig::from_env();
    let port = config.port;
    let interval_secs = config.collection_interval_secs;

    info!(
        port,
        collection_interval_secs = interval_secs,
        cpu_threshold = config.cpu_alert_threshold,
        memory_threshold = config.memory_alert_threshold,
        disk_threshold = config.disk_alert_threshold,
        "Starting agentd-monitor daemon"
    );

    let app_state = AppState::new(config);
    let api_state = ApiState { app_state: app_state.clone() };
    let router = create_router_with_tracing(api_state);

    // Background metrics collection task
    let bg_state = app_state.clone();
    let collection_task = tokio::spawn(async move {
        info!("Background metrics collector starting (interval: {}s)", interval_secs);
        loop {
            let metrics = metrics_collector::collect();
            bg_state.push_metrics(metrics).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
        }
    });

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

    collection_task.abort();
    info!("agentd-monitor daemon stopped");
    Ok(())
}
