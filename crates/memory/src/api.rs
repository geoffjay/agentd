//! REST API handlers for the memory service.
//!
//! This module provides HTTP endpoints for the agentd-memory service. Currently
//! only the `/health` endpoint is implemented as part of the initial scaffold.
//! Storage, semantic search, and full CRUD endpoints will be added in subsequent
//! issues.
//!
//! # API Endpoints
//!
//! - `GET /health` - Health check endpoint
//!
//! # Examples
//!
//! ## Creating a Router
//!
//! ```no_run
//! use memory::api::{create_router, ApiState};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let state = ApiState {};
//!     let router = create_router(state);
//!
//!     // Bind and serve
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:17008").await?;
//!     axum::serve(listener, router).await?;
//!     Ok(())
//! }
//! ```

use axum::{response::IntoResponse, Json, Router};

/// Shared state passed to all API handlers.
///
/// Will be extended with storage backends (SQLite via SeaORM for metadata,
/// LanceDB for vector embeddings) in subsequent issues.
#[derive(Clone)]
pub struct ApiState {}

/// Creates and configures the Axum router with all API endpoints.
///
/// # Arguments
///
/// * `state` - The API state (currently empty; will hold storage in future issues)
///
/// # Returns
///
/// Returns a configured [`Router`] ready to serve HTTP requests.
pub fn create_router(state: ApiState) -> Router {
    Router::new().route("/health", axum::routing::get(health_check)).with_state(state)
}

/// Health check endpoint handler.
///
/// Returns basic service information including status and version.
///
/// # Endpoint
///
/// `GET /health`
///
/// # Response
///
/// Returns HTTP 200 with JSON body:
/// ```json
/// {
///   "status": "ok",
///   "service": "agentd-memory",
///   "version": "0.2.0"
/// }
/// ```
///
/// # Examples
///
/// ```bash
/// curl http://localhost:17008/health
/// ```
async fn health_check() -> impl IntoResponse {
    Json(agentd_common::types::HealthResponse::ok("agentd-memory", env!("CARGO_PKG_VERSION")))
}
