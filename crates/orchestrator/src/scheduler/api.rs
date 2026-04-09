use crate::api::ApiError;
use crate::manager::AgentManager;
use crate::scheduler::storage::QueueStats;
use crate::scheduler::template::validate_template;
use crate::scheduler::types::{WebhookSource, *};
use crate::scheduler::webhook;
use crate::scheduler::Scheduler;
use crate::types::{clamp_limit, AgentStatus, PaginatedResponse};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

#[derive(Deserialize)]
struct PaginationParams {
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Clone)]
pub struct WorkflowState {
    pub scheduler: Arc<Scheduler>,
    pub manager: Arc<AgentManager>,
}

pub fn workflow_routes(state: WorkflowState) -> Router {
    Router::new()
        .route("/workflows", get(list_workflows).post(create_workflow))
        .route("/workflows/{id}", get(get_workflow).put(update_workflow).delete(delete_workflow))
        .route("/workflows/{id}/history", get(dispatch_history))
        .route("/workflows/{id}/trigger", post(trigger_workflow))
        .with_state(state)
}

async fn create_workflow(
    State(state): State<WorkflowState>,
    Json(req): Json<CreateWorkflowRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate the prompt template
    let warnings = validate_template(&req.prompt_template);
    let errors: Vec<&String> = warnings
        .iter()
        .filter(|w| {
            w.contains("Unknown template variable") || w.contains("Unclosed") || w.contains("empty")
        })
        .collect();
    if !errors.is_empty() {
        return Err(ApiError::InvalidInput(format!(
            "Invalid prompt template: {}",
            errors.iter().map(|e| e.as_str()).collect::<Vec<_>>().join("; ")
        )));
    }

    // Validate trigger configuration.
    match &req.trigger_config {
        TriggerConfig::GithubIssues { owner, repo, .. }
        | TriggerConfig::GithubPullRequests { owner, repo, .. } => {
            if owner.trim().is_empty() || repo.trim().is_empty() {
                return Err(ApiError::InvalidInput(
                    "GitHub trigger requires non-empty 'owner' and 'repo'".to_string(),
                ));
            }
        }
        TriggerConfig::Cron { expression } => {
            if expression.trim().is_empty() {
                return Err(ApiError::InvalidInput(
                    "Cron trigger requires a non-empty 'expression'".to_string(),
                ));
            }
            // Validate the cron expression at creation time (fail fast).
            if let Err(e) = expression.parse::<croner::Cron>() {
                return Err(ApiError::InvalidInput(format!(
                    "Invalid cron expression '{}': {}",
                    expression, e
                )));
            }
        }
        TriggerConfig::Delay { run_at } => {
            if run_at.trim().is_empty() {
                return Err(ApiError::InvalidInput(
                    "Delay trigger requires a non-empty 'run_at' datetime".to_string(),
                ));
            }
            // Validate the datetime is parseable as ISO 8601 / RFC 3339.
            if chrono::DateTime::parse_from_rfc3339(run_at).is_err()
                && run_at.parse::<chrono::DateTime<Utc>>().is_err()
            {
                return Err(ApiError::InvalidInput(format!(
                    "Invalid run_at datetime '{}': expected ISO 8601 format (e.g., 2025-01-01T09:00:00Z)",
                    run_at
                )));
            }
        }
        TriggerConfig::AgentLifecycle { event } => {
            let valid_events = ["session_start", "session_end", "context_clear"];
            if !valid_events.contains(&event.as_str()) {
                return Err(ApiError::InvalidInput(format!(
                    "Invalid agent lifecycle event '{}'. Valid values: {}",
                    event,
                    valid_events.join(", ")
                )));
            }
        }
        TriggerConfig::DispatchResult { .. } => {
            // No additional validation needed; source_workflow_id and status are optional.
        }
        TriggerConfig::Webhook { .. } | TriggerConfig::Manual {} => {}
        TriggerConfig::AgentIdle { idle_seconds } => {
            if *idle_seconds == 0 {
                return Err(ApiError::InvalidInput(
                    "AgentIdle trigger requires 'idle_seconds' to be greater than 0".to_string(),
                ));
            }
        }
        TriggerConfig::LinearIssues { team_key, project, status, labels, assignee } => {
            // Require at least one filter so the scheduler does not poll the
            // entire Linear workspace indiscriminately.  All filter fields are
            // optional individually, but at least one must be provided.
            let has_team = team_key.as_deref().is_some_and(|v| !v.trim().is_empty());
            let has_project = project.as_deref().is_some_and(|v| !v.trim().is_empty());
            let has_status = status.as_deref().is_some_and(|v| !v.is_empty());
            let has_labels = !labels.is_empty();
            let has_assignee = assignee.as_deref().is_some_and(|v| !v.trim().is_empty());

            if !has_team && !has_project && !has_status && !has_labels && !has_assignee {
                return Err(ApiError::InvalidInput(
                    "Linear trigger requires at least one filter: \
                     team_key, project, status, labels, or assignee."
                        .to_string(),
                ));
            }
        }
        TriggerConfig::Composite { mode, triggers, .. } => {
            // Validate combinator mode.
            if mode != "or" && mode != "and" {
                return Err(ApiError::InvalidInput(format!(
                    "Invalid composite mode '{}'. Valid values: 'or', 'and'",
                    mode
                )));
            }
            // Require at least 2 sub-triggers.
            if triggers.len() < 2 {
                return Err(ApiError::InvalidInput(
                    "Composite trigger requires at least 2 sub-triggers".to_string(),
                ));
            }
            // Guard against excessive nesting (max 3 levels).
            fn check_depth(tc: &TriggerConfig, depth: usize) -> Result<(), String> {
                if let TriggerConfig::Composite { triggers, .. } = tc {
                    if depth >= 3 {
                        return Err(
                            "Composite trigger nesting exceeds maximum depth of 3".to_string()
                        );
                    }
                    for sub in triggers {
                        check_depth(sub, depth + 1)?;
                    }
                }
                Ok(())
            }
            if let Err(msg) = check_depth(&req.trigger_config, 0) {
                return Err(ApiError::InvalidInput(msg));
            }
        }
        TriggerConfig::Queue { queue_name, .. } => {
            if queue_name.trim().is_empty() {
                return Err(ApiError::InvalidInput(
                    "Queue trigger requires a non-empty 'queue_name'".to_string(),
                ));
            }
            if !queue_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                || queue_name.len() > 64
            {
                return Err(ApiError::InvalidInput(
                    "Queue name may only contain alphanumeric characters and hyphens (max 64 chars)".to_string(),
                ));
            }
        }
        TriggerConfig::AskResponse { .. } => {
            // No required fields — all filters (agent_id, category, response_pattern) are optional.
        }
    }

    // Reject trigger types that are not yet implemented.
    if !req.trigger_config.is_implemented() {
        return Err(ApiError::InvalidInput(format!(
            "Trigger type '{}' is not yet implemented. See documentation for currently supported trigger types.",
            req.trigger_config.trigger_type()
        )));
    }

    // For implemented trigger types that require external credentials, validate
    // them here so callers get a clear error at creation time rather than a
    // silent failure on the first poll. The key value is never included in any
    // error message.
    if matches!(req.trigger_config, TriggerConfig::LinearIssues { .. })
        && !crate::scheduler::linear::LinearConfig::is_configured()
    {
        return Err(ApiError::InvalidInput(
            "Linear API key not configured. \
             Set the AGENTD_LINEAR_API_KEY environment variable \
             or add 'api_key' to the [linear] section of the agentd config file."
                .to_string(),
        ));
    }

    // Validate that the agent exists and is running.
    let agent = state
        .manager
        .get_agent(&req.agent_id)
        .await?
        .ok_or(ApiError::InvalidInput("Agent not found".to_string()))?;

    if agent.status != AgentStatus::Running {
        return Err(ApiError::InvalidInput(format!(
            "Agent {} is not running (status: {})",
            req.agent_id, agent.status
        )));
    }

    let now = Utc::now();
    let config = WorkflowConfig {
        id: Uuid::new_v4(),
        name: req.name,
        agent_id: req.agent_id,
        trigger_config: req.trigger_config,
        prompt_template: req.prompt_template,
        poll_interval_secs: req.poll_interval_secs,
        enabled: req.enabled,
        tool_policy: req.tool_policy,
        created_at: now,
        updated_at: now,
        project_id: None,
    };

    state.scheduler.storage().add_workflow(&config).await?;

    // Start the runner if enabled.
    if config.enabled {
        state.scheduler.start_workflow(config.clone()).await?;
    }

    Ok((StatusCode::CREATED, Json(WorkflowResponse::from(config))))
}

async fn list_workflows(
    State(state): State<WorkflowState>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let (workflows, total) =
        state.scheduler.storage().list_workflows_paginated(limit, offset, None).await?;
    let items: Vec<WorkflowResponse> = workflows.into_iter().map(WorkflowResponse::from).collect();
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

async fn get_workflow(
    State(state): State<WorkflowState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let workflow = state.scheduler.storage().get_workflow(&id).await?.ok_or(ApiError::NotFound)?;

    Ok(Json(WorkflowResponse::from(workflow)))
}

async fn update_workflow(
    State(state): State<WorkflowState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWorkflowRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut workflow =
        state.scheduler.storage().get_workflow(&id).await?.ok_or(ApiError::NotFound)?;

    let was_enabled = workflow.enabled;

    if let Some(name) = req.name {
        workflow.name = name;
    }
    if let Some(template) = req.prompt_template {
        workflow.prompt_template = template;
    }
    if let Some(interval) = req.poll_interval_secs {
        workflow.poll_interval_secs = interval;
    }
    if let Some(enabled) = req.enabled {
        workflow.enabled = enabled;
    }
    if let Some(policy) = req.tool_policy {
        workflow.tool_policy = policy;
    }
    workflow.updated_at = Utc::now();

    state.scheduler.storage().update_workflow(&workflow).await?;

    // Handle enable/disable transitions.
    if !was_enabled && workflow.enabled {
        // Enabling: start the runner.
        if let Err(e) = state.scheduler.start_workflow(workflow.clone()).await {
            tracing::warn!(%e, "Failed to start workflow after enabling");
        }
    } else if was_enabled && !workflow.enabled {
        // Disabling: stop the runner.
        let _ = state.scheduler.stop_workflow(&id).await;
    }

    Ok(Json(WorkflowResponse::from(workflow)))
}

async fn delete_workflow(
    State(state): State<WorkflowState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Stop the runner if it's running.
    let _ = state.scheduler.stop_workflow(&id).await;

    state.scheduler.storage().delete_workflow(&id).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn dispatch_history(
    State(state): State<WorkflowState>,
    Path(id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify workflow exists.
    state.scheduler.storage().get_workflow(&id).await?.ok_or(ApiError::NotFound)?;

    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let (dispatches, total) =
        state.scheduler.storage().list_dispatches_paginated(&id, limit, offset).await?;
    let items: Vec<DispatchResponse> = dispatches.into_iter().map(DispatchResponse::from).collect();
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

// ---------------------------------------------------------------------------
// Manual trigger endpoint
// ---------------------------------------------------------------------------

/// Manually trigger a workflow on demand, bypassing its normal trigger strategy.
///
/// Accepts an optional JSON body:
/// ```json
/// { "title": "...", "body": "...", "metadata": { "key": "value" } }
/// ```
///
/// Returns:
/// - `200 OK` with the [`DispatchResponse`] on success
/// - `400 Bad Request` if the workflow is disabled
/// - `404 Not Found` if the workflow does not exist
/// - `409 Conflict` if the agent is currently busy
/// - `503 Service Unavailable` if the agent is not connected
async fn trigger_workflow(
    State(state): State<WorkflowState>,
    Path(id): Path<Uuid>,
    body: Option<Json<TriggerWorkflowRequest>>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify workflow exists.
    state.scheduler.storage().get_workflow(&id).await?.ok_or(ApiError::NotFound)?;

    let req = body.map(|Json(r)| r).unwrap_or_default();

    // Build a synthetic Task from the request body (or defaults).
    let source_id = format!("manual:{}", Uuid::new_v4());
    let task = Task {
        source_id,
        title: req.title.unwrap_or_else(|| "Manual trigger".to_string()),
        body: req.body.unwrap_or_default(),
        url: String::new(),
        labels: vec![],
        assignee: None,
        metadata: req.metadata,
    };

    info!(
        workflow_id = %id,
        source_id = %task.source_id,
        title = %task.title,
        "Manual workflow trigger requested"
    );

    let record = state.scheduler.trigger_workflow(&id, task).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not enabled") {
            ApiError::InvalidInput(msg)
        } else if msg.contains("currently busy") {
            ApiError::Conflict(msg)
        } else if msg.contains("not connected") {
            ApiError::ServiceUnavailable(msg)
        } else {
            ApiError::Internal(e)
        }
    })?;

    Ok(Json(DispatchResponse::from(record)))
}

// ---------------------------------------------------------------------------
// Webhook endpoint
// ---------------------------------------------------------------------------

/// Routes for inbound webhook delivery.
pub fn webhook_routes(state: WorkflowState) -> Router {
    Router::new().route("/webhooks/{workflow_id}", post(handle_webhook)).with_state(state)
}

/// Accept an inbound webhook POST, verify the signature (if configured),
/// parse the payload into a [`Task`], and push it to the workflow's channel.
///
/// Returns:
/// - `202 Accepted` on success
/// - `401 Unauthorized` if the signature is invalid
/// - `404 Not Found` if the workflow is not running or not a webhook trigger
/// - `422 Unprocessable Entity` if the workflow exists but is not a webhook type
/// - `503 Service Unavailable` if the channel is full (backpressure)
async fn handle_webhook(
    State(state): State<WorkflowState>,
    Path(workflow_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    // Look up the workflow in the webhook registry.
    let (sender, secret, source) =
        match state.scheduler.webhook_registry().lookup(&workflow_id).await {
            Some(entry) => entry,
            None => {
                // Distinguish between "not found" and "not a webhook trigger".
                if let Ok(Some(wf)) = state.scheduler.storage().get_workflow(&workflow_id).await {
                    if !matches!(wf.trigger_config, TriggerConfig::Webhook { .. }) {
                        return Err(ApiError::InvalidInput(format!(
                            "Workflow {} is not a webhook trigger (type: {})",
                            workflow_id,
                            wf.trigger_config.trigger_type()
                        )));
                    }
                }
                return Err(ApiError::NotFound);
            }
        };

    // Extract source-specific headers for payload parsing and signature verification.
    let github_event = headers.get("x-github-event").and_then(|v| v.to_str().ok());
    let delivery_id = headers.get("x-github-delivery").and_then(|v| v.to_str().ok());
    let linear_event = headers.get("linear-event").and_then(|v| v.to_str().ok());
    let linear_delivery = headers.get("linear-delivery").and_then(|v| v.to_str().ok());

    // Enforce that the incoming request matches the registered webhook source.
    // This prevents source-header spoofing attacks where an attacker injects
    // platform headers to bypass or redirect signature verification.
    match &source {
        WebhookSource::GitHub if linear_event.is_some() => {
            return Err(ApiError::InvalidInput(
                "This webhook workflow only accepts GitHub events; unexpected Linear-Event header"
                    .to_string(),
            ));
        }
        WebhookSource::Linear if linear_event.is_none() => {
            return Err(ApiError::InvalidInput(
                "This webhook workflow only accepts Linear events; missing Linear-Event header"
                    .to_string(),
            ));
        }
        _ => {}
    }

    // Verify HMAC-SHA256 signature if a secret is configured.
    // Signature header is chosen based on the *registered* source to prevent
    // an attacker from switching verification paths by injecting headers.
    if let Some(ref secret_value) = secret {
        let (signature, header_name) = match &source {
            WebhookSource::Linear => (
                headers.get("linear-signature").and_then(|v| v.to_str().ok()).unwrap_or(""),
                "Linear-Signature",
            ),
            WebhookSource::GitHub => (
                headers.get("x-hub-signature-256").and_then(|v| v.to_str().ok()).unwrap_or(""),
                "X-Hub-Signature-256",
            ),
            WebhookSource::Any => {
                // When source is unspecified, infer from presence of Linear-Event header.
                if linear_event.is_some() {
                    (
                        headers.get("linear-signature").and_then(|v| v.to_str().ok()).unwrap_or(""),
                        "Linear-Signature",
                    )
                } else {
                    (
                        headers
                            .get("x-hub-signature-256")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or(""),
                        "X-Hub-Signature-256",
                    )
                }
            }
        };

        if signature.is_empty() {
            return Err(ApiError::Unauthorized(format!("Missing {} header", header_name)));
        }

        if !webhook::verify_signature(secret_value, &body, signature) {
            return Err(ApiError::Unauthorized("Invalid webhook signature".to_string()));
        }
    }

    // Parse the payload into a Task.
    let task = webhook::parse_webhook_payload(
        github_event,
        delivery_id,
        linear_event,
        linear_delivery,
        &body,
    );

    info!(
        %workflow_id,
        source_id = %task.source_id,
        title = %task.title,
        "Webhook payload received"
    );

    // Send the task through the channel.
    sender.try_send(task).map_err(|e| match e {
        tokio::sync::mpsc::error::TrySendError::Full(_) => ApiError::ServiceUnavailable(
            "Webhook channel full — workflow runner cannot keep up".to_string(),
        ),
        tokio::sync::mpsc::error::TrySendError::Closed(_) => ApiError::ServiceUnavailable(
            "Webhook channel closed — workflow runner may have stopped".to_string(),
        ),
    })?;

    Ok(StatusCode::ACCEPTED)
}

// ---------------------------------------------------------------------------
// Queue endpoints
// ---------------------------------------------------------------------------

/// Validates a queue name: alphanumeric + hyphens, 1–64 characters.
fn validate_queue_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::InvalidInput("Queue name must be 1–64 characters long".to_string()));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(ApiError::InvalidInput(
            "Queue name may only contain alphanumeric characters and hyphens".to_string(),
        ));
    }
    Ok(())
}

/// Request body for `POST /queues/{name}/push`.
#[derive(Deserialize)]
struct PushQueueRequest {
    title: String,
    body: Option<String>,
    #[serde(default)]
    priority: i32,
}

/// Response for a newly enqueued task.
#[derive(Serialize)]
struct QueueTaskResponse {
    id: String,
    queue_name: String,
    status: String,
    created_at: String,
}

/// Query params for `GET /queues/{name}/peek`.
#[derive(Deserialize)]
struct PeekParams {
    limit: Option<u64>,
}

/// Response item for a peeked queue task.
#[derive(Serialize)]
struct QueueTaskItem {
    id: String,
    queue_name: String,
    title: String,
    body: Option<String>,
    priority: i32,
    status: String,
    retry_count: i32,
    max_retries: i32,
    created_at: String,
}

/// Routes for queue management endpoints.
pub fn queue_routes(state: WorkflowState) -> Router {
    Router::new()
        .route("/queues/{name}/push", post(push_queue))
        .route("/queues/{name}/stats", get(queue_stats))
        .route("/queues/{name}/peek", get(peek_queue))
        .route("/queues/{name}", delete(purge_queue))
        .with_state(state)
}

/// `POST /queues/{name}/push` — enqueue a task.
async fn push_queue(
    State(state): State<WorkflowState>,
    Path(name): Path<String>,
    Json(req): Json<PushQueueRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_queue_name(&name)?;

    if req.title.trim().is_empty() {
        return Err(ApiError::InvalidInput("Task title must not be empty".to_string()));
    }

    let now = Utc::now().to_rfc3339();
    let id = state
        .scheduler
        .storage()
        .enqueue(&name, &req.title, req.body.as_deref(), req.priority)
        .await?;

    info!(queue_name = %name, task_id = %id, "Task enqueued");

    Ok((
        StatusCode::CREATED,
        Json(QueueTaskResponse {
            id,
            queue_name: name,
            status: "pending".to_string(),
            created_at: now,
        }),
    ))
}

/// `GET /queues/{name}/stats` — return counts by status.
async fn queue_stats(
    State(state): State<WorkflowState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    validate_queue_name(&name)?;

    let stats: QueueStats = state.scheduler.storage().queue_stats(&name).await?;
    Ok(Json(stats))
}

/// `GET /queues/{name}/peek?limit=N` — view pending tasks without claiming.
async fn peek_queue(
    State(state): State<WorkflowState>,
    Path(name): Path<String>,
    Query(params): Query<PeekParams>,
) -> Result<impl IntoResponse, ApiError> {
    validate_queue_name(&name)?;

    let limit = params.limit.unwrap_or(10).min(100);
    let tasks = state.scheduler.storage().peek_queue(&name, limit).await?;

    let items: Vec<QueueTaskItem> = tasks
        .into_iter()
        .map(|t| QueueTaskItem {
            id: t.id,
            queue_name: t.queue_name,
            title: t.title,
            body: t.body,
            priority: t.priority,
            status: t.status,
            retry_count: t.retry_count,
            max_retries: t.max_retries,
            created_at: t.created_at,
        })
        .collect();

    Ok(Json(items))
}

/// `DELETE /queues/{name}` — purge all tasks from the queue.
async fn purge_queue(
    State(state): State<WorkflowState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    validate_queue_name(&name)?;

    let deleted = state.scheduler.storage().purge_queue(&name).await?;

    info!(queue_name = %name, deleted, "Queue purged");

    Ok(Json(serde_json::json!({ "deleted": deleted })))
}
