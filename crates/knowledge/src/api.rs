//! REST API router for the agentd-knowledge service.
//!
//! Full handler implementation lives in KB-3; this stub provides the
//! health endpoint so KB-1 can build and serve `GET /health`.

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::storage::KnowledgeStorage;

/// Shared API state.
#[derive(Clone)]
pub struct ApiState {
    #[allow(dead_code)]
    pub storage: Arc<KnowledgeStorage>,
}

/// Create the Axum router (no persistent state).
#[allow(dead_code)]
pub fn create_router() -> Router {
    Router::new().route("/health", get(health_handler))
}

/// Create the Axum router with shared storage state.
pub fn create_router_with_state(storage: Arc<KnowledgeStorage>) -> Router {
    let state = ApiState { storage };
    Router::new().route("/health", get(health_handler)).with_state(state)
}

async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "agentd-knowledge" }))
}
