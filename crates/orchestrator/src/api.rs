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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
        .route("/agents/{id}/model", axum::routing::put(set_agent_model))
        .route("/agents/{id}/policy", get(get_agent_policy).put(update_agent_policy))
        .route("/agents/{id}/usage", get(get_agent_usage))
        .route("/agents/{id}/clear-context", post(clear_agent_context))
        .route("/agents/{id}/approvals", get(list_agent_approvals))
        .route("/approvals", get(list_all_approvals))
        .route("/approvals/{id}", get(get_approval))
        .route("/approvals/{id}/approve", post(approve_tool))
        .route("/approvals/{id}/deny", post(deny_tool))
        .route("/debug/agents", get(debug_agents))
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
    Json(
        HealthResponse::ok("agentd-orchestrator", env!("CARGO_PKG_VERSION"))
            .with_detail("agents_active", serde_json::json!(active)),
    )
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
        model: req.model,
        env: req.env,
        auto_clear_threshold: req.auto_clear_threshold,
        network_policy: req.network_policy,
        docker_image: req.docker_image,
        extra_mounts: req.extra_mounts,
        resource_limits: req.resource_limits,
        additional_dirs: req.additional_dirs,
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
        return Err(ApiError::Conflict(format!(
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

/// Set or change the model for an agent.
///
/// Updates the stored model. If `restart: true`, kills and re-launches
/// the agent process with the new `--model` flag.
async fn set_agent_model(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SetModelRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let agent = state.manager.set_model(&id, req.model, req.restart).await?;

    info!(agent_id = %id, model = ?agent.config.model, restart = req.restart, "Agent model changed via API");

    Ok(Json(AgentResponse::from(agent)))
}

// -- Usage & context endpoints --

/// Get usage statistics for an agent.
async fn get_agent_usage(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists; 404 if not.
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    let stats = state.manager.get_usage_stats(&id).await?;

    Ok(Json(stats))
}

/// Clear an agent's context and start a fresh session.
async fn clear_agent_context(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists; 404 if not.
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    let response = state.manager.clear_context(&id).await?;

    info!(agent_id = %id, new_session = response.new_session_number, "Agent context cleared via API");

    Ok(Json(response))
}

// -- Tool approval endpoints --

#[derive(Deserialize)]
struct ApprovalListQuery {
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_all_approvals(
    State(state): State<ApiState>,
    Query(query): Query<ApprovalListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let status_filter = query
        .status
        .as_deref()
        .map(|s| s.parse::<ApprovalStatus>())
        .transpose()
        .map_err(|e| ApiError::InvalidInput(e.to_string()))?;

    let mut approvals = state.registry.approvals.list(None, status_filter.as_ref()).await;
    approvals.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = approvals.len();
    let limit = clamp_limit(query.limit);
    let offset = query.offset.unwrap_or(0);
    let items: Vec<PendingApproval> = approvals.into_iter().skip(offset).take(limit).collect();

    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

async fn get_approval(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let approval = state.registry.approvals.get(&id).await.ok_or(ApiError::NotFound)?;
    Ok(Json(approval))
}

async fn approve_tool(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(_req): Json<ApprovalActionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let approval = state
        .registry
        .approvals
        .resolve(&id, ApprovalDecision::Approve)
        .await
        .map_err(|e| ApiError::InvalidInput(e.to_string()))?;

    info!(approval_id = %id, agent_id = %approval.agent_id, tool = %approval.tool_name, "Tool approved via API");
    Ok(Json(approval))
}

async fn deny_tool(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(_req): Json<ApprovalActionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let approval = state
        .registry
        .approvals
        .resolve(&id, ApprovalDecision::Deny)
        .await
        .map_err(|e| ApiError::InvalidInput(e.to_string()))?;

    info!(approval_id = %id, agent_id = %approval.agent_id, tool = %approval.tool_name, "Tool denied via API");
    Ok(Json(approval))
}

async fn list_agent_approvals(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ApprovalListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    let status_filter = query
        .status
        .as_deref()
        .map(|s| s.parse::<ApprovalStatus>())
        .transpose()
        .map_err(|e| ApiError::InvalidInput(e.to_string()))?;

    let mut approvals = state.registry.approvals.list(Some(&id), status_filter.as_ref()).await;
    approvals.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = approvals.len();
    let limit = clamp_limit(query.limit);
    let offset = query.offset.unwrap_or(0);
    let items: Vec<PendingApproval> = approvals.into_iter().skip(offset).take(limit).collect();

    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

// -- Debug endpoint --

#[derive(Serialize)]
struct DebugAgentEntry {
    id: Uuid,
    name: String,
    status: AgentStatus,
    session_id: Option<String>,
    ws_connected: bool,
    model: Option<String>,
    workflows: Vec<Uuid>,
}

#[derive(Serialize)]
struct DebugResponse {
    agents: Vec<DebugAgentEntry>,
    /// Agent IDs that have a WebSocket connection but no DB record.
    orphan_connections: Vec<Uuid>,
    /// Summary counts for quick scanning.
    summary: DebugSummary,
}

#[derive(Serialize)]
struct DebugSummary {
    total_agents: usize,
    running: usize,
    ws_connected: usize,
    running_but_disconnected: Vec<Uuid>,
    connected_but_not_running: Vec<Uuid>,
    active_workflows: usize,
}

async fn debug_agents(State(state): State<ApiState>) -> Result<impl IntoResponse, ApiError> {
    let agents = state.manager.list_agents(None).await?;
    let connected_ids = state.registry.connected_ids().await;
    let running_workflows = state.scheduler.running_workflows().await;

    // Build a map of agent_id → list of running workflow IDs.
    let mut wf_map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (wf_id, agent_id) in &running_workflows {
        wf_map.entry(*agent_id).or_default().push(*wf_id);
    }

    let connected_set: std::collections::HashSet<Uuid> = connected_ids.iter().copied().collect();
    let agent_id_set: std::collections::HashSet<Uuid> = agents.iter().map(|a| a.id).collect();

    let mut running_but_disconnected = Vec::new();
    let mut connected_but_not_running = Vec::new();
    let mut running_count = 0;

    let entries: Vec<DebugAgentEntry> = agents
        .iter()
        .map(|agent| {
            let ws_connected = connected_set.contains(&agent.id);
            let is_running = agent.status == AgentStatus::Running;

            if is_running {
                running_count += 1;
            }
            if is_running && !ws_connected {
                running_but_disconnected.push(agent.id);
            }
            if ws_connected && !is_running {
                connected_but_not_running.push(agent.id);
            }

            DebugAgentEntry {
                id: agent.id,
                name: agent.name.clone(),
                status: agent.status.clone(),
                session_id: agent.session_id.clone(),
                ws_connected,
                model: agent.config.model.clone(),
                workflows: wf_map.remove(&agent.id).unwrap_or_default(),
            }
        })
        .collect();

    let orphan_connections: Vec<Uuid> =
        connected_ids.iter().filter(|id| !agent_id_set.contains(id)).copied().collect();

    let summary = DebugSummary {
        total_agents: entries.len(),
        running: running_count,
        ws_connected: connected_set.len(),
        running_but_disconnected,
        connected_but_not_running,
        active_workflows: running_workflows.len(),
    };

    Ok(Json(DebugResponse { agents: entries, orphan_connections, summary }))
}

// -- Error handling --

// Re-export shared ApiError from agentd-common.
pub use agentd_common::error::ApiError;
