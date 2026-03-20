use crate::manager::AgentManager;
use crate::scheduler::api::{webhook_routes, workflow_routes, WorkflowState};
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
use communicate::client::CommunicateClient;
use communicate::error::CommunicateError;
use communicate::types::{
    AddParticipantRequest, CreateMessageRequest, CreateRoomRequest, ParticipantKind,
    ParticipantRole, RoomType,
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
    pub communicate: CommunicateClient,
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
    let wf_routes = workflow_routes(wf_state.clone());
    let wh_routes = webhook_routes(wf_state);

    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/{id}", get(get_agent).delete(terminate_agent))
        .route("/agents/{id}/message", post(send_message))
        .route("/agents/{id}/model", axum::routing::put(set_agent_model))
        .route("/agents/{id}/policy", get(get_agent_policy).put(update_agent_policy))
        .route("/agents/{id}/dirs", post(add_agent_dir).delete(remove_agent_dir))
        .route("/agents/{id}/usage", get(get_agent_usage))
        .route("/agents/{id}/clear-context", post(clear_agent_context))
        .route("/agents/{id}/approvals", get(list_agent_approvals))
        .route("/agents/{id}/rooms", get(list_agent_rooms).post(join_agent_room))
        .route("/agents/{id}/rooms/{room_id}", axum::routing::delete(leave_agent_room))
        .route(
            "/agents/{id}/rooms/{room_id}/messages",
            get(get_agent_room_messages).post(send_agent_room_message),
        )
        .route("/approvals", get(list_all_approvals))
        .route("/approvals/{id}", get(get_approval))
        .route("/approvals/{id}/approve", post(approve_tool))
        .route("/approvals/{id}/deny", post(deny_tool))
        .route("/debug/agents", get(debug_agents))
        .with_state(state);

    api_routes.merge(ws_agent_routes).merge(ws_stream_routes).merge(wf_routes).merge(wh_routes)
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
    let mut items: Vec<AgentResponse> = Vec::with_capacity(agents.len());
    for agent in agents {
        let id = agent.id;
        let mut response = AgentResponse::from(agent);
        response.activity = state.registry.get_activity_state(&id).await;
        items.push(response);
    }

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
        rooms: req.rooms,
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
    let mut response = AgentResponse::from(agent);
    response.activity = state.registry.get_activity_state(&id).await;

    Ok(Json(response))
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

/// Add a directory to the agent's `additional_dirs` list.
///
/// Returns 404 if the agent does not exist, 422 if the path is not a directory.
/// The operation is idempotent — adding an already-present path is a no-op.
/// Changes take effect on the next agent restart.
async fn add_agent_dir(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddDirRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    // Validate that the path is a directory.
    if !std::path::Path::new(&req.path).is_dir() {
        return Err(ApiError::InvalidInput(format!(
            "Path is not a directory or does not exist: {}",
            req.path
        )));
    }

    // Canonicalize the path, falling back to the original if it fails.
    let canonical = std::fs::canonicalize(&req.path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| req.path.clone());

    // Idempotent add.
    if !agent.config.additional_dirs.contains(&canonical) {
        agent.config.additional_dirs.push(canonical);
    }

    state
        .manager
        .update_additional_dirs(&id, &agent.config.additional_dirs)
        .await
        .map_err(ApiError::Internal)?;

    info!(agent_id = %id, path = %req.path, "Directory added to agent");

    Ok(Json(AddDirResponse {
        agent_id: id,
        additional_dirs: agent.config.additional_dirs,
        requires_restart: true,
    }))
}

/// Remove a directory from the agent's `additional_dirs` list.
///
/// Returns 404 if the agent does not exist. The operation is idempotent —
/// removing a path that is not in the list is a no-op.
/// Changes take effect on the next agent restart.
async fn remove_agent_dir(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddDirRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    // Canonicalize the path for consistent comparison, falling back to original.
    let canonical = std::fs::canonicalize(&req.path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| req.path.clone());

    // Idempotent remove — also try the raw path in case it was stored non-canonical.
    agent.config.additional_dirs.retain(|d| d != &canonical && d != &req.path);

    state
        .manager
        .update_additional_dirs(&id, &agent.config.additional_dirs)
        .await
        .map_err(ApiError::Internal)?;

    info!(agent_id = %id, path = %req.path, "Directory removed from agent");

    Ok(Json(AddDirResponse {
        agent_id: id,
        additional_dirs: agent.config.additional_dirs,
        requires_restart: true,
    }))
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

// -- Room management endpoints --

/// Request body for joining (or creating) a room.
#[derive(Deserialize)]
struct JoinRoomRequest {
    /// Room name — looked up first; created if it does not exist.
    room_name: Option<String>,
    /// Room UUID — used directly when provided (takes priority over `room_name`).
    room_id: Option<Uuid>,
}

/// Request body for sending a message to a room as an agent.
#[derive(Deserialize)]
struct SendRoomMessageRequest {
    /// Message content.
    content: String,
    /// Optional ID of the message being replied to.
    reply_to: Option<Uuid>,
}

/// Query parameters for listing room messages.
#[derive(Deserialize)]
struct RoomMessagesQuery {
    /// Maximum number of messages to return (default: 20, max: 100).
    limit: Option<usize>,
    /// RFC3339 timestamp cursor — return only messages before this time.
    before: Option<String>,
}

/// Convert a [`CommunicateError`] into an [`ApiError`].
///
/// | `CommunicateError` variant | HTTP status |
/// |---|---|
/// | `Conflict`                  | 409 Conflict |
/// | `NotFound`                  | 404 Not Found |
/// | `Other` (connection refused / transport) | 503 Service Unavailable |
/// | `Other` (anything else)     | 500 Internal Server Error |
///
/// The transport-error heuristics (`"Failed to GET"` / `"connection refused"`)
/// match the exact context strings added by [`CommunicateClient`]'s internal
/// helpers before the TCP/HTTP send, distinguishing them from application-level
/// error messages (which start with `"GET {url} failed with status …"`).
fn communicate_error(e: CommunicateError) -> ApiError {
    match e {
        CommunicateError::Conflict => {
            ApiError::Conflict("resource already exists in communicate service".to_string())
        }
        CommunicateError::NotFound => ApiError::NotFound,
        CommunicateError::Other(inner) => {
            let msg = inner.to_string();
            if msg.contains("connection refused")
                || msg.contains("os error 61")
                || msg.contains("Failed to GET")
                || msg.contains("Failed to POST")
                || msg.contains("Failed to DELETE")
            {
                ApiError::ServiceUnavailable(
                    "communicate service is unavailable — ensure it is running".to_string(),
                )
            } else {
                ApiError::Internal(inner)
            }
        }
    }
}

/// Check that `agent_id` is a participant of `room_id`.
///
/// Returns `ApiError::Forbidden` when the agent is not in the room,
/// or the appropriate service/internal error on failure.
async fn assert_agent_in_room(
    communicate: &CommunicateClient,
    agent_id: &Uuid,
    room_id: Uuid,
) -> Result<(), ApiError> {
    let rooms = communicate
        .get_rooms_for_participant(&agent_id.to_string())
        .await
        .map_err(|e| communicate_error(e.into()))?;

    if rooms.iter().any(|r| r.id == room_id) {
        Ok(())
    } else {
        Err(ApiError::Forbidden(format!("agent {} is not a member of room {}", agent_id, room_id)))
    }
}

/// `GET /agents/{id}/rooms` — list all rooms the agent is a member of.
///
/// Returns a [`PaginatedResponse`] wrapper consistent with other list endpoints.
/// The communicate client fetches up to 500 rooms; `total` reflects the actual
/// count returned.
async fn list_agent_rooms(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists.
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    let rooms = state
        .communicate
        .get_rooms_for_participant(&id.to_string())
        .await
        .map_err(|e| communicate_error(e.into()))?;

    let total = rooms.len();
    Ok(Json(PaginatedResponse { items: rooms, total, limit: total, offset: 0 }))
}

/// `POST /agents/{id}/rooms` — join (or create and join) a room.
///
/// Accepts either `room_id` (UUID of an existing room) or `room_name`
/// (find-or-create semantics). `room_id` takes priority when both are given.
/// Adding an agent that is already a participant is treated as success (idempotent).
async fn join_agent_room(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<JoinRoomRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists.
    let agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    // Resolve the room.
    let room = match req.room_id {
        Some(room_id) => state
            .communicate
            .get_room(room_id)
            .await
            .map_err(|e| communicate_error(e.into()))?
            .ok_or(ApiError::NotFound)?,
        None => {
            let name = req.room_name.ok_or_else(|| {
                ApiError::InvalidInput("either room_id or room_name must be provided".to_string())
            })?;

            match state
                .communicate
                .get_room_by_name(&name)
                .await
                .map_err(|e| communicate_error(e.into()))?
            {
                Some(r) => r,
                None => state
                    .communicate
                    .create_room(&CreateRoomRequest {
                        name: name.clone(),
                        topic: None,
                        description: None,
                        room_type: RoomType::Group,
                        created_by: agent.name.clone(),
                    })
                    .await
                    .map_err(|e| communicate_error(e.into()))?,
            }
        }
    };

    // Add the agent as a Member participant — 409 Conflict is treated as success
    // (the agent is already in the room).
    let result = state
        .communicate
        .add_participant(
            room.id,
            &AddParticipantRequest {
                identifier: id.to_string(),
                kind: ParticipantKind::Agent,
                display_name: agent.name.clone(),
                role: ParticipantRole::Member,
            },
        )
        .await;

    match result {
        Ok(participant) => {
            info!(agent_id = %id, room_id = %room.id, room_name = %room.name, "Agent joined room via API");
            Ok((
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "room": room,
                    "participant": participant,
                })),
            ))
        }
        Err(CommunicateError::Conflict) => {
            // Already a member — idempotent success.
            info!(agent_id = %id, room_id = %room.id, "Agent already in room (join idempotent)");
            Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "room": room,
                    "participant": null,
                })),
            ))
        }
        Err(e) => Err(communicate_error(e)),
    }
}

/// `DELETE /agents/{id}/rooms/{room_id}` — remove an agent from a room.
async fn leave_agent_room(
    State(state): State<ApiState>,
    Path((id, room_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists.
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    // Verify room exists.
    state
        .communicate
        .get_room(room_id)
        .await
        .map_err(|e| communicate_error(e.into()))?
        .ok_or(ApiError::NotFound)?;

    state
        .communicate
        .remove_participant(room_id, &id.to_string())
        .await
        .map_err(communicate_error)?;

    info!(agent_id = %id, %room_id, "Agent left room via API");

    Ok(StatusCode::NO_CONTENT)
}

/// `POST /agents/{id}/rooms/{room_id}/messages` — send a message to a room as
/// the specified agent.
async fn send_agent_room_message(
    State(state): State<ApiState>,
    Path((id, room_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<SendRoomMessageRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists.
    let agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    // Verify room exists.
    state
        .communicate
        .get_room(room_id)
        .await
        .map_err(|e| communicate_error(e.into()))?
        .ok_or(ApiError::NotFound)?;

    // Verify agent is a member of the room.
    assert_agent_in_room(&state.communicate, &id, room_id).await?;

    let message = state
        .communicate
        .send_message(
            room_id,
            &CreateMessageRequest {
                sender_id: id.to_string(),
                sender_name: agent.name.clone(),
                sender_kind: ParticipantKind::Agent,
                content: req.content,
                metadata: Default::default(),
                reply_to: req.reply_to,
            },
        )
        .await
        .map_err(|e| communicate_error(e.into()))?;

    info!(agent_id = %id, %room_id, message_id = %message.id, "Agent sent room message via API");

    Ok((StatusCode::CREATED, Json(message)))
}

/// `GET /agents/{id}/rooms/{room_id}/messages` — get recent messages from a
/// room the agent is a member of.
async fn get_agent_room_messages(
    State(state): State<ApiState>,
    Path((id, room_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<RoomMessagesQuery>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists.
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    // Verify room exists.
    state
        .communicate
        .get_room(room_id)
        .await
        .map_err(|e| communicate_error(e.into()))?
        .ok_or(ApiError::NotFound)?;

    // Verify agent is a member of the room.
    assert_agent_in_room(&state.communicate, &id, room_id).await?;

    let limit = query.limit.unwrap_or(20).min(100);

    let messages = if let Some(before_str) = query.before {
        let before: chrono::DateTime<chrono::Utc> = before_str
            .parse()
            .map_err(|_| ApiError::InvalidInput("invalid 'before' timestamp".to_string()))?;
        state
            .communicate
            .list_messages(room_id, limit, Some(before))
            .await
            .map_err(|e| communicate_error(e.into()))?
    } else {
        state
            .communicate
            .get_latest_messages(room_id, limit)
            .await
            .map_err(|e| communicate_error(e.into()))?
    };

    Ok(Json(messages))
}

// -- Error handling --

// Re-export shared ApiError from agentd-common.
pub use agentd_common::error::ApiError;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    /// Idempotent add: inserting the same path twice should not duplicate it.
    #[test]
    fn test_add_dir_idempotent() {
        let mut dirs: Vec<String> = vec!["/tmp".to_string()];
        let path = "/tmp".to_string();
        if !dirs.contains(&path) {
            dirs.push(path);
        }
        assert_eq!(dirs.len(), 1);
    }

    /// Removing a path that is present should leave it gone.
    #[test]
    fn test_remove_dir_present() {
        let mut dirs: Vec<String> = vec!["/tmp".to_string(), "/var".to_string()];
        let path = "/tmp".to_string();
        dirs.retain(|d| d != &path);
        assert_eq!(dirs, vec!["/var".to_string()]);
    }

    /// Removing a path that is absent is a no-op (idempotent).
    #[test]
    fn test_remove_dir_absent_is_noop() {
        let mut dirs: Vec<String> = vec!["/var".to_string()];
        let path = "/tmp".to_string();
        let original_len = dirs.len();
        dirs.retain(|d| d != &path);
        assert_eq!(dirs.len(), original_len);
    }

    /// Non-existent path should fail the is_dir() check.
    #[test]
    fn test_path_validation_nonexistent() {
        let path = "/definitely/does/not/exist/agentd-test-12345";
        assert!(!std::path::Path::new(path).is_dir());
    }

    /// A known existing directory should pass the is_dir() check.
    #[test]
    fn test_path_validation_existing_dir() {
        assert!(std::path::Path::new("/tmp").is_dir());
    }
}
