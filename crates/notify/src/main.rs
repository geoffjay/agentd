//! agentd-notify service entry point.
//!
//! This is the main executable for the notification service. It initializes
//! the storage backend, sets up the REST API server, and starts a background
//! task to clean up expired notifications.
//!
//! # Features
//!
//! - SQLite-based persistent notification storage
//! - REST API on `http://127.0.0.1:17004` (dev default)
//! - Automatic cleanup of expired notifications every 5 minutes
//! - Structured logging with tracing
//! - Graceful shutdown support
//!
//! # Running the Service
//!
//! ```bash
//! # Run with default INFO logging
//! cargo run -p agentd-notify
//!
//! # Run with DEBUG logging
//! RUST_LOG=debug cargo run -p agentd-notify
//!
//! # Run the release build
//! cargo run -p agentd-notify --release
//! ```
//!
//! # Environment Variables
//!
//! - `RUST_LOG` - Controls logging level (e.g., `debug`, `info`, `warn`, `error`)
//!   Defaults to `info` if not set.
//!
//! # API Endpoints
//!
//! Once running, the service exposes the following endpoints:
//!
//! - `GET /health` - Health check
//! - `GET /notifications` - List all notifications
//! - `POST /notifications` - Create a notification
//! - `GET /notifications/:id` - Get a specific notification
//! - `PUT /notifications/:id` - Update a notification
//! - `DELETE /notifications/:id` - Delete a notification
//! - `GET /notifications/actionable` - List actionable notifications
//! - `GET /notifications/history` - List notification history
//!
//! # Database Location
//!
//! The SQLite database is stored at a platform-specific location:
//! - Linux: `~/.local/share/agentd-notify/notify.db`
//! - macOS: `~/Library/Application Support/agentd-notify/notify.db`
//! - Windows: `C:\Users\<user>\AppData\Local\agentd-notify\notify.db`
//!
//! # Examples
//!
//! ```bash
//! # Start the service
//! cargo run -p agentd-notify
//!
//! # In another terminal, test the API
//! curl http://localhost:17004/health
//! ```

mod api;
mod entity;
mod migration;
mod notification;
mod storage;
mod types;

use api::{create_router, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::env;
use std::sync::Arc;
use storage::NotificationStorage;
use tokio::time::{interval, Duration};

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info", "version" => env!("CARGO_PKG_VERSION"), "service" => "notify")
        .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}
use tracing::{info, warn};

/// Main entry point for the agentd-notify service.
///
/// This function performs the following initialization steps:
/// 1. Sets up structured logging with tracing
/// 2. Initializes the SQLite storage backend
/// 3. Spawns a background task for cleaning up expired notifications
/// 4. Creates and configures the Axum HTTP router
/// 5. Starts the HTTP server on `127.0.0.1:17004` (dev default)
///
/// # Returns
///
/// Returns `Ok(())` on successful shutdown, or an error if initialization
/// or server startup fails.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to initialize the database
/// - Unable to bind to the network address
/// - The HTTP server encounters a fatal error
///
/// # Panics
///
/// Does not panic under normal operation.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-notify service...");

    // Initialize SQLite storage
    let storage = NotificationStorage::new().await?;
    info!("Notification storage initialized at: {:?}", NotificationStorage::get_db_path()?);

    // Wrap storage for sharing
    let storage = Arc::new(storage);

    // Spawn background cleanup task
    let storage_clone = storage.clone();
    tokio::spawn(async move {
        let mut cleanup_interval = interval(Duration::from_secs(300)); // Every 5 minutes
        loop {
            cleanup_interval.tick().await;
            match storage_clone.cleanup_expired().await {
                Ok(count) => {
                    if count > 0 {
                        info!("Cleaned up {} expired notifications", count);
                    }
                }
                Err(e) => {
                    warn!("Failed to cleanup expired notifications: {}", e);
                }
            }
        }
    });

    // Initialize Prometheus metrics
    let metrics_handle = init_metrics();

    // Create API state and router with metrics endpoint and tracing middleware
    let api_state = ApiState { storage: storage.clone() };
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router(api_state)
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    // Bind to address (use AGENTD_PORT env var, default 17004 for dev, 7004 for production)
    let port = env::var("AGENTD_PORT").unwrap_or_else(|_| "17004".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Notification API server listening on http://{}", addr);

    // Start the HTTP server
    axum::serve(listener, app).await?;

    Ok(())
}
