//! Ask service entry point and initialization.
//!
//! This module contains the main function that initializes and runs the ask service.
//! It handles service startup, configuration, health checks, and graceful shutdown.
//!
//! # Environment Variables
//!
//! The following environment variables configure the service:
//!
//! - `PORT` - The port to bind to (default: 17001 dev, 7001 production)
//! - `NOTIFY_SERVICE_URL` - Base URL of the notification service (default: http://localhost:3000)
//! - `RUST_LOG` - Logging configuration (default: info)
//!
//! # Service Lifecycle
//!
//! 1. **Initialization**: Sets up logging, loads configuration, checks tmux availability
//! 2. **State Creation**: Initializes application state and notification client
//! 3. **Health Check**: Verifies notification service connectivity
//! 4. **Server Start**: Binds to configured port and starts accepting requests
//! 5. **Background Tasks**: Spawns cleanup task for old questions (runs hourly)
//! 6. **Graceful Shutdown**: Listens for CTRL+C and shuts down cleanly
//!
//! # Examples
//!
//! ## Running with default configuration
//!
//! ```bash
//! cargo run
//! # Starts on port 3001, connects to notification service at http://localhost:3000
//! ```
//!
//! ## Running with custom configuration
//!
//! ```bash
//! PORT=8080 NOTIFY_SERVICE_URL=http://notify:7004 cargo run
//! ```
//!
//! ## With debug logging
//!
//! ```bash
//! RUST_LOG=debug cargo run
//! ```

mod api;
mod error;
mod notification_client;
mod state;
mod tmux_check;
mod types;

use anyhow::Result;
use api::{create_router_with_tracing, ApiState};
use notification_client::NotificationClient;
use state::AppState;
use std::env;
use tracing::{error, info, warn};

/// Service entry point.
///
/// Initializes the ask service, sets up all components, and runs the HTTP server
/// with graceful shutdown support.
///
/// # Returns
///
/// Returns `Ok(())` if the service starts and stops successfully, or an error
/// if initialization or runtime fails.
///
/// # Errors
///
/// Returns an error if:
/// - Failed to bind to the configured port
/// - Invalid port configuration
/// - Failed to start the HTTP server
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting agentd-ask service...");

    // Get configuration from environment
    let port =
        env::var("PORT").unwrap_or_else(|_| "17001".to_string()).parse::<u16>().unwrap_or(17001);

    let notify_service_url =
        env::var("NOTIFY_SERVICE_URL").unwrap_or_else(|_| "http://localhost:7004".to_string());

    info!("Configuration:");
    info!("  Port: {}", port);
    info!("  Notification service: {}", notify_service_url);

    // Check if tmux is installed
    if !tmux_check::is_tmux_installed() {
        warn!("tmux is not installed - tmux checks will fail");
    } else {
        info!("tmux is installed");
    }

    // Initialize application state
    let app_state = AppState::new();

    // Initialize notification client
    let notification_client = NotificationClient::new(notify_service_url.clone());

    // Check notification service health
    match notification_client.health_check().await {
        Ok(true) => info!("Notification service is healthy"),
        Ok(false) => warn!("Notification service returned unhealthy status"),
        Err(e) => {
            error!("Failed to connect to notification service: {}", e);
            warn!("Service will continue, but notifications may fail");
        }
    }

    // Create API state
    let api_state = ApiState {
        app_state: app_state.clone(),
        notification_client,
        notification_service_url: notify_service_url,
    };

    // Create router with tracing middleware
    let app = create_router_with_tracing(api_state);

    // Bind to address
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Listening on {}", addr);

    // Set up graceful shutdown signal handler
    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        warn!("Shutdown signal received, stopping service...");
    };

    // Start background task to clean up old questions
    let cleanup_state = app_state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            info!("Running cleanup of old questions");
            cleanup_state.cleanup_old_questions().await;
        }
    });

    // Run the server with graceful shutdown
    info!("agentd-ask service is ready");
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal).await?;

    info!("agentd-ask service stopped");
    Ok(())
}
