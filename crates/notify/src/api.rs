//! REST API handlers for the notification service.
//!
//! This module provides HTTP endpoints for managing notifications through a RESTful
//! API. It uses the Axum web framework to expose CRUD operations and specialized
//! queries for notifications.
//!
//! # API Endpoints
//!
//! - `GET /health` - Health check endpoint
//! - `GET /notifications` - List all notifications (with optional status filter)
//! - `POST /notifications` - Create a new notification
//! - `GET /notifications/{id}` - Get a specific notification by ID
//! - `PUT /notifications/{id}` - Update a notification
//! - `DELETE /notifications/{id}` - Delete a notification
//! - `GET /notifications/actionable` - List actionable notifications
//! - `GET /notifications/history` - List notification history
//!
//! # Examples
//!
//! ## Creating a Router
//!
//! ```no_run
//! use notify::api::{create_router, ApiState};
//! use notify::storage::NotificationStorage;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let storage = NotificationStorage::new().await?;
//!     let state = ApiState {
//!         storage: Arc::new(storage),
//!     };
//!     let router = create_router(state);
//!
//!     // Bind and serve
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:17004").await?;
//!     axum::serve(listener, router).await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Making Requests
//!
//! ```bash
//! # Health check
//! curl http://localhost:17004/health
//!
//! # Create notification
//! curl -X POST http://localhost:17004/notifications \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "source": "System",
//!     "lifetime": "Persistent",
//!     "priority": "High",
//!     "title": "Test Notification",
//!     "message": "This is a test",
//!     "requires_response": false
//!   }'
//!
//! # List notifications
//! curl http://localhost:17004/notifications
//!
//! # Filter by status
//! curl "http://localhost:17004/notifications?status=pending"
//!
//! # Get actionable notifications
//! curl http://localhost:17004/notifications/actionable
//! ```

use crate::{storage::NotificationStorage, types::*};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Shared state passed to all API handlers.
///
/// Contains the notification storage backend wrapped in an [`Arc`] for
/// efficient cloning across async handlers.
///
/// # Examples
///
/// ```no_run
/// use notify::api::ApiState;
/// use notify::storage::NotificationStorage;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let storage = NotificationStorage::new().await?;
///     let state = ApiState {
///         storage: Arc::new(storage),
///     };
///     // Use state with router...
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct ApiState {
    /// Shared notification storage backend
    pub storage: Arc<NotificationStorage>,
}

/// Creates and configures the Axum router with all API endpoints.
///
/// Sets up all HTTP routes and attaches the shared state. The router is
/// ready to be served by Axum's `serve` function.
///
/// # Arguments
///
/// * `state` - The API state containing the storage backend
///
/// # Returns
///
/// Returns a configured [`Router`] ready to serve HTTP requests.
///
/// # Examples
///
/// ```no_run
/// use notify::api::{create_router, ApiState};
/// use notify::storage::NotificationStorage;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let storage = NotificationStorage::new().await?;
///     let state = ApiState {
///         storage: Arc::new(storage),
///     };
///     let router = create_router(state);
///
///     let listener = tokio::net::TcpListener::bind("127.0.0.1:17004").await?;
///     axum::serve(listener, router).await?;
///     Ok(())
/// }
/// ```
pub fn create_router(state: ApiState) -> Router {
    let notifications_router = Router::new()
        .route("/", axum::routing::get(list_notifications).post(create_notification))
        .route("/count", axum::routing::get(count_notifications))
        .route("/actionable", axum::routing::get(list_actionable))
        .route("/history", axum::routing::get(list_history))
        .route(
            "/{id}",
            axum::routing::get(get_notification)
                .put(update_notification)
                .delete(delete_notification),
        );

    Router::new()
        .route("/health", axum::routing::get(health_check))
        .nest("/notifications", notifications_router)
        .with_state(state)
}

/// Health check endpoint handler.
///
/// Returns basic service information including status and version.
/// This endpoint is useful for monitoring and load balancer health checks.
///
/// # Endpoint
///
/// `GET /health`
///
/// # Response
///
/// Returns HTTP 200 with JSON body:
/// ```json
/// {
///   "status": "ok",
///   "service": "agentd-notify",
///   "version": "0.1.0"
/// }
/// ```
///
/// # Examples
///
/// ```bash
/// curl http://localhost:17004/health
/// ```
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "agentd-notify",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Lists all notifications with optional status filter.
///
/// Returns all notifications from storage, optionally filtered by status.
/// Results are ordered by creation time (newest first).
///
/// # Endpoint
///
/// `GET /notifications?status=<status>`
///
/// # Query Parameters
///
/// - `status` (optional) - Filter by status. Valid values: `pending`, `viewed`,
///   `dismissed`, `responded`, `expired` (case-insensitive)
///
/// # Response
///
/// Returns HTTP 200 with JSON array of notifications.
///
/// # Errors
///
/// - HTTP 400 - Invalid status parameter
/// - HTTP 500 - Database error
///
/// # Examples
///
/// ```bash
/// # Get all notifications
/// curl http://localhost:17004/notifications
///
/// # Get only pending notifications
/// curl "http://localhost:17004/notifications?status=pending"
/// ```
async fn list_notifications(
    State(state): State<ApiState>,
    axum::extract::Query(params): axum::extract::Query<ListParams>,
) -> Result<Json<PaginatedResponse<Notification>>, ApiError> {
    let status = params
        .status
        .map(|s| s.parse::<NotificationStatus>().map_err(|e| ApiError::InvalidInput(e.to_string())))
        .transpose()?;
    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let (items, total) = state.storage.list_paginated(status, limit, offset).await?;
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

/// Lists actionable notifications.
///
/// Returns notifications that can still be acted upon (Pending or Viewed status
/// and not expired). Results are ordered by priority (highest first), then by
/// creation time (oldest first).
///
/// # Endpoint
///
/// `GET /notifications/actionable`
///
/// # Response
///
/// Returns HTTP 200 with JSON array of actionable notifications.
///
/// # Errors
///
/// - HTTP 500 - Database error
///
/// # Examples
///
/// ```bash
/// curl http://localhost:17004/notifications/actionable
/// ```
async fn list_actionable(
    State(state): State<ApiState>,
    axum::extract::Query(params): axum::extract::Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Notification>>, ApiError> {
    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let (items, total) = state.storage.list_actionable_paginated(limit, offset).await?;
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

/// Lists notification history.
///
/// Returns notifications that are no longer actionable (Dismissed, Responded,
/// or Expired status). Results are ordered by update time (newest first).
///
/// # Endpoint
///
/// `GET /notifications/history`
///
/// # Response
///
/// Returns HTTP 200 with JSON array of historical notifications.
///
/// # Errors
///
/// - HTTP 500 - Database error
///
/// # Examples
///
/// ```bash
/// curl http://localhost:17004/notifications/history
/// ```
async fn list_history(
    State(state): State<ApiState>,
    axum::extract::Query(params): axum::extract::Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Notification>>, ApiError> {
    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let (items, total) = state.storage.list_history_paginated(limit, offset).await?;
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

/// Returns the count of notifications grouped by status.
///
/// Provides statistics about how many notifications exist in each status category.
/// Useful for dashboard displays and monitoring notification load.
///
/// # Endpoint
///
/// `GET /notifications/count`
///
/// # Response
///
/// Returns HTTP 200 with JSON object containing:
/// - `total` - Total number of notifications
/// - `by_status` - Array of status counts with status name and count
///
/// # Example
///
/// ```bash
/// curl http://localhost:17004/notifications/count
/// ```
///
/// Response:
/// ```json
/// {
///   "total": 42,
///   "by_status": [
///     {"status": "pending", "count": 15},
///     {"status": "viewed", "count": 10},
///     {"status": "responded", "count": 12},
///     {"status": "dismissed", "count": 5}
///   ]
/// }
/// ```
async fn count_notifications(
    State(state): State<ApiState>,
) -> Result<Json<CountResponse>, ApiError> {
    let counts = state.storage.count().await?;

    let total: usize = counts.iter().map(|(_, count)| count).sum();
    let by_status: Vec<StatusCount> = counts
        .into_iter()
        .map(|(status, count)| StatusCount { status: format!("{status:?}").to_lowercase(), count })
        .collect();

    Ok(Json(CountResponse { total, by_status }))
}

/// Creates a new notification.
///
/// Accepts a JSON request body and creates a new notification in storage.
/// The notification ID is automatically generated.
///
/// # Endpoint
///
/// `POST /notifications`
///
/// # Request Body
///
/// JSON object with the following fields:
/// - `source` - Notification source (System, AgentHook, AskService, MonitorService)
/// - `lifetime` - Lifetime type (Persistent or Ephemeral with expires_at)
/// - `priority` - Priority level (Low, Normal, High, Urgent)
/// - `title` - Notification title (string)
/// - `message` - Notification message (string)
/// - `requires_response` - Whether response is required (boolean)
///
/// # Response
///
/// Returns HTTP 201 with the created notification as JSON.
///
/// # Errors
///
/// - HTTP 400 - Invalid request body
/// - HTTP 500 - Database error
///
/// # Examples
///
/// ```bash
/// curl -X POST http://localhost:17004/notifications \
///   -H "Content-Type: application/json" \
///   -d '{
///     "source": "System",
///     "lifetime": "Persistent",
///     "priority": "High",
///     "title": "Important Update",
///     "message": "Please review the changes",
///     "requires_response": false
///   }'
/// ```
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

/// Gets a specific notification by ID.
///
/// Retrieves a single notification from storage by its UUID.
///
/// # Endpoint
///
/// `GET /notifications/{id}`
///
/// # Path Parameters
///
/// - `id` - UUID of the notification
///
/// # Response
///
/// Returns HTTP 200 with the notification as JSON.
///
/// # Errors
///
/// - HTTP 404 - Notification not found
/// - HTTP 500 - Database error
///
/// # Examples
///
/// ```bash
/// curl http://localhost:17004/notifications/550e8400-e29b-41d4-a716-446655440000
/// ```
async fn get_notification(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Notification>, ApiError> {
    let notification = state
        .storage
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Notification {id} not found")))?;

    Ok(Json(notification))
}

/// Updates a notification.
///
/// Updates the status and/or response of an existing notification.
/// Only the `status` and `response` fields can be modified.
///
/// # Endpoint
///
/// `PUT /notifications/{id}`
///
/// # Path Parameters
///
/// - `id` - UUID of the notification to update
///
/// # Request Body
///
/// JSON object with optional fields:
/// - `status` (optional) - New status (Pending, Viewed, Dismissed, Responded, Expired)
/// - `response` (optional) - User's response text (only for notifications requiring response)
///
/// # Response
///
/// Returns HTTP 200 with the updated notification as JSON.
///
/// # Errors
///
/// - HTTP 400 - Invalid request (e.g., setting response on notification that doesn't require it)
/// - HTTP 404 - Notification not found
/// - HTTP 500 - Database error
///
/// # Examples
///
/// ```bash
/// # Dismiss a notification
/// curl -X PUT http://localhost:17004/notifications/550e8400-e29b-41d4-a716-446655440000 \
///   -H "Content-Type: application/json" \
///   -d '{"status": "Dismissed"}'
///
/// # Respond to a notification
/// curl -X PUT http://localhost:17004/notifications/550e8400-e29b-41d4-a716-446655440000 \
///   -H "Content-Type: application/json" \
///   -d '{"response": "Approved"}'
/// ```
async fn update_notification(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateNotificationRequest>,
) -> Result<Json<Notification>, ApiError> {
    let mut notification = state
        .storage
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Notification {id} not found")))?;

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

/// Deletes a notification.
///
/// Permanently removes a notification from storage. This operation cannot be undone.
///
/// # Endpoint
///
/// `DELETE /notifications/{id}`
///
/// # Path Parameters
///
/// - `id` - UUID of the notification to delete
///
/// # Response
///
/// Returns HTTP 204 (No Content) on success.
///
/// # Errors
///
/// - HTTP 404 - Notification not found
/// - HTTP 500 - Database error
///
/// # Examples
///
/// ```bash
/// curl -X DELETE http://localhost:17004/notifications/550e8400-e29b-41d4-a716-446655440000
/// ```
async fn delete_notification(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.storage.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// === Request/Response Types ===

/// Query parameters for listing notifications.
///
/// Used by the `GET /notifications` endpoint to filter results by status.
#[derive(Debug, Deserialize)]
struct ListParams {
    /// Optional status filter (case-insensitive)
    status: Option<String>,
    /// Maximum number of items to return (default: 50, max: 200)
    limit: Option<usize>,
    /// Number of items to skip (default: 0)
    offset: Option<usize>,
}

/// Pagination query parameters for endpoints without other filters.
#[derive(Debug, Deserialize)]
struct PaginationParams {
    /// Maximum number of items to return (default: 50, max: 200)
    limit: Option<usize>,
    /// Number of items to skip (default: 0)
    offset: Option<usize>,
}

// === Error Handling ===

/// API error types that can be returned from handlers.
///
/// These errors are automatically converted to appropriate HTTP responses
/// with status codes and JSON error messages.
///
/// # HTTP Status Mapping
///
/// - `Database` -> 500 Internal Server Error
/// - `NotFound` -> 404 Not Found
/// - `InvalidInput` -> 400 Bad Request
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Database operation error (HTTP 500)
    #[error("database error: {0}")]
    Database(#[from] anyhow::Error),
    /// Resource not found error (HTTP 404)
    #[error("not found: {0}")]
    NotFound(String),
    /// Invalid input or request error (HTTP 400)
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::InvalidInput(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
