//! REST API handlers for the memory service.
//!
//! This module provides HTTP endpoints for the agentd-memory service. Currently
//! only the `/health` endpoint is implemented; full CRUD and search endpoints
//! will be added in subsequent issues.
//!
//! # API Endpoints
//!
//! - `GET /health` — Health check

use axum::{response::IntoResponse, Json, Router};

use crate::storage::MemoryStorage;

/// Shared state passed to all API handlers.
#[derive(Clone)]
pub struct ApiState {
    /// SQLite-backed metadata storage.
    pub storage: MemoryStorage,
}

/// Create and configure the Axum router.
pub fn create_router(state: ApiState) -> Router {
    Router::new().route("/health", axum::routing::get(health_check)).with_state(state)
}

/// `GET /health` — service liveness check.
async fn health_check() -> impl IntoResponse {
    Json(agentd_common::types::HealthResponse::ok("agentd-memory", env!("CARGO_PKG_VERSION")))
}
