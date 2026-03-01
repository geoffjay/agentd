use crate::manager::AgentManager;
use crate::scheduler::api::{workflow_routes, WorkflowState};
use crate::scheduler::Scheduler;
use crate::types::*;
use crate::websocket::{ws_handler, ConnectionRegistry};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiState {
    pub manager: Arc<AgentManager>,
    pub registry: ConnectionRegistry,
    pub scheduler: Arc<Scheduler>,
}

pub fn create_router(state: ApiState) -> Router {
    let ws_routes =
        Router::new().route("/ws/{agent_id}", get(ws_handler)).with_state(state.registry.clone());

    let wf_state = WorkflowState {
        scheduler: state.scheduler.clone(),
        manager: state.manager.clone(),
    };
    let wf_routes = workflow_routes(wf_state);

    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/{id}", get(get_agent).delete(terminate_agent))
        .with_state(state);

    api_routes.merge(ws_routes).merge(wf_routes)
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
}

async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    let active = state.manager.registry().connected_count().await;
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

    let agents = state.manager.list_agents(status_filter).await?;
    let responses: Vec<AgentResponse> = agents.into_iter().map(AgentResponse::from).collect();

    Ok(Json(responses))
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
    };

    let agent = state.manager.spawn_agent(req.name, config).await?;

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

// -- Error handling --

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Not found")]
    NotFound,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::InvalidInput(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
