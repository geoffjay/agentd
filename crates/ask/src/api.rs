//! REST API stub for the ask service.
//!
//! The check/trigger system has been removed. The full Q&A API
//! (POST /questions, POST /questions/{id}/answer, etc.) is implemented
//! in issue #922.

use crate::{error::ApiError, state::AppState, types::HealthResponse};
use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use tower_http::trace::TraceLayer;

/// Shared state for API handlers.
#[derive(Clone)]
pub struct ApiState {
    pub app_state: AppState,
}

/// Health check handler.
async fn health_handler(State(state): State<ApiState>) -> Result<impl IntoResponse, ApiError> {
    let _ = state;
    Ok(Json(HealthResponse::ok("agentd-ask", env!("CARGO_PKG_VERSION"))))
}

/// Creates the Axum router with tracing middleware.
pub fn create_router_with_tracing(api_state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .with_state(api_state)
        .layer(TraceLayer::new_for_http())
}
