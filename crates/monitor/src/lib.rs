//! Monitor Service - System health monitoring and alerting daemon.
//!
//! The `agentd-monitor` service provides a REST API for collecting and querying
//! system health metrics including CPU, memory, disk, and load average. It supports
//! configurable thresholds for anomaly detection and optional AI-powered log analysis
//! via the BAML integration.
//!
//! # Features
//!
//! - **System Metrics**: CPU usage, memory usage, disk usage, load averages
//! - **Metrics History**: Configurable in-memory ring buffer of metric snapshots
//! - **Prometheus Export**: `/metrics` endpoint in Prometheus text format
//! - **REST API**: JSON endpoints for health checks and metrics collection
//! - **Configurable Thresholds**: Alert when CPU/memory/disk exceed thresholds
//!
//! # REST API Endpoints
//!
//! ## GET /health
//!
//! Health check endpoint that returns service status.
//!
//! ```bash
//! curl http://localhost:17003/health
//! ```
//!
//! ## GET /metrics
//!
//! Returns the latest system metrics snapshot as JSON.
//!
//! ```bash
//! curl http://localhost:17003/metrics
//! ```
//!
//! ## POST /collect
//!
//! Triggers an immediate metrics collection and returns the snapshot.
//!
//! ```bash
//! curl -X POST http://localhost:17003/collect
//! ```
//!
//! ## GET /history
//!
//! Returns all collected metrics snapshots (up to the configured history size).
//!
//! ```bash
//! curl http://localhost:17003/history
//! ```
//!
//! ## GET /status
//!
//! Returns a health assessment based on configured thresholds.
//!
//! ```bash
//! curl http://localhost:17003/status
//! ```
//!
//! # Environment Variables
//!
//! - `AGENTD_PORT` - Port to listen on (default: 17003 dev, 7003 production)
//! - `AGENTD_COLLECTION_INTERVAL_SECS` - Seconds between auto-collections (default: 30)
//! - `AGENTD_CPU_ALERT_THRESHOLD` - CPU usage % to trigger alert (default: 90.0)
//! - `AGENTD_MEMORY_ALERT_THRESHOLD` - Memory usage % to trigger alert (default: 90.0)
//! - `AGENTD_DISK_ALERT_THRESHOLD` - Disk usage % to trigger alert (default: 90.0)
//! - `AGENTD_HISTORY_SIZE` - Number of metric snapshots to retain (default: 120)
//! - `RUST_LOG` - Logging level (default: info)

pub mod api;
pub mod client;
pub mod config;
pub mod error;
pub mod metrics_collector;
pub mod state;
pub mod types;

use anyhow::Result;
use std::net::SocketAddr;
use tracing::{info, warn};

/// Run the monitor service with the given configuration.
///
/// This is the main entry point for the monitor daemon. It sets up the API
/// server, starts the background metrics collection task, and blocks until
/// a shutdown signal is received.
pub async fn run(config: config::MonitorConfig) -> Result<()> {
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

    let app_state = state::AppState::new(config);
    let api_state = api::ApiState { app_state: app_state.clone() };
    let router = api::create_router_with_tracing(api_state);

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
