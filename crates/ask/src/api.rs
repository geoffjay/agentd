//! REST API endpoints and routing for the ask service.
//!
//! This module defines all HTTP endpoints, request handlers, and the router configuration.
//! It provides endpoints for health checks, triggering environment checks, and submitting
//! answers to questions.
//!
//! # API Endpoints
//!
//! - `GET /health` - Health check returning service status
//! - `POST /trigger` - Trigger checks and create notifications if needed
//! - `POST /answer` - Submit an answer to a pending question
//!
//! # Request/Response Flow
//!
//! 1. Client sends POST /trigger request
//! 2. Service checks tmux sessions
//! 3. If no sessions running and cooldown expired, creates notification
//! 4. Returns check results and notification IDs
//! 5. Client can then POST /answer to respond to questions
//!
//! # Examples
//!
//! ## Health Check
//!
//! ```bash
//! curl http://localhost:17001/health
//! # Returns: {"status":"ok","service":"agentd-ask","version":"0.1.0",...}
//! ```
//!
//! ## Trigger Checks
//!
//! ```bash
//! curl -X POST http://localhost:17001/trigger
//! # Returns: {"checks_run":["tmux_sessions"],"notifications_sent":[...],...}
//! ```
//!
//! ## Submit Answer
//!
//! ```bash
//! curl -X POST http://localhost:17001/answer \
//!   -H "Content-Type: application/json" \
//!   -d '{"question_id":"550e8400-e29b-41d4-a716-446655440000","answer":"yes"}'
//! # Returns: {"success":true,"message":"Answer recorded...","question_id":"..."}
//! ```

use crate::{
    error::ApiError,
    notification_client::NotificationClient,
    state::AppState,
    tmux_check,
    types::{
        AnswerRequest, AnswerResponse, CheckType, HealthResponse, NotificationStatus, QuestionInfo,
        QuestionStatus, TriggerResponse, TriggerResults, UpdateNotificationRequest,
    },
};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Shared state for API handlers.
///
/// This struct holds all the shared state needed by API handlers, including
/// application state, the notification client, and configuration. It is cloned
/// cheaply (uses `Arc` internally) for each request.
///
/// # Fields
///
/// - `app_state` - Thread-safe application state for questions and cooldowns
/// - `notification_client` - HTTP client for communicating with notification service
/// - `notification_service_url` - Base URL of the notification service
///
/// # Examples
///
/// ```no_run
/// use ask::{api::ApiState, state::AppState, notification_client::NotificationClient};
///
/// let api_state = ApiState {
///     app_state: AppState::new(),
///     notification_client: NotificationClient::new("http://localhost:17004".to_string()),
///     notification_service_url: "http://localhost:17004".to_string(),
/// };
/// ```
#[derive(Clone)]
pub struct ApiState {
    pub app_state: AppState,
    pub notification_client: NotificationClient,
    pub notification_service_url: String,
}

/// Creates the API router without middleware.
///
/// Configures all routes and attaches the shared state. This is the base router
/// that can be wrapped with additional middleware layers.
///
/// # Arguments
///
/// - `state` - The shared [`ApiState`] containing application state and clients
///
/// # Returns
///
/// Returns a configured [`Router`] ready to be served or wrapped with middleware.
///
/// # Examples
///
/// ```no_run
/// use ask::{api::{create_router, ApiState}, state::AppState, notification_client::NotificationClient};
///
/// # async fn example() {
/// let api_state = ApiState {
///     app_state: AppState::new(),
///     notification_client: NotificationClient::new("http://localhost:17004".to_string()),
///     notification_service_url: "http://localhost:17004".to_string(),
/// };
///
/// let router = create_router(api_state);
/// // Serve the router...
/// # }
/// ```
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/trigger", post(trigger_checks))
        .route("/answer", post(answer_question))
        .with_state(state)
}

/// Health check endpoint handler.
///
/// Returns service status, version, and configuration information. This endpoint
/// is useful for monitoring and verifying the service is running correctly.
///
/// # HTTP Method
///
/// `GET /health`
///
/// # Returns
///
/// Returns HTTP 200 with [`HealthResponse`] JSON containing:
/// - Service status ("ok")
/// - Service name
/// - Version number
/// - Notification service URL
///
/// # Examples
///
/// ```bash
/// curl http://localhost:17001/health
/// ```
///
/// Response:
/// ```json
/// {
///   "status": "ok",
///   "service": "agentd-ask",
///   "version": "0.1.0",
///   "notification_service_url": "http://localhost:17004"
/// }
/// ```
async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    let response = HealthResponse {
        status: "ok".to_string(),
        service: "agentd-ask".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        notification_service_url: state.notification_service_url.clone(),
    };

    Json(response)
}

/// Triggers environment checks and creates notifications if conditions are met.
///
/// Performs all configured checks (currently tmux session check) and creates
/// notifications for conditions that need user attention. Respects cooldown
/// periods to avoid notification spam.
///
/// # HTTP Method
///
/// `POST /trigger`
///
/// # Behavior
///
/// 1. Checks for running tmux sessions
/// 2. If no sessions and cooldown expired, creates a question notification
/// 3. Records notification timestamp to enforce cooldown
/// 4. Stores question in application state
/// 5. Returns check results and notification IDs
///
/// # Returns
///
/// Returns HTTP 200 with [`TriggerResponse`] containing:
/// - List of checks that were run
/// - List of notification IDs that were created
/// - Detailed results for each check
///
/// # Errors
///
/// - [`ApiError::TmuxError`] if tmux is not installed
/// - [`ApiError::NotificationError`] if notification service is unavailable
///
/// # Examples
///
/// ```bash
/// curl -X POST http://localhost:17001/trigger
/// ```
///
/// Response when notification is sent:
/// ```json
/// {
///   "checks_run": ["tmux_sessions"],
///   "notifications_sent": ["550e8400-e29b-41d4-a716-446655440000"],
///   "results": {
///     "tmux_sessions": {
///       "running": false,
///       "session_count": 0,
///       "sessions": []
///     }
///   }
/// }
/// ```
///
/// Response when in cooldown:
/// ```json
/// {
///   "checks_run": ["tmux_sessions"],
///   "notifications_sent": [],
///   "results": {
///     "tmux_sessions": {
///       "running": false,
///       "session_count": 0,
///       "sessions": []
///     }
///   }
/// }
/// ```
async fn trigger_checks(State(state): State<ApiState>) -> Result<Json<TriggerResponse>, ApiError> {
    info!("Running trigger checks");

    let mut checks_run = Vec::new();
    let mut notifications_sent = Vec::new();

    // Check tmux sessions
    checks_run.push(CheckType::TmuxSessions.as_str().to_string());

    let tmux_result = match tmux_check::check_tmux_sessions() {
        Ok(result) => {
            debug!(
                "tmux check succeeded: running={}, count={}",
                result.running, result.session_count
            );
            result
        }
        Err(e) => {
            warn!("tmux check failed: {}", e);
            // For all errors (including tmux not installed), assume no sessions running
            // This allows the service to operate gracefully in environments without tmux
            crate::types::TmuxCheckResult {
                running: false,
                session_count: 0,
                sessions: Some(Vec::new()),
            }
        }
    };

    // If no sessions running and we can send a notification, do it
    if !tmux_result.running && state.app_state.can_send_notification(CheckType::TmuxSessions).await
    {
        info!("No tmux sessions running, sending notification");

        let question_id = Uuid::new_v4();

        match state.notification_client.create_tmux_session_question(question_id).await {
            Ok(notification) => {
                info!("Created notification {} for question {}", notification.id, question_id);

                // Record the notification
                state.app_state.record_notification(CheckType::TmuxSessions).await;

                // Store the question
                let question = QuestionInfo {
                    question_id,
                    notification_id: notification.id,
                    check_type: CheckType::TmuxSessions,
                    asked_at: Utc::now(),
                    status: QuestionStatus::Pending,
                    answer: None,
                };
                state.app_state.add_question(question).await;

                notifications_sent.push(notification.id);
            }
            Err(e) => {
                error!("Failed to create notification: {}", e);
                return Err(ApiError::NotificationError(e));
            }
        }
    } else if !tmux_result.running {
        debug!(
            "No tmux sessions running, but notification was sent recently (within cooldown period)"
        );
    }

    let response = TriggerResponse {
        checks_run,
        notifications_sent,
        results: TriggerResults { tmux_sessions: tmux_result },
    };

    Ok(Json(response))
}

/// Submits an answer to a pending question.
///
/// Accepts a user's response to a question that was created by a previous trigger
/// check. Updates the question status to answered and notifies the notification
/// service of the response.
///
/// # HTTP Method
///
/// `POST /answer`
///
/// # Request Body
///
/// Expects JSON with [`AnswerRequest`]:
/// ```json
/// {
///   "question_id": "550e8400-e29b-41d4-a716-446655440000",
///   "answer": "yes"
/// }
/// ```
///
/// # Behavior
///
/// 1. Validates question exists
/// 2. Checks question is still pending (not already answered or expired)
/// 3. Updates question status in application state
/// 4. Sends update to notification service
/// 5. Processes answer based on check type (logs for now)
///
/// # Returns
///
/// Returns HTTP 200 with [`AnswerResponse`] containing:
/// - Success status
/// - Confirmation message
/// - Question ID
///
/// # Errors
///
/// - [`ApiError::QuestionNotFound`] (404) if question ID doesn't exist
/// - [`ApiError::QuestionNotActionable`] (410) if question is already answered or expired
///
/// # Examples
///
/// ```bash
/// curl -X POST http://localhost:17001/answer \
///   -H "Content-Type: application/json" \
///   -d '{
///     "question_id": "550e8400-e29b-41d4-a716-446655440000",
///     "answer": "yes"
///   }'
/// ```
///
/// Success response:
/// ```json
/// {
///   "success": true,
///   "message": "Answer recorded for question 550e8400-e29b-41d4-a716-446655440000",
///   "question_id": "550e8400-e29b-41d4-a716-446655440000"
/// }
/// ```
///
/// Error response (question not found):
/// ```json
/// {
///   "error": "Question 550e8400-e29b-41d4-a716-446655440000 not found"
/// }
/// ```
async fn answer_question(
    State(state): State<ApiState>,
    Json(request): Json<AnswerRequest>,
) -> Result<Json<AnswerResponse>, ApiError> {
    info!("Received answer for question {}", request.question_id);

    // Get the question
    let question = state.app_state.get_question(&request.question_id).await.ok_or_else(|| {
        ApiError::QuestionNotFound(format!("Question {} not found", request.question_id))
    })?;

    // Check if the question is still actionable
    if question.status != QuestionStatus::Pending {
        return Err(ApiError::QuestionNotActionable(format!(
            "Question {} is not pending (status: {:?})",
            request.question_id, question.status
        )));
    }

    // Update the question with the answer
    let updated_question = state
        .app_state
        .answer_question(&request.question_id, request.answer.clone())
        .await
        .map_err(ApiError::InternalError)?;

    info!("Question {} answered with: {}", request.question_id, request.answer);

    // Update the notification status
    let update_request = UpdateNotificationRequest {
        status: Some(NotificationStatus::Responded),
        response: Some(request.answer.clone()),
    };

    match state
        .notification_client
        .update_notification(question.notification_id, update_request)
        .await
    {
        Ok(notification) => {
            info!("Updated notification {} with response", notification.id);
        }
        Err(e) => {
            error!("Failed to update notification {}: {}", question.notification_id, e);
            // Don't fail the request, but log the error
        }
    }

    // Process the answer based on check type
    match question.check_type {
        CheckType::TmuxSessions => {
            info!("User answered '{}' to tmux session question", request.answer);
            // In a real implementation, we could trigger an action here
            // For now, we just log it
        }
    }

    let response = AnswerResponse {
        success: true,
        message: format!("Answer recorded for question {}", updated_question.question_id),
        question_id: updated_question.question_id,
    };

    Ok(Json(response))
}

/// Creates the API router with HTTP tracing middleware.
///
/// Wraps the base router with Tower's tracing middleware for automatic request
/// and response logging. This is the recommended router to use in production.
///
/// # Arguments
///
/// - `state` - The shared [`ApiState`] containing application state and clients
///
/// # Returns
///
/// Returns a configured [`Router`] with tracing middleware attached, ready to serve.
///
/// # Tracing
///
/// Logs all HTTP requests and responses at INFO level, including:
/// - Request method and path
/// - Response status code
/// - Request duration
///
/// # Examples
///
/// ```no_run
/// use ask::{api::{create_router_with_tracing, ApiState}, state::AppState, notification_client::NotificationClient};
///
/// # async fn example() {
/// let api_state = ApiState {
///     app_state: AppState::new(),
///     notification_client: NotificationClient::new("http://localhost:17004".to_string()),
///     notification_service_url: "http://localhost:17004".to_string(),
/// };
///
/// let router = create_router_with_tracing(api_state);
/// // Router now logs all requests automatically
/// # }
/// ```
pub fn create_router_with_tracing(state: ApiState) -> Router {
    use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};

    create_router(state).layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
            .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_state_creation() {
        let app_state = AppState::new();
        let notification_client = NotificationClient::new("http://localhost:17004".to_string());
        let api_state = ApiState {
            app_state,
            notification_client,
            notification_service_url: "http://localhost:17004".to_string(),
        };

        assert_eq!(api_state.notification_service_url, "http://localhost:17004");
    }

    // More comprehensive tests would require mocking or integration testing
    // These are better suited for the integration test suite
}
