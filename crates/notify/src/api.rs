use crate::{notification::*, storage::NotificationStorage};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub storage: Arc<NotificationStorage>,
}

/// Create the API router
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/notifications", axum::routing::get(list_notifications))
        .route("/notifications", axum::routing::post(create_notification))
        .route("/notifications/:id", axum::routing::get(get_notification))
        .route(
            "/notifications/:id",
            axum::routing::put(update_notification),
        )
        .route(
            "/notifications/:id",
            axum::routing::delete(delete_notification),
        )
        .route(
            "/notifications/actionable",
            axum::routing::get(list_actionable),
        )
        .route("/notifications/history", axum::routing::get(list_history))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "agentd-notify",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// List all notifications with optional status filter
async fn list_notifications(
    State(state): State<ApiState>,
    axum::extract::Query(params): axum::extract::Query<ListParams>,
) -> Result<Json<Vec<Notification>>, ApiError> {
    let status = params.status.map(|s| s.parse()).transpose()?;
    let notifications = state.storage.list(status).await?;
    Ok(Json(notifications))
}

/// Get actionable notifications
async fn list_actionable(State(state): State<ApiState>) -> Result<Json<Vec<Notification>>, ApiError> {
    let notifications = state.storage.list_actionable().await?;
    Ok(Json(notifications))
}

/// Get notification history
async fn list_history(State(state): State<ApiState>) -> Result<Json<Vec<Notification>>, ApiError> {
    let notifications = state.storage.list_history().await?;
    Ok(Json(notifications))
}

/// Create a new notification
async fn create_notification(
    State(state): State<ApiState>,
    Json(req): Json<CreateNotificationRequest>,
) -> Result<(StatusCode, Json<Notification>), ApiError> {
    let notification = Notification::new(
        req.source,
        req.lifetime,
        req.priority,
        req.title,
        req.message,
        req.requires_response,
    );

    state.storage.add(&notification).await?;

    Ok((StatusCode::CREATED, Json(notification)))
}

/// Get a specific notification by ID
async fn get_notification(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Notification>, ApiError> {
    let notification = state
        .storage
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Notification {} not found", id)))?;

    Ok(Json(notification))
}

/// Update a notification
async fn update_notification(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateNotificationRequest>,
) -> Result<Json<Notification>, ApiError> {
    let mut notification = state
        .storage
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Notification {} not found", id)))?;

    // Apply updates
    if let Some(status) = req.status {
        notification.status = status;
    }
    if let Some(response) = req.response {
        notification.set_response(response)?;
    }

    state.storage.update(&notification).await?;

    Ok(Json(notification))
}

/// Delete a notification
async fn delete_notification(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.storage.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// === Request/Response Types ===

#[derive(Debug, Deserialize)]
struct ListParams {
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNotificationRequest {
    pub source: NotificationSource,
    pub lifetime: NotificationLifetime,
    pub priority: NotificationPriority,
    pub title: String,
    pub message: String,
    pub requires_response: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotificationRequest {
    pub status: Option<NotificationStatus>,
    pub response: Option<String>,
}

// === Error Handling ===

#[derive(Debug)]
pub enum ApiError {
    Database(anyhow::Error),
    NotFound(String),
    InvalidInput(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::Database(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
            }
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Database(err)
    }
}

impl std::str::FromStr for NotificationStatus {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(NotificationStatus::Pending),
            "viewed" => Ok(NotificationStatus::Viewed),
            "dismissed" => Ok(NotificationStatus::Dismissed),
            "responded" => Ok(NotificationStatus::Responded),
            "expired" => Ok(NotificationStatus::Expired),
            _ => Err(ApiError::InvalidInput(format!("Invalid status: {}", s))),
        }
    }
}
