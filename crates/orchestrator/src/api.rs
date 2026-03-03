use crate::manager::AgentManager;
use crate::scheduler::api::{workflow_routes, WorkflowState};
use crate::scheduler::Scheduler;
use crate::types::*;
use crate::websocket::{
    ws_handler, ws_stream_agent_handler, ws_stream_all_handler, ConnectionRegistry,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiState {
    pub manager: Arc<AgentManager>,
    pub registry: ConnectionRegistry,
    pub scheduler: Arc<Scheduler>,
}

pub fn create_router(state: ApiState) -> Router {
    // Agent SDK WebSocket (claude code connects here).
    let ws_agent_routes =
        Router::new().route("/ws/{agent_id}", get(ws_handler)).with_state(state.registry.clone());

    // Monitoring streams on a separate path to avoid route conflicts.
    let ws_stream_routes = Router::new()
        .route("/stream", get(ws_stream_all_handler))
        .route("/stream/{agent_id}", get(ws_stream_agent_handler))
        .with_state(state.registry.clone());

    let wf_state =
        WorkflowState { scheduler: state.scheduler.clone(), manager: state.manager.clone() };
    let wf_routes = workflow_routes(wf_state);

    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/{id}", get(get_agent).delete(terminate_agent))
        .route("/agents/{id}/message", post(send_message))
        .route("/agents/{id}/policy", get(get_agent_policy).put(update_agent_policy))
        .with_state(state);

    api_routes.merge(ws_agent_routes).merge(ws_stream_routes).merge(wf_routes)
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    let active = state.manager.registry().connected_count().await;
    metrics::gauge!("websocket_connections_active").set(active as f64);
    Json(HealthResponse { status: "ok".to_string(), agents_active: active })
}

async fn list_agents(
    State(state): State<ApiState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let status_filter = query
        .status
        .as_deref()
        .map(|s| s.parse::<AgentStatus>())
        .transpose()
        .map_err(|e| ApiError::InvalidInput(e.to_string()))?;

    let limit = clamp_limit(query.limit);
    let offset = query.offset.unwrap_or(0);

    let (agents, total) = state.manager.list_agents_paginated(status_filter, limit, offset).await?;
    let items: Vec<AgentResponse> = agents.into_iter().map(AgentResponse::from).collect();

    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

async fn create_agent(
    State(state): State<ApiState>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let config = AgentConfig {
        working_dir: req.working_dir,
        user: req.user,
        shell: req.shell,
        interactive: req.interactive,
        prompt: req.prompt,
        worktree: req.worktree,
        system_prompt: req.system_prompt,
        tool_policy: req.tool_policy,
    };

    let agent = state.manager.spawn_agent(req.name, config).await?;

    metrics::counter!("agents_created_total").increment(1);

    Ok((StatusCode::CREATED, Json(AgentResponse::from(agent))))
}

async fn get_agent(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    Ok(Json(AgentResponse::from(agent)))
}

async fn terminate_agent(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let agent = state.manager.terminate_agent(&id).await?;

    Ok(Json(AgentResponse::from(agent)))
}

/// Send a message (prompt) to a running non-interactive agent.
async fn send_message(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SendMessageRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists and is running.
    let agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;
    if agent.status != AgentStatus::Running {
        return Err(ApiError::AgentNotRunning(format!(
            "Agent {} is not running (status: {})",
            id, agent.status
        )));
    }

    state
        .registry
        .send_user_message(&id, &req.content)
        .await
        .map_err(|e| ApiError::InvalidInput(e.to_string()))?;

    Ok(Json(serde_json::json!({ "status": "sent", "agent_id": id })))
}

/// Get the tool policy for a specific agent.
async fn get_agent_policy(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;
    Ok(Json(agent.config.tool_policy))
}

/// Update the tool policy for a specific agent.
async fn update_agent_policy(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(policy): Json<ToolPolicy>,
) -> Result<impl IntoResponse, ApiError> {
    let mut agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    agent.config.tool_policy = policy.clone();
    agent.updated_at = chrono::Utc::now();

    // Update in database.
    state.manager.update_agent(&agent).await?;

    // Update in the live WebSocket registry.
    state.registry.set_policy(id, policy.clone()).await;

    info!(agent_id = %id, ?policy, "Agent tool policy updated");

    Ok(Json(policy))
}

// -- Error handling --

/// API error types for the orchestrator service.
///
/// # HTTP Status Mapping
///
/// - `NotFound` -> 404 Not Found
/// - `InvalidInput` -> 400 Bad Request
/// - `AgentNotRunning` -> 409 Conflict
/// - `Internal` -> 500 Internal Server Error
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Resource not found (HTTP 404)
    #[error("not found")]
    NotFound,
    /// Invalid input or request (HTTP 400)
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Agent is not in the expected state (HTTP 409)
    #[error("agent not running: {0}")]
    AgentNotRunning(String),
    /// Internal server error (HTTP 500)
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::InvalidInput(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::AgentNotRunning(_) => (StatusCode::CONFLICT, self.to_string()),
            ApiError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
