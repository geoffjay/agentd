//! Hook Service - Shell and git hook integration daemon.
//!
//! The `agentd-hook` service receives shell and git hook events and creates
//! notifications in the notify service when user intervention is required.
//!
//! # Features
//!
//! - **Hook Events**: Receive and record shell/git hook events via REST
//! - **Shell Integration**: Generate shell scripts for zsh, bash, and fish
//! - **Notification Integration**: Forward important events to the notify service
//! - **REST API**: Simple HTTP endpoints for health, events, and shell integration
//!
//! # REST API Endpoints
//!
//! ## GET /health
//!
//! Health check endpoint.
//!
//! ```bash
//! curl http://localhost:17002/health
//! ```
//!
//! ## POST /events
//!
//! Receive a hook event (shell command completion, git hook, etc.).
//!
//! ```bash
//! curl -X POST http://localhost:17002/events \
//!   -H "Content-Type: application/json" \
//!   -d '{"kind":"shell","command":"cargo build","exit_code":0,"duration_ms":1200}'
//! ```
//!
//! ## GET /shell/:shell
//!
//! Generate a shell integration script.
//!
//! ```bash
//! curl http://localhost:17002/shell/zsh
//! ```
//!
//! # Environment Variables
//!
//! - `AGENTD_PORT` - Port to listen on (default: 17002 dev, 7002 production)
//! - `AGENTD_HISTORY_SIZE` - Number of events to retain (default: 500)
//! - `AGENTD_NOTIFY_ON_FAILURE` - Notify on command failure (default: true)
//! - `AGENTD_NOTIFY_ON_LONG_RUNNING` - Notify on long-running commands (default: true)
//! - `AGENTD_LONG_RUNNING_THRESHOLD_MS` - Threshold in ms (default: 30000)
//! - `RUST_LOG` - Logging level (default: info)

pub mod api;
pub mod client;
pub mod config;
pub mod error;
pub mod shell;
pub mod state;
pub mod types;

use anyhow::Result;
use api::{create_router_with_tracing, ApiState};
use std::net::SocketAddr;
use tracing::{info, warn};

/// Run the hook service with the given configuration.
///
/// Binds to `0.0.0.0:<config.port>`, serves the REST API, and shuts down
/// gracefully on CTRL+C.
pub async fn run(config: config::HookConfig) -> Result<()> {
    let port = config.port;
    info!(port, "Starting agentd-hook daemon");

    let api_state = ApiState::new(config);
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
