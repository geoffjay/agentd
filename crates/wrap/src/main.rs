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
use std::env;
use tracing::info;

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
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting agentd-wrap service...");

    // Create API router
    let app = create_router();

    // Bind to address (use PORT env var, default 17005 for dev)
    let port = env::var("PORT").unwrap_or_else(|_| "17005".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Wrap API server listening on http://{}", addr);

    // Start the HTTP server
    axum::serve(listener, app).await?;

    Ok(())
}
