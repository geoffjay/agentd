//! REST API endpoints for the ask service (agent-driven Q&A).
//!
//! # Endpoints
//!
//! - `POST /questions` — agent creates a question
//! - `POST /questions/{id}/answer` — human answers a question
//! - `POST /questions/{id}/dismiss` — human dismisses a question
//! - `GET /questions` — list questions with optional filters
//! - `GET /questions/{id}` — get a single question
//! - `GET /health` — health check

use crate::{
    error::ApiError,
    state::AppState,
    types::{
        AnswerQuestionRequest, CreateQuestionRequest, HealthResponse, ListQuestionsQuery, Question,
        QuestionStatus, QuestionsListResponse,
    },
};
use agentd_common::tenant::OptionalTenantId;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use tower_http::trace::TraceLayer;
use tracing::info;
use uuid::Uuid;

/// Shared state for API handlers.
#[derive(Clone)]
pub struct ApiState {
    pub app_state: AppState,
    /// Optional orchestrator callback URL for ask_response events.
    pub orchestrator_url: Option<String>,
    /// Shared HTTP client for outbound requests (e.g. orchestrator callbacks).
    pub http_client: reqwest::Client,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /health`
async fn health_handler(State(_state): State<ApiState>) -> impl IntoResponse {
    Json(HealthResponse::ok("agentd-ask", env!("CARGO_PKG_VERSION")))
}

/// `POST /questions` — agent creates a new question.
async fn create_question_handler(
    OptionalTenantId(org_id): OptionalTenantId,
    State(state): State<ApiState>,
    Json(req): Json<CreateQuestionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate request.
    if req.agent_id.trim().is_empty() {
        return Err(ApiError::InvalidRequest("agent_id is required".to_string()));
    }
    if req.question.trim().is_empty() {
        return Err(ApiError::InvalidRequest("question text is required".to_string()));
    }

    let question = state.app_state.storage.create_with_org(&req, org_id.as_deref()).await?;
    info!("Question {} created by agent {}", question.id, question.agent_id);

    Ok((StatusCode::CREATED, Json(question)))
}

/// `POST /questions/{id}/answer` — human answers a question.
async fn answer_question_handler(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AnswerQuestionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if req.answer.trim().is_empty() {
        return Err(ApiError::InvalidRequest("answer text is required".to_string()));
    }

    let question = state
        .app_state
        .storage
        .update_status(&id, QuestionStatus::Answered, Some(req.answer))
        .await?;
    info!("Question {} answered", id);

    // Fire-and-forget orchestrator callback.
    if let Some(ref url) = state.orchestrator_url {
        fire_orchestrator_callback(&state.http_client, url, &question, "question_answered");
    }

    Ok(Json(question))
}

/// `POST /questions/{id}/dismiss` — human dismisses a question.
async fn dismiss_question_handler(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let question =
        state.app_state.storage.update_status(&id, QuestionStatus::Dismissed, None).await?;
    info!("Question {} dismissed", id);

    // Fire-and-forget orchestrator callback.
    if let Some(ref url) = state.orchestrator_url {
        fire_orchestrator_callback(&state.http_client, url, &question, "question_dismissed");
    }

    Ok(Json(question))
}

/// `GET /questions` — list questions with optional filters.
async fn list_questions_handler(
    OptionalTenantId(org_id): OptionalTenantId,
    State(state): State<ApiState>,
    Query(query): Query<ListQuestionsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let status = query
        .status
        .as_deref()
        .map(|s| s.parse::<QuestionStatus>())
        .transpose()
        .map_err(|e| ApiError::InvalidRequest(format!("invalid status: {e}")))?;

    let questions: Vec<Question> = state
        .app_state
        .storage
        .list_org(
            status,
            query.agent_id.as_deref(),
            query.category.as_deref(),
            org_id.as_deref(),
            query.limit,
            query.offset,
        )
        .await?;

    let total = questions.len();
    Ok(Json(QuestionsListResponse { questions, total }))
}

/// `GET /questions/{id}` — get a single question.
async fn get_question_handler(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let question = state
        .app_state
        .storage
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::QuestionNotFound(format!("Question {id} not found")))?;

    Ok(Json(question))
}

// ---------------------------------------------------------------------------
// Orchestrator callback (fire-and-forget)
// ---------------------------------------------------------------------------

/// Spawns a background task that POSTs the ask event to the orchestrator.
///
/// Failures are logged but do not block the API response.
fn fire_orchestrator_callback(
    client: &reqwest::Client,
    orchestrator_url: &str,
    question: &Question,
    event_type: &str,
) {
    let url = format!("{}/events/ask", orchestrator_url.trim_end_matches('/'));
    let payload = serde_json::json!({
        "event_type": event_type,
        "question_id": question.id,
        "agent_id": question.agent_id,
        "workflow_id": question.workflow_id,
        "dispatch_id": question.dispatch_id,
        "category": question.category,
        "question": question.question,
        "answer": question.answer,
        "answered_at": question.answered_at,
    });

    let client = client.clone();
    tokio::spawn(async move {
        match client.post(&url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!("Orchestrator callback sent to {}", url);
            }
            Ok(resp) => {
                tracing::warn!("Orchestrator callback to {} returned {}", url, resp.status());
            }
            Err(e) => {
                tracing::warn!("Orchestrator callback to {} failed: {}", url, e);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Creates the Axum router with all endpoints and tracing middleware.
pub fn create_router_with_tracing(api_state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/questions", post(create_question_handler).get(list_questions_handler))
        .route("/questions/{id}/answer", post(answer_question_handler))
        .route("/questions/{id}/dismiss", post(dismiss_question_handler))
        .route("/questions/{id}", get(get_question_handler))
        .with_state(api_state)
        .layer(TraceLayer::new_for_http())
}
