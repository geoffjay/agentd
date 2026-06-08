//! UI Service — Static file server and API reverse proxy.
//!
//! The `agentd-ui` service serves the built React SPA and proxies API
//! requests to the appropriate backend services.
//!
//! **Default port:** 17009 (dev) / 7009 (production)
//!
//! # Usage
//!
//! ```bash
//! # Start with defaults (port 17009, UI from ./ui/dist)
//! agentd-ui
//!
//! # Override via environment variables
//! AGENTD_PORT=7009 AGENTD_UI_DIR=/path/to/ui/dist agentd-ui
//! ```
//!
//! # Environment Variables
//!
//! - `AGENTD_PORT` — Port to listen on (default: 17009)
//! - `AGENTD_UI_DIR` — Path to built UI assets (default: `./ui/dist`)
//! - `AGENTD_ASK_SERVICE_URL` — Ask service URL (default: `http://localhost:7001`)
//! - `AGENTD_NOTIFY_SERVICE_URL` — Notify service URL (default: `http://localhost:7004`)
//! - `AGENTD_ORCHESTRATOR_SERVICE_URL` — Orchestrator service URL (default: `http://localhost:7006`)
//! - `RUST_LOG` — Logging level (default: info)

pub mod config;
pub mod proxy;

use anyhow::Result;
use axum::routing::{any, get};
use axum::Router;
use proxy::ProxyState;
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{info, warn};

/// Run the UI service with the given configuration.
pub async fn run(config: config::UiConfig) -> Result<()> {
    let port = config.port;
    let ui_dir = config.ui_dir.clone();

    info!(port, ui_dir = %ui_dir, "Starting agentd-ui service");

    let ui_path = Path::new(&ui_dir);
    if !ui_path.exists() {
        warn!("UI directory {} does not exist — static file serving will return 404s", ui_dir);
    }

    let proxy_state = ProxyState {
        client: reqwest::Client::new(),
        ask_url: config.ask_service_url,
        notify_url: config.notify_service_url,
        orchestrator_url: config.orchestrator_service_url,
    };

    // SPA fallback: serve index.html for any path that doesn't match a file
    let index_path = ui_path.join("index.html");
    let serve_dir = ServeDir::new(&ui_dir).fallback(ServeFile::new(&index_path));

    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        // API proxy routes — use `any` to support all HTTP methods
        .route("/api/ask/{*path}", any(proxy::proxy_ask))
        .route("/api/notify/{*path}", any(proxy::proxy_notify))
        .route("/api/orchestrator/{*path}", any(proxy::proxy_orchestrator))
        .with_state(proxy_state)
        // Static files with SPA fallback (must be last)
        .fallback_service(serve_dir)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let host = std::env::var("AGENTD_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("HTTP server listening on http://{}", addr);

    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        warn!("Shutdown signal received, stopping service...");
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

    info!("agentd-ui service stopped");
    Ok(())
}
