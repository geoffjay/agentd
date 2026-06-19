use crate::manager::AgentManager;
use crate::scheduler::api::{queue_routes, webhook_routes, workflow_routes, WorkflowState};
use crate::scheduler::events::SystemEvent;
use crate::scheduler::types::WorkflowResponse;
use crate::scheduler::Scheduler;
use crate::types::*;
use crate::websocket::{
    ws_handler, ws_stream_agent_handler, ws_stream_agent_v2_handler, ws_stream_all_handler,
    ws_terminal_handler, ConnectionRegistry, TerminalRelayState,
};
use agentd_common::tenant::OptionalTenantId;
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
use wrap::types::{BackendInfo, BackendType};

#[derive(Clone)]
pub struct ApiState {
    pub manager: Arc<AgentManager>,
    pub registry: ConnectionRegistry,
    pub scheduler: Arc<Scheduler>,
    pub communicate: CommunicateClient,
    /// The active execution backend type — used for capability reporting.
    pub backend_type: BackendType,
}

pub fn create_router(state: ApiState) -> Router {
    // Agent SDK WebSocket (claude code connects here).
    let ws_agent_routes =
        Router::new().route("/ws/{agent_id}", get(ws_handler)).with_state(state.registry.clone());

    // Monitoring streams on a separate path to avoid route conflicts.
    // v1 (/stream, /stream/{agent_id}) is kept for one release for any
    // external consumers; first-party clients (Web UI, TUI) use v2.
    let ws_stream_routes = Router::new()
        .route("/stream", get(ws_stream_all_handler))
        .route("/stream/{agent_id}", get(ws_stream_agent_handler))
        .route("/v2/stream/{agent_id}", get(ws_stream_agent_v2_handler))
        .with_state(state.registry.clone());

    // PTY terminal relay WebSocket — binary frames of raw terminal I/O.
    let ws_terminal_routes = Router::new()
        .route("/terminal/{agent_id}", get(ws_terminal_handler))
        .with_state(TerminalRelayState { manager: state.manager.clone() });

    let wf_state =
        WorkflowState { scheduler: state.scheduler.clone(), manager: state.manager.clone() };
    let wf_routes = workflow_routes(wf_state.clone());
    let wh_routes = webhook_routes(wf_state.clone());
    let q_routes = queue_routes(wf_state);

    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/info", get(backend_info))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/system-agents", get(list_system_agents))
        .route("/agents/{id}", get(get_agent).patch(update_agent).delete(terminate_agent))
        .route("/agents/{id}/message", post(send_message))
        .route("/agents/{id}/restart", post(restart_agent))
        .route("/agents/{id}/model", axum::routing::put(set_agent_model))
        .route("/agents/{id}/policy", get(get_agent_policy).put(update_agent_policy))
        .route("/agents/{id}/dirs", post(add_agent_dir).delete(remove_agent_dir))
        .route("/agents/{id}/usage", get(get_agent_usage))
        .route("/agents/{id}/clear-context", post(clear_agent_context))
        .route("/agents/{id}/approvals", get(list_agent_approvals))
        // Conversation history (static sub-path must precede wildcard)
        .route(
            "/agents/{id}/conversation",
            get(get_agent_conversation).delete(delete_agent_conversation),
        )
        .route("/agents/{id}/conversation/summary", get(get_agent_conversation_summary))
        .route("/agents/{id}/conversation/{event_id}", get(get_agent_conversation_event))
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
        .route("/events/ask", post(ask_event_handler))
        // Project association endpoints (CRUD lives in core service)
        .route("/projects/{id}/agents", get(list_project_agents))
        .route(
            "/projects/{id}/agents/{agent_id}",
            post(associate_project_agent).delete(dissociate_project_agent),
        )
        .route("/projects/{id}/workflows", get(list_project_workflows))
        .route(
            "/projects/{id}/workflows/{workflow_id}",
            post(associate_project_workflow).delete(dissociate_project_workflow),
        )
        .with_state(state);

    api_routes
        .merge(ws_agent_routes)
        .merge(ws_stream_routes)
        .merge(ws_terminal_routes)
        .merge(wf_routes)
        .merge(wh_routes)
        .merge(q_routes)
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub project_id: Option<Uuid>,
    /// When `true`, include built-in system agents in the response.
    ///
    /// By default `GET /agents` excludes system agents -- they are listed
    /// separately via `GET /system-agents`.  Pass `?include_builtin=true`
    /// to include them (e.g., for admin or debug tooling).
    pub include_builtin: Option<bool>,
}

async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    let active = state.manager.registry().connected_count().await;
    metrics::gauge!("websocket_connections_active").set(active as f64);
    Json(
        HealthResponse::ok("agentd-orchestrator", env!("CARGO_PKG_VERSION"))
            .with_detail("agents_active", serde_json::json!(active)),
    )
}

/// `GET /info` — active backend type and capabilities.
///
/// Returns information about the execution backend currently in use, including
/// its capabilities. Clients (UI, CLI) can use this to discover which features
/// are available (e.g., PTY streaming, health checks).
async fn backend_info(State(state): State<ApiState>) -> impl IntoResponse {
    let caps = state.backend_type.capabilities();
    Json(BackendInfo {
        backend_type: state.backend_type,
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: caps,
    })
}

async fn list_agents(
    OptionalTenantId(org_id): OptionalTenantId,
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

    // By default exclude built-in system agents from the user-facing list.
    // Pass ?include_builtin=true to see all agents regardless of the flag.
    let built_in_filter = if query.include_builtin.unwrap_or(false) {
        None // no filter -- return everything
    } else {
        Some(false) // exclude system agents
    };

    let (agents, total) = state
        .manager
        .list_agents_paginated_org(
            status_filter,
            built_in_filter,
            query.project_id,
            org_id.as_deref(),
            limit,
            offset,
        )
        .await?;
    let mut items: Vec<AgentResponse> = Vec::with_capacity(agents.len());
    for agent in agents {
        let id = agent.id;
        let mut response = AgentResponse::from(agent);
        response.activity = state.registry.get_activity_state(&id).await;
        items.push(response);
    }

    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

/// `GET /system-agents` — list all built-in system agents.
///
/// Returns only agents with `built_in = true`. These agents are created
/// programmatically by the orchestrator at startup and are always present
/// while the service is running.
async fn list_system_agents(State(state): State<ApiState>) -> Result<impl IntoResponse, ApiError> {
    let agents = state.manager.list_system_agents().await?;
    let mut items: Vec<AgentResponse> = Vec::with_capacity(agents.len());
    for agent in agents {
        let id = agent.id;
        let mut response = AgentResponse::from(agent);
        response.activity = state.registry.get_activity_state(&id).await;
        items.push(response);
    }
    Ok(Json(items))
}

/// Validate that a `system_prompt_file` path exists and is a regular file,
/// returning its canonicalized form. Shared by create and update handlers.
fn validate_system_prompt_file(raw: String) -> Result<String, ApiError> {
    let p = std::path::Path::new(&raw);
    if !p.exists() {
        return Err(ApiError::InvalidInput(format!("system_prompt_file does not exist: {raw}")));
    }
    if !p.is_file() {
        return Err(ApiError::InvalidInput(format!(
            "system_prompt_file is not a regular file: {raw}"
        )));
    }
    Ok(std::fs::canonicalize(p).map(|c| c.to_string_lossy().to_string()).unwrap_or(raw))
}

async fn create_agent(
    OptionalTenantId(org_id): OptionalTenantId,
    State(state): State<ApiState>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate and canonicalize system_prompt_file if provided.
    let system_prompt_file = req.system_prompt_file.map(validate_system_prompt_file).transpose()?;

    if let Some(servers) = &req.mcp_servers {
        for (name, server) in servers {
            if server.command.trim().is_empty() {
                return Err(ApiError::InvalidInput(format!(
                    "mcp_servers entry '{name}' has an empty command"
                )));
            }
        }
    }

    let config = AgentConfig {
        working_dir: req.working_dir,
        user: req.user,
        shell: req.shell,
        interactive: req.interactive,
        prompt: req.prompt,
        worktree: req.worktree,
        system_prompt: req.system_prompt,
        system_prompt_file,
        append_system_prompt: req.append_system_prompt,
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
        mcp_servers: req.mcp_servers,
    };

    // Pass organization_id directly into spawn_agent so the initial DB INSERT
    // includes the correct value — avoids a two-step INSERT then UPDATE that
    // would briefly expose the agent as unscoped to concurrent list queries.
    let agent = state.manager.spawn_agent(req.name, config, false, org_id).await?;

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
    // Guard: built-in system agents cannot be deleted via the API.
    let existing = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;
    if existing.built_in {
        return Err(ApiError::Forbidden("built-in system agents cannot be deleted".to_string()));
    }

    let agent = state.manager.terminate_agent(&id).await?;

    metrics::counter!("agents_terminated_total").increment(1);

    Ok(Json(AgentResponse::from(agent)))
}

/// Restart an agent: kill any existing session and re-launch with the same config.
///
/// Accepts agents in any status (Running, Failed, Stopped). Preserves the
/// agent ID, name, and config. Returns the updated agent.
async fn restart_agent(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let agent = state.manager.restart_agent_by_id(&id).await?;

    info!(agent_id = %id, "Agent restarted via API");
    metrics::counter!("agents_restarted_total").increment(1);

    Ok(Json(AgentResponse::from(agent)))
}

/// Fields that are baked into the launch command or session at spawn time.
/// Changing any of these on a running agent requires a restart to take effect.
fn launch_affecting_changed(before: &Agent, after: &Agent) -> bool {
    let (b, a) = (&before.config, &after.config);
    b.working_dir != a.working_dir
        || b.shell != a.shell
        || b.model != a.model
        || b.env != a.env
        || b.system_prompt != a.system_prompt
        || b.system_prompt_file != a.system_prompt_file
        || b.append_system_prompt != a.append_system_prompt
        || b.additional_dirs != a.additional_dirs
        || b.worktree != a.worktree
        || b.mcp_servers != a.mcp_servers
}

/// Update an agent's configuration (merge-patch semantics).
///
/// Absent fields are left unchanged. The config is always persisted; pass
/// `restart: true` to relaunch the process so launch-affecting changes apply
/// immediately. Tool policy changes apply to the live connection without a
/// restart. See [`UpdateAgentRequest`] for env redaction round-trip rules.
async fn update_agent(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAgentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let before = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;
    if before.built_in {
        return Err(ApiError::Forbidden("built-in system agents cannot be modified".to_string()));
    }

    let mut agent = before.clone();

    if let Some(name) = req.name {
        if name.trim().is_empty() {
            return Err(ApiError::InvalidInput("name must not be empty".to_string()));
        }
        agent.name = name.trim().to_string();
    }

    if let Some(working_dir) = req.working_dir {
        if !std::path::Path::new(&working_dir).is_dir() {
            return Err(ApiError::InvalidInput(format!(
                "working_dir is not a directory or does not exist: {working_dir}"
            )));
        }
        agent.config.working_dir = working_dir;
    }

    if let Some(shell) = req.shell {
        agent.config.shell = shell;
    }
    if let Some(prompt) = req.prompt {
        agent.config.prompt = if prompt.is_empty() { None } else { Some(prompt) };
    }

    // system_prompt and system_prompt_file are mutually exclusive: setting one
    // non-empty clears the other; an empty string clears the field itself.
    if let Some(sp) = req.system_prompt {
        if sp.is_empty() {
            agent.config.system_prompt = None;
        } else {
            agent.config.system_prompt = Some(sp);
            agent.config.system_prompt_file = None;
        }
    }
    if let Some(spf) = req.system_prompt_file {
        if spf.is_empty() {
            agent.config.system_prompt_file = None;
        } else {
            agent.config.system_prompt_file = Some(validate_system_prompt_file(spf)?);
            agent.config.system_prompt = None;
        }
    }
    if let Some(append) = req.append_system_prompt {
        agent.config.append_system_prompt = append;
    }

    if let Some(model) = req.model {
        agent.config.model = Some(model);
    }
    if let Some(policy) = req.tool_policy {
        agent.config.tool_policy = policy;
    }

    // Env: full replacement, except the redaction sentinel keeps the stored
    // value so clients can round-trip a redacted config (see ENV_REDACTED).
    if let Some(env) = req.env {
        let mut merged = HashMap::with_capacity(env.len());
        for (key, value) in env {
            if value == ENV_REDACTED {
                match before.config.env.get(&key) {
                    Some(stored) => {
                        merged.insert(key, stored.clone());
                    }
                    None => {
                        return Err(ApiError::InvalidInput(format!(
                            "env value for key '{key}' is the redaction placeholder but the key has no stored value"
                        )));
                    }
                }
            } else {
                merged.insert(key, value);
            }
        }
        agent.config.env = merged;
    }

    if let Some(threshold) = req.auto_clear_threshold {
        agent.config.auto_clear_threshold = Some(threshold);
    }

    if let Some(dirs) = req.additional_dirs {
        let mut canonical_dirs = Vec::with_capacity(dirs.len());
        for dir in dirs {
            if !std::path::Path::new(&dir).is_dir() {
                return Err(ApiError::InvalidInput(format!(
                    "additional_dirs entry is not a directory or does not exist: {dir}"
                )));
            }
            let canonical =
                std::fs::canonicalize(&dir).map(|p| p.to_string_lossy().to_string()).unwrap_or(dir);
            if !canonical_dirs.contains(&canonical) {
                canonical_dirs.push(canonical);
            }
        }
        agent.config.additional_dirs = canonical_dirs;
    }

    if let Some(rooms) = req.rooms {
        agent.config.rooms = rooms;
    }
    if let Some(worktree) = req.worktree {
        agent.config.worktree = worktree;
    }

    // MCP servers: full replacement (empty map clears). Entry env values that
    // are the redaction sentinel keep the stored value, mirroring `env`.
    if let Some(servers) = req.mcp_servers {
        if servers.is_empty() {
            agent.config.mcp_servers = None;
        } else {
            let mut merged = HashMap::with_capacity(servers.len());
            for (name, mut server) in servers {
                if server.command.trim().is_empty() {
                    return Err(ApiError::InvalidInput(format!(
                        "mcp_servers entry '{name}' has an empty command"
                    )));
                }
                for (key, value) in server.env.iter_mut() {
                    if value == ENV_REDACTED {
                        let stored = before
                            .config
                            .mcp_servers
                            .as_ref()
                            .and_then(|m| m.get(&name))
                            .and_then(|s| s.env.get(key));
                        match stored {
                            Some(stored) => *value = stored.clone(),
                            None => {
                                return Err(ApiError::InvalidInput(format!(
                                    "mcp_servers entry '{name}' env key '{key}' is the redaction \
                                     placeholder but the key has no stored value"
                                )));
                            }
                        }
                    }
                }
                merged.insert(name, server);
            }
            agent.config.mcp_servers = Some(merged);
        }
    }

    agent.updated_at = chrono::Utc::now();

    let needs_restart = launch_affecting_changed(&before, &agent);
    let agent = state.manager.update_agent_and_maybe_restart(agent, req.restart).await?;

    info!(agent_id = %id, restarted = req.restart, "Agent updated via API");
    metrics::counter!("agents_updated_total").increment(1);

    let requires_restart = needs_restart && !req.restart && agent.status == AgentStatus::Running;
    Ok(Json(UpdateAgentResponse {
        agent: AgentResponse::from(agent),
        requires_restart,
        restarted: req.restart,
    }))
}

/// Send a message (prompt) to a running non-interactive agent.
async fn send_message(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SendMessageRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists and is running.
    let mut agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    // Lazy built-in agents stay dormant until first contact: spawn on
    // demand, then wait for the session to connect before delivering.
    if agent.built_in && agent.status != AgentStatus::Running {
        agent = state
            .manager
            .ensure_builtin_running(&id)
            .await
            .map_err(|e| ApiError::Internal(e.context("Failed to wake system agent")))?;

        // SDK-mode delivery goes over the WebSocket, which needs the freshly
        // spawned Claude process to connect back first.  Subprocess stdio
        // backends register immediately; PTY/interactive agents bypass the
        // registry entirely.
        if !agent.config.interactive && !state.registry.is_connected(&id).await {
            for _ in 0..30 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if state.registry.is_connected(&id).await {
                    break;
                }
            }
        }
    }

    if agent.status != AgentStatus::Running {
        return Err(ApiError::Conflict(format!(
            "Agent {} is not running (status: {})",
            id, agent.status
        )));
    }

    if agent.config.interactive {
        // Interactive mode: Claude reads from PTY stdin, not the WebSocket.
        // Inject the prompt as raw bytes so it appears as if the user typed it.
        state
            .manager
            .inject_pty_prompt(&id, &req.content)
            .await
            .map_err(|e| ApiError::InvalidInput(e.to_string()))?;
        metrics::counter!("agent_messages_sent_total", "mode" => "pty").increment(1);
    } else {
        // SDK mode: deliver via the orchestrator WebSocket protocol.
        state
            .registry
            .send_user_message(&id, &req.content)
            .await
            .map_err(|e| ApiError::InvalidInput(e.to_string()))?;
        metrics::counter!("agent_messages_sent_total", "mode" => "sdk").increment(1);
    }

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

    metrics::counter!("context_clears_total", "trigger" => "manual").increment(1);

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
    approvals.sort_by_key(|a| std::cmp::Reverse(a.created_at));

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

    metrics::counter!("approvals_resolved_total", "decision" => "approve").increment(1);
    let pending = state.registry.approvals.list(None, Some(&ApprovalStatus::Pending)).await.len();
    metrics::gauge!("approvals_pending").set(pending as f64);

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

    metrics::counter!("approvals_resolved_total", "decision" => "deny").increment(1);
    let pending = state.registry.approvals.list(None, Some(&ApprovalStatus::Pending)).await.len();
    metrics::gauge!("approvals_pending").set(pending as f64);

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
    approvals.sort_by_key(|a| std::cmp::Reverse(a.created_at));

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
                        project_id: None,
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

// ---------------------------------------------------------------------------
// Ask service event callback
// ---------------------------------------------------------------------------

/// Payload sent by the ask service when a question is answered or dismissed.
#[derive(Debug, Deserialize)]
struct AskEventPayload {
    event_type: String,
    question_id: uuid::Uuid,
    agent_id: String,
    workflow_id: Option<uuid::Uuid>,
    dispatch_id: Option<uuid::Uuid>,
    category: Option<String>,
    question: String,
    answer: Option<String>,
}

/// `POST /events/ask`
///
/// Receives ask-response callbacks from the ask service and publishes a
/// [`SystemEvent::AskResponseReceived`] to the event bus so that
/// `ask_response` workflow triggers can fire.
async fn ask_event_handler(
    State(state): State<ApiState>,
    Json(payload): Json<AskEventPayload>,
) -> impl IntoResponse {
    info!(
        question_id = %payload.question_id,
        agent_id = %payload.agent_id,
        event_type = %payload.event_type,
        "Received ask event callback"
    );

    state.scheduler.publish_event(SystemEvent::AskResponseReceived {
        question_id: payload.question_id,
        agent_id: payload.agent_id,
        workflow_id: payload.workflow_id,
        dispatch_id: payload.dispatch_id,
        category: payload.category,
        question: payload.question,
        answer: payload.answer,
        event_type: payload.event_type,
    });

    StatusCode::NO_CONTENT
}

// ---------------------------------------------------------------------------
// Project association handlers
// ---------------------------------------------------------------------------
// Project CRUD (create/read/update/delete) has been moved to the core service.
// These handlers manage the associations between projects and agents/workflows.

#[derive(Deserialize)]
struct ProjectListQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_project_agents(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ProjectListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = clamp_limit(query.limit);
    let offset = query.offset.unwrap_or(0);

    let (agents, total) = state
        .manager
        .agent_storage()
        .list_paginated(None, None, Some(id), limit, offset)
        .await
        .map_err(ApiError::Internal)?;

    let mut items: Vec<AgentResponse> = Vec::with_capacity(agents.len());
    for agent in agents {
        let aid = agent.id;
        let mut response = AgentResponse::from(agent);
        response.activity = state.registry.get_activity_state(&aid).await;
        items.push(response);
    }
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

async fn associate_project_agent(
    State(state): State<ApiState>,
    Path((id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify agent exists.
    state
        .manager
        .get_agent(&agent_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    state
        .manager
        .agent_storage()
        .set_agent_project(&agent_id, Some(id))
        .await
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn dissociate_project_agent(
    State(state): State<ApiState>,
    Path((_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .manager
        .get_agent(&agent_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    state
        .manager
        .agent_storage()
        .set_agent_project(&agent_id, None)
        .await
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_project_workflows(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ProjectListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = clamp_limit(query.limit);
    let offset = query.offset.unwrap_or(0);

    let (workflows, total) = state
        .scheduler
        .storage()
        .list_workflows_paginated(limit, offset, Some(id))
        .await
        .map_err(ApiError::Internal)?;

    let items: Vec<WorkflowResponse> = workflows.into_iter().map(WorkflowResponse::from).collect();
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

async fn associate_project_workflow(
    State(state): State<ApiState>,
    Path((id, workflow_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify workflow exists.
    state
        .scheduler
        .storage()
        .get_workflow(&workflow_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    state
        .scheduler
        .storage()
        .set_workflow_project(&workflow_id, Some(id))
        .await
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn dissociate_project_workflow(
    State(state): State<ApiState>,
    Path((_id, workflow_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify workflow exists.
    state
        .scheduler
        .storage()
        .get_workflow(&workflow_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    state
        .scheduler
        .storage()
        .set_workflow_project(&workflow_id, None)
        .await
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Conversation history endpoints ──────────────────────────────────────────

/// `GET /agents/{id}/conversation` — paginated conversation event history.
///
/// Supports optional query parameters:
/// - `limit` (default 100, max 500)
/// - `before` / `after` — RFC 3339 timestamp bounds
/// - `event_type` — comma-separated filter (e.g. `"output,tool_use"`)
/// - `session` — restrict to a specific session number
async fn get_agent_conversation(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ConversationHistoryQuery>,
) -> Result<impl IntoResponse, ApiError> {
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    let limit = query.limit.unwrap_or(100).min(500);

    let since = query
        .after
        .as_deref()
        .map(|s| {
            s.parse::<chrono::DateTime<chrono::Utc>>()
                .map_err(|e| ApiError::InvalidInput(format!("invalid 'after' timestamp: {e}")))
        })
        .transpose()?;

    let until = query
        .before
        .as_deref()
        .map(|s| {
            s.parse::<chrono::DateTime<chrono::Utc>>()
                .map_err(|e| ApiError::InvalidInput(format!("invalid 'before' timestamp: {e}")))
        })
        .transpose()?;

    let event_types = query
        .event_type
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|t| {
                    t.trim()
                        .parse::<ConversationEventType>()
                        .map_err(|e| ApiError::InvalidInput(format!("invalid event_type: {e}")))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    let opts = ConversationQuery {
        event_types,
        since,
        until,
        // Session filter is pushed to the DB so that `has_more` and `total`
        // are accurate for the filtered result set.
        session_number: query.session,
        // Fetch one extra to determine `has_more` without a separate COUNT query.
        limit: Some(limit + 1),
        offset: None,
    };

    let mut events = state
        .manager
        .agent_storage()
        .list_conversation_events(id, &opts)
        .await
        .map_err(ApiError::Internal)?;

    let has_more = events.len() as u64 > limit;
    if has_more {
        events.truncate(limit as usize);
    }

    let total = state
        .manager
        .agent_storage()
        .count_conversation_events(id)
        .await
        .map_err(ApiError::Internal)?;

    let items: Vec<ConversationEventResponse> =
        events.into_iter().map(ConversationEventResponse::from).collect();

    Ok(Json(ConversationHistoryResponse { events: items, total, has_more }))
}

/// `GET /agents/{id}/conversation/summary` — aggregate statistics.
///
/// Returns total event count, per-type counts, session count, and first/last
/// event timestamps.
async fn get_agent_conversation_summary(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    let summary = state
        .manager
        .agent_storage()
        .get_conversation_summary(id)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(summary))
}

/// `GET /agents/{id}/conversation/{event_id}` — single event by ID.
///
/// Returns 404 if the event does not exist or does not belong to the agent.
async fn get_agent_conversation_event(
    State(state): State<ApiState>,
    Path((id, event_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    let event = state
        .manager
        .agent_storage()
        .get_conversation_event_by_id(id, event_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(ConversationEventResponse::from(event)))
}

/// `DELETE /agents/{id}/conversation` — delete all conversation history for an agent.
///
/// Returns 204 No Content on success, 404 if the agent does not exist.
async fn delete_agent_conversation(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;

    state
        .manager
        .agent_storage()
        .delete_conversation_events_for_agent(id)
        .await
        .map_err(ApiError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}

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
