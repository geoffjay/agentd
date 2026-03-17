use crate::api::ApiError;
use crate::manager::AgentManager;
use crate::scheduler::template::validate_template;
use crate::scheduler::types::*;
use crate::scheduler::Scheduler;
use crate::types::{clamp_limit, AgentStatus, PaginatedResponse};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;
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
    }

    // Reject trigger types that are not yet implemented.
    if !req.trigger_config.is_implemented() {
        return Err(ApiError::InvalidInput(format!(
            "Trigger type '{}' is not yet implemented. Currently supported: github_issues, github_pull_requests, cron, delay",
            req.trigger_config.trigger_type()
        )));
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
        state.scheduler.storage().list_workflows_paginated(limit, offset).await?;
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
