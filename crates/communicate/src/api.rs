//! REST API router for the communicate service.
//!
//! # Endpoints
//!
//! - `GET /health` — liveness check
//! - `POST /rooms` — create a room (201 Created)
//! - `GET /rooms` — list rooms (paginated, optional `room_type` filter)
//! - `GET /rooms/{id}` — get room by ID
//! - `PUT /rooms/{id}` — update room topic/description
//! - `DELETE /rooms/{id}` — delete room
//! - `GET /rooms/{id}/participants` — list participants in a room (paginated)
//! - `POST /rooms/{id}/participants` — add a participant to a room (201 Created)
//! - `GET /rooms/{id}/participants/{identifier}` — get a specific participant
//! - `PUT /rooms/{id}/participants/{identifier}` — update participant role
//! - `DELETE /rooms/{id}/participants/{identifier}` — remove participant from room
//! - `GET /participants/{identifier}/rooms` — list all rooms for a participant
//! - `POST /rooms/{id}/messages` — send a message to a room (201 Created)
//! - `GET /rooms/{id}/messages` — list messages (paginated, with before/after filters)
//! - `GET /rooms/{id}/messages/latest` — get N most recent messages (default 50)
//! - `GET /messages/{id}` — get a specific message
//! - `DELETE /messages/{id}` — delete a message

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::storage::CommunicateStorage;
use crate::types::{
    AddParticipantRequest, CreateMessageRequest, CreateRoomRequest, MessageResponse,
    PaginatedResponse, ParticipantResponse, RoomResponse, RoomType, UpdateParticipantRoleRequest,
    UpdateRoomRequest,
};
use crate::websocket::{ws_handler, ConnectionManager, RoomEvent};

pub use agentd_common::error::ApiError;

/// Shared application state injected into all route handlers.
#[derive(Clone)]
pub struct ApiState {
    pub storage: Arc<CommunicateStorage>,
    pub connection_manager: Arc<ConnectionManager>,
}

/// Build the Axum router with all communicate API routes.
pub fn create_router(state: ApiState) -> Router {
    let participant_nested =
        Router::new().route("/", get(list_participants).post(add_participant)).route(
            "/{identifier}",
            get(get_participant).put(update_participant_role).delete(remove_participant),
        );

    let messages_nested = Router::new()
        .route("/", get(list_messages_in_room).post(send_message))
        .route("/latest", get(get_latest_messages));

    let rooms_router = Router::new()
        .route("/", post(create_room).get(list_rooms))
        .route("/{id}", get(get_room).put(update_room).delete(delete_room))
        .nest("/{id}/participants", participant_nested)
        .nest("/{id}/messages", messages_nested);

    let participants_router =
        Router::new().route("/{identifier}/rooms", get(list_rooms_for_participant));

    let messages_router = Router::new().route("/{id}", get(get_message).delete(delete_message));

    Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .nest("/rooms", rooms_router)
        .nest("/participants", participants_router)
        .nest("/messages", messages_router)
        .with_state(state)
}

/// `GET /health` — liveness check.
async fn health() -> Json<agentd_common::types::HealthResponse> {
    Json(agentd_common::types::HealthResponse::ok("agentd-communicate", env!("CARGO_PKG_VERSION")))
}

// ---------------------------------------------------------------------------
// Query parameter types
// ---------------------------------------------------------------------------

/// Query parameters for `GET /rooms`.
#[derive(Debug, Deserialize)]
struct ListRoomsParams {
    limit: Option<usize>,
    offset: Option<usize>,
    room_type: Option<String>,
}

/// Query parameters for paginated list endpoints.
#[derive(Debug, Deserialize)]
struct PaginationParams {
    limit: Option<usize>,
    offset: Option<usize>,
}

/// Query parameters for `GET /rooms/{id}/messages`.
#[derive(Debug, Deserialize)]
struct ListMessagesParams {
    limit: Option<usize>,
    offset: Option<usize>,
    /// RFC3339 timestamp — return only messages created before this time.
    before: Option<String>,
    /// RFC3339 timestamp — return only messages created after this time.
    after: Option<String>,
}

/// Query parameters for `GET /rooms/{id}/messages/latest`.
#[derive(Debug, Deserialize)]
struct LatestMessagesParams {
    count: Option<usize>,
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(50).min(200)
}

// ---------------------------------------------------------------------------
// Room handlers
// ---------------------------------------------------------------------------

/// `POST /rooms` — create a new room.
async fn create_room(
    State(state): State<ApiState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<(StatusCode, Json<RoomResponse>), ApiError> {
    let room = state.storage.create_room(&req).await?;

    metrics::counter!("rooms_created_total").increment(1);

    let active = state.storage.count_rooms().await?;
    metrics::gauge!("rooms_active").set(active as f64);

    Ok((StatusCode::CREATED, Json(RoomResponse::from(room))))
}

/// `GET /rooms` — list all rooms, with optional type filter and pagination.
async fn list_rooms(
    State(state): State<ApiState>,
    Query(params): Query<ListRoomsParams>,
) -> Result<Json<PaginatedResponse<RoomResponse>>, ApiError> {
    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let (rooms, total) = if let Some(type_str) = params.room_type {
        let rt = type_str.parse::<RoomType>().map_err(|e| ApiError::InvalidInput(e.to_string()))?;
        state.storage.list_rooms_by_type(&rt, limit, offset).await?
    } else {
        state.storage.list_rooms(limit, offset).await?
    };

    let items = rooms.into_iter().map(RoomResponse::from).collect();
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

/// `GET /rooms/{id}` — get a room by ID.
async fn get_room(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room = state.storage.get_room(&id).await?.ok_or(ApiError::NotFound)?;
    Ok(Json(RoomResponse::from(room)))
}

/// `PUT /rooms/{id}` — update a room's topic and/or description.
async fn update_room(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateRoomRequest>,
) -> Result<Json<RoomResponse>, ApiError> {
    let room = state
        .storage
        .update_room(&id, req.topic, req.description)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(RoomResponse::from(room)))
}

/// `DELETE /rooms/{id}` — delete a room.
async fn delete_room(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = state.storage.delete_room(&id).await?;
    if deleted {
        let active = state.storage.count_rooms().await?;
        metrics::gauge!("rooms_active").set(active as f64);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Participant handlers
// ---------------------------------------------------------------------------

/// `GET /rooms/{id}/participants` — list participants in a room (paginated).
async fn list_participants(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ParticipantResponse>>, ApiError> {
    // Verify room exists first.
    state.storage.get_room(&id).await?.ok_or(ApiError::NotFound)?;

    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let (participants, total) = state.storage.list_participants_in_room(&id, limit, offset).await?;

    let items = participants.into_iter().map(ParticipantResponse::from).collect();
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

/// `POST /rooms/{id}/participants` — add a participant to a room.
async fn add_participant(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddParticipantRequest>,
) -> Result<(StatusCode, Json<ParticipantResponse>), ApiError> {
    let participant = state.storage.add_participant(&id, &req).await?;

    let count = state.storage.count_participants_in_room(&id).await?;
    metrics::gauge!("participants_total", "room_id" => id.to_string()).set(count as f64);

    let response = ParticipantResponse::from(participant);

    // Broadcast participant joined event to WebSocket subscribers
    state
        .connection_manager
        .broadcast_to_room(id, RoomEvent::ParticipantJoined(response.clone()))
        .await;

    Ok((StatusCode::CREATED, Json(response)))
}

/// `GET /rooms/{id}/participants/{identifier}` — get a specific participant.
async fn get_participant(
    State(state): State<ApiState>,
    Path((id, identifier)): Path<(Uuid, String)>,
) -> Result<Json<ParticipantResponse>, ApiError> {
    let participant =
        state.storage.get_participant(&id, &identifier).await?.ok_or(ApiError::NotFound)?;
    Ok(Json(ParticipantResponse::from(participant)))
}

/// `PUT /rooms/{id}/participants/{identifier}` — update a participant's role.
async fn update_participant_role(
    State(state): State<ApiState>,
    Path((id, identifier)): Path<(Uuid, String)>,
    Json(req): Json<UpdateParticipantRoleRequest>,
) -> Result<Json<ParticipantResponse>, ApiError> {
    let participant = state
        .storage
        .update_participant_role(&id, &identifier, req.role)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(ParticipantResponse::from(participant)))
}

/// `DELETE /rooms/{id}/participants/{identifier}` — remove a participant from a room.
async fn remove_participant(
    State(state): State<ApiState>,
    Path((id, identifier)): Path<(Uuid, String)>,
) -> Result<StatusCode, ApiError> {
    // Verify room exists so we can distinguish 404-room vs 404-participant.
    state.storage.get_room(&id).await?.ok_or(ApiError::NotFound)?;

    let removed = state.storage.remove_participant(&id, &identifier).await?;
    if removed {
        let count = state.storage.count_participants_in_room(&id).await?;
        metrics::gauge!("participants_total", "room_id" => id.to_string()).set(count as f64);

        // Broadcast participant left event to WebSocket subscribers
        state
            .connection_manager
            .broadcast_to_room(
                id,
                RoomEvent::ParticipantLeft { room_id: id, identifier: identifier.clone() },
            )
            .await;

        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

/// `GET /participants/{identifier}/rooms` — list all rooms for a participant.
async fn list_rooms_for_participant(
    State(state): State<ApiState>,
    Path(identifier): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<RoomResponse>>, ApiError> {
    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let (rooms, total) =
        state.storage.get_rooms_for_participant(&identifier, limit, offset).await?;

    let items = rooms.into_iter().map(RoomResponse::from).collect();
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

// ---------------------------------------------------------------------------
// Message handlers
// ---------------------------------------------------------------------------

/// `POST /rooms/{id}/messages` — send a message to a room.
async fn send_message(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateMessageRequest>,
) -> Result<(StatusCode, Json<MessageResponse>), ApiError> {
    let message = state.storage.send_message(&id, &req).await?;

    metrics::counter!("messages_sent_total", "room_id" => id.to_string()).increment(1);

    let count = state.storage.get_room_message_count(&id).await?;
    metrics::histogram!("messages_per_room", "room_id" => id.to_string()).record(count as f64);

    let response = MessageResponse::from(message);

    // Broadcast message to WebSocket subscribers
    state.connection_manager.broadcast_to_room(id, RoomEvent::Message(response.clone())).await;

    Ok((StatusCode::CREATED, Json(response)))
}

/// `GET /rooms/{id}/messages` — list messages in a room (paginated, filterable).
async fn list_messages_in_room(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Query(params): Query<ListMessagesParams>,
) -> Result<Json<PaginatedResponse<MessageResponse>>, ApiError> {
    // Verify room exists.
    state.storage.get_room(&id).await?.ok_or(ApiError::NotFound)?;

    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    let before = params
        .before
        .as_deref()
        .map(|s| {
            s.parse::<chrono::DateTime<chrono::Utc>>()
                .map_err(|e| ApiError::InvalidInput(format!("invalid 'before' timestamp: {}", e)))
        })
        .transpose()?;

    let after = params
        .after
        .as_deref()
        .map(|s| {
            s.parse::<chrono::DateTime<chrono::Utc>>()
                .map_err(|e| ApiError::InvalidInput(format!("invalid 'after' timestamp: {}", e)))
        })
        .transpose()?;

    let (messages, total) = state.storage.list_messages(&id, limit, offset, before, after).await?;

    let items = messages.into_iter().map(MessageResponse::from).collect();
    Ok(Json(PaginatedResponse { items, total, limit, offset }))
}

/// `GET /rooms/{id}/messages/latest` — get the N most recent messages.
async fn get_latest_messages(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
    Query(params): Query<LatestMessagesParams>,
) -> Result<Json<Vec<MessageResponse>>, ApiError> {
    // Verify room exists.
    state.storage.get_room(&id).await?.ok_or(ApiError::NotFound)?;

    let count = params.count.unwrap_or(50).min(200);
    let messages = state.storage.get_latest_messages(&id, count).await?;

    Ok(Json(messages.into_iter().map(MessageResponse::from).collect()))
}

/// `GET /messages/{id}` — get a specific message by ID.
async fn get_message(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    let message = state.storage.get_message(&id).await?.ok_or(ApiError::NotFound)?;
    Ok(Json(MessageResponse::from(message)))
}

/// `DELETE /messages/{id}` — delete a message.
async fn delete_message(
    State(state): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = state.storage.delete_message(&id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tempfile::TempDir;
    use tower::ServiceExt;

    async fn build_test_app() -> (Router, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Arc::new(CommunicateStorage::with_path(&db_path).await.unwrap());
        let connection_manager = Arc::new(ConnectionManager::new());
        let state = ApiState { storage, connection_manager };
        (create_router(state), temp_dir)
    }

    async fn body_json(body: axum::body::Body) -> Value {
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn create_room_body(name: &str) -> Body {
        Body::from(
            serde_json::to_vec(&json!({
                "name": name,
                "created_by": "test-agent"
            }))
            .unwrap(),
        )
    }

    /// Creates a room and returns its ID string.
    async fn create_room_get_id(app: &Router, name: &str) -> String {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .header("content-type", "application/json")
                    .body(create_room_body(name))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        body_json(resp.into_body()).await["id"].as_str().unwrap().to_string()
    }

    /// Adds a participant and returns the response JSON.
    async fn add_participant_to_room(
        app: &Router,
        room_id: &str,
        identifier: &str,
        kind: &str,
    ) -> Value {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/participants"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "identifier": identifier,
                            "kind": kind,
                            "display_name": identifier
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        body_json(resp.into_body()).await
    }

    /// Sends a message and returns the response body.
    async fn send_msg(
        app: &Router,
        room_id: &str,
        sender_id: &str,
        content: &str,
    ) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "sender_id": sender_id,
                            "sender_name": sender_id,
                            "sender_kind": "agent",
                            "content": content
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    // -----------------------------------------------------------------------
    // Room tests (carried over)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_health_endpoint() {
        let (app, _temp) = build_test_app().await;

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response.into_body()).await;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "agentd-communicate");
    }

    #[tokio::test]
    async fn test_create_room_returns_201() {
        let (app, _temp) = build_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .header("content-type", "application/json")
                    .body(create_room_body("general"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let json = body_json(response.into_body()).await;
        assert_eq!(json["name"], "general");
        assert!(json["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_create_duplicate_name_returns_409() {
        let (app, _temp) = build_test_app().await;

        let resp1 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .header("content-type", "application/json")
                    .body(create_room_body("dup-room"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp1.status(), StatusCode::CREATED);

        let resp2 = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .header("content-type", "application/json")
                    .body(create_room_body("dup-room"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_get_nonexistent_room_returns_404() {
        let (app, _temp) = build_test_app().await;
        let missing_id = Uuid::new_v4();

        let response = app
            .oneshot(
                Request::builder().uri(format!("/rooms/{missing_id}")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_rooms_paginated() {
        let (app, _temp) = build_test_app().await;

        for name in ["alpha", "beta", "gamma"] {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/rooms")
                        .header("content-type", "application/json")
                        .body(create_room_body(name))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        let response = app
            .oneshot(Request::builder().uri("/rooms?limit=2&offset=0").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response.into_body()).await;
        assert_eq!(json["total"], 3);
        assert_eq!(json["items"].as_array().unwrap().len(), 2);
        assert_eq!(json["limit"], 2);
        assert_eq!(json["offset"], 0);
    }

    #[tokio::test]
    async fn test_delete_room() {
        let (app, _temp) = build_test_app().await;

        let id = create_room_get_id(&app, "to-delete").await;

        let del_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/rooms/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

        let get_resp = app
            .oneshot(Request::builder().uri(format!("/rooms/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_room() {
        let (app, _temp) = build_test_app().await;

        let id = create_room_get_id(&app, "updateable").await;

        let update_body =
            Body::from(serde_json::to_vec(&json!({ "topic": "Updated topic" })).unwrap());
        let update_resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/rooms/{id}"))
                    .header("content-type", "application/json")
                    .body(update_body)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update_resp.status(), StatusCode::OK);
        let updated = body_json(update_resp.into_body()).await;
        assert_eq!(updated["topic"], "Updated topic");
    }

    // -----------------------------------------------------------------------
    // Participant tests
    // -----------------------------------------------------------------------

    fn add_participant_body(identifier: &str, kind: &str, display_name: &str) -> Body {
        Body::from(
            serde_json::to_vec(&json!({
                "identifier": identifier,
                "kind": kind,
                "display_name": display_name
            }))
            .unwrap(),
        )
    }

    #[tokio::test]
    async fn test_add_agent_participant_returns_201() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "agent-room").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/participants"))
                    .header("content-type", "application/json")
                    .body(add_participant_body(
                        "550e8400-e29b-41d4-a716-446655440000",
                        "agent",
                        "Worker Agent",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp.into_body()).await;
        assert_eq!(json["identifier"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["kind"], "agent");
        assert_eq!(json["role"], "member");
    }

    #[tokio::test]
    async fn test_add_human_participant_returns_201() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "human-room").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/participants"))
                    .header("content-type", "application/json")
                    .body(add_participant_body("alice@example.com", "human", "Alice"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp.into_body()).await;
        assert_eq!(json["kind"], "human");
        assert_eq!(json["display_name"], "Alice");
    }

    #[tokio::test]
    async fn test_add_duplicate_participant_returns_409() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "dup-participant-room").await;

        let body1 = add_participant_body("agent-123", "agent", "Agent One");
        let resp1 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/participants"))
                    .header("content-type", "application/json")
                    .body(body1)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp1.status(), StatusCode::CREATED);

        let body2 = add_participant_body("agent-123", "agent", "Agent One Again");
        let resp2 = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/participants"))
                    .header("content-type", "application/json")
                    .body(body2)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_add_participant_to_missing_room_returns_404() {
        let (app, _temp) = build_test_app().await;
        let missing_id = Uuid::new_v4();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{missing_id}/participants"))
                    .header("content-type", "application/json")
                    .body(add_participant_body("agent-1", "agent", "Agent"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_remove_participant_returns_204() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "remove-room").await;

        // Add participant.
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/participants"))
                    .header("content-type", "application/json")
                    .body(add_participant_body("agent-del", "agent", "To Delete"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Remove participant.
        let del_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/rooms/{room_id}/participants/agent-del"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

        // Verify gone.
        let get_resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{room_id}/participants/agent-del"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_remove_nonexistent_participant_returns_404() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "no-member-room").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/rooms/{room_id}/participants/ghost"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_participant_role() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "role-room").await;

        // Add as member.
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/participants"))
                    .header("content-type", "application/json")
                    .body(add_participant_body("promote-me", "agent", "Promotable"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Promote to admin.
        let update_body = Body::from(serde_json::to_vec(&json!({ "role": "admin" })).unwrap());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/rooms/{room_id}/participants/promote-me"))
                    .header("content-type", "application/json")
                    .body(update_body)
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        assert_eq!(json["role"], "admin");
    }

    #[tokio::test]
    async fn test_list_rooms_for_participant() {
        let (app, _temp) = build_test_app().await;

        // Create 3 rooms and add the same participant to 2 of them.
        let room1 = create_room_get_id(&app, "participant-room-1").await;
        let room2 = create_room_get_id(&app, "participant-room-2").await;
        let _room3 = create_room_get_id(&app, "participant-room-3").await;

        for room_id in [&room1, &room2] {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/rooms/{room_id}/participants"))
                        .header("content-type", "application/json")
                        .body(add_participant_body("shared-agent", "agent", "Shared Agent"))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/participants/shared-agent/rooms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        assert_eq!(json["total"], 2);
        assert_eq!(json["items"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_observer_participant_included_in_listing() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "observer-room").await;

        // Add observer participant.
        let body = Body::from(
            serde_json::to_vec(&json!({
                "identifier": "observer-1",
                "kind": "human",
                "display_name": "Observer",
                "role": "observer"
            }))
            .unwrap(),
        );
        let add_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/participants"))
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(add_resp.status(), StatusCode::CREATED);

        // List participants — observer must appear.
        let list_resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{room_id}/participants"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let json = body_json(list_resp.into_body()).await;
        assert_eq!(json["total"], 1);
        assert_eq!(json["items"][0]["role"], "observer");
    }

    #[tokio::test]
    async fn test_list_participants_returns_404_for_missing_room() {
        let (app, _temp) = build_test_app().await;
        let missing_id = Uuid::new_v4();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{missing_id}/participants"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // -----------------------------------------------------------------------
    // Message tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_message_as_participant_returns_201() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "msg-room").await;
        add_participant_to_room(&app, &room_id, "agent-sender", "agent").await;

        let resp = send_msg(&app, &room_id, "agent-sender", "Hello, room!").await;
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = body_json(resp.into_body()).await;
        assert_eq!(json["content"], "Hello, room!");
        assert_eq!(json["sender_id"], "agent-sender");
        assert_eq!(json["status"], "sent");
        assert!(json["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_send_message_as_non_participant_returns_403() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "forbidden-room").await;
        // Do NOT add "outsider" as participant.

        let resp = send_msg(&app, &room_id, "outsider", "I shouldn't be here").await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_send_empty_message_returns_400() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "empty-content-room").await;
        add_participant_to_room(&app, &room_id, "sender", "agent").await;

        let resp = send_msg(&app, &room_id, "sender", "   ").await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_send_message_to_missing_room_returns_404() {
        let (app, _temp) = build_test_app().await;
        let missing_id = Uuid::new_v4();

        let resp = send_msg(&app, &missing_id.to_string(), "agent-1", "Hello").await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_message_by_id() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "get-msg-room").await;
        add_participant_to_room(&app, &room_id, "sender", "agent").await;

        let send_resp = send_msg(&app, &room_id, "sender", "Fetchable message").await;
        assert_eq!(send_resp.status(), StatusCode::CREATED);
        let msg_id = body_json(send_resp.into_body()).await["id"].as_str().unwrap().to_string();

        let get_resp = app
            .oneshot(
                Request::builder().uri(format!("/messages/{msg_id}")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::OK);
        let json = body_json(get_resp.into_body()).await;
        assert_eq!(json["id"], msg_id);
        assert_eq!(json["content"], "Fetchable message");
    }

    #[tokio::test]
    async fn test_delete_message_returns_204() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "del-msg-room").await;
        add_participant_to_room(&app, &room_id, "sender", "agent").await;

        let send_resp = send_msg(&app, &room_id, "sender", "Delete me").await;
        assert_eq!(send_resp.status(), StatusCode::CREATED);
        let msg_id = body_json(send_resp.into_body()).await["id"].as_str().unwrap().to_string();

        let del_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/messages/{msg_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

        // Verify it's gone.
        let get_resp = app
            .oneshot(
                Request::builder().uri(format!("/messages/{msg_id}")).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_message_returns_404() {
        let (app, _temp) = build_test_app().await;
        let missing_id = Uuid::new_v4();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/messages/{missing_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_messages_paginated() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "list-msg-room").await;
        add_participant_to_room(&app, &room_id, "sender", "agent").await;

        for i in 0..5 {
            let resp = send_msg(&app, &room_id, "sender", &format!("Message {i}")).await;
            assert_eq!(resp.status(), StatusCode::CREATED);
        }

        let list_resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{room_id}/messages?limit=3&offset=0"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let json = body_json(list_resp.into_body()).await;
        assert_eq!(json["total"], 5);
        assert_eq!(json["items"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_list_messages_returns_404_for_missing_room() {
        let (app, _temp) = build_test_app().await;
        let missing_id = Uuid::new_v4();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{missing_id}/messages"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_reply_to_creates_threading() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "thread-room").await;
        add_participant_to_room(&app, &room_id, "sender", "agent").await;

        // Send original message.
        let orig_resp = send_msg(&app, &room_id, "sender", "Original message").await;
        assert_eq!(orig_resp.status(), StatusCode::CREATED);
        let orig_id = body_json(orig_resp.into_body()).await["id"].as_str().unwrap().to_string();

        // Send reply.
        let reply_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "sender_id": "sender",
                            "sender_name": "Sender",
                            "sender_kind": "agent",
                            "content": "This is a reply",
                            "reply_to": orig_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(reply_resp.status(), StatusCode::CREATED);
        let reply_json = body_json(reply_resp.into_body()).await;
        assert_eq!(reply_json["reply_to"], orig_id);
        assert_eq!(reply_json["content"], "This is a reply");
    }

    #[tokio::test]
    async fn test_reply_to_wrong_room_returns_400() {
        let (app, _temp) = build_test_app().await;
        let room1 = create_room_get_id(&app, "room-a").await;
        let room2 = create_room_get_id(&app, "room-b").await;
        add_participant_to_room(&app, &room1, "sender", "agent").await;
        add_participant_to_room(&app, &room2, "sender", "agent").await;

        // Send a message in room1.
        let msg_resp = send_msg(&app, &room1, "sender", "Room-1 message").await;
        assert_eq!(msg_resp.status(), StatusCode::CREATED);
        let msg_id = body_json(msg_resp.into_body()).await["id"].as_str().unwrap().to_string();

        // Try to reply in room2 referencing the room1 message.
        let bad_reply = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room2}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "sender_id": "sender",
                            "sender_name": "Sender",
                            "sender_kind": "agent",
                            "content": "Bad reply",
                            "reply_to": msg_id
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(bad_reply.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_message_metadata_stored_and_returned() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "meta-room").await;
        add_participant_to_room(&app, &room_id, "sender", "agent").await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/rooms/{room_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "sender_id": "sender",
                            "sender_name": "Sender",
                            "sender_kind": "agent",
                            "content": "Message with metadata",
                            "metadata": {
                                "source": "workflow",
                                "dispatch_id": "wf-123"
                            }
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp.into_body()).await;
        assert_eq!(json["metadata"]["source"], "workflow");
        assert_eq!(json["metadata"]["dispatch_id"], "wf-123");
    }

    #[tokio::test]
    async fn test_get_latest_messages() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "latest-room").await;
        add_participant_to_room(&app, &room_id, "sender", "agent").await;

        for i in 0..10 {
            let resp = send_msg(&app, &room_id, "sender", &format!("Message {i:02}")).await;
            assert_eq!(resp.status(), StatusCode::CREATED);
        }

        let latest_resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{room_id}/messages/latest?count=3"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(latest_resp.status(), StatusCode::OK);
        let json = body_json(latest_resp.into_body()).await;
        let items = json.as_array().unwrap();
        assert_eq!(items.len(), 3);
        // Should be the 3 most recent (last 3 sent), returned oldest-first.
        assert_eq!(items[0]["content"], "Message 07");
        assert_eq!(items[1]["content"], "Message 08");
        assert_eq!(items[2]["content"], "Message 09");
    }

    #[tokio::test]
    async fn test_list_messages_with_before_after_filters() {
        let (app, _temp) = build_test_app().await;
        let room_id = create_room_get_id(&app, "filter-room").await;
        add_participant_to_room(&app, &room_id, "sender", "agent").await;

        // Use a fixed "before" anchor in the past — all messages will be after it.
        let before_anchor = "2099-01-01T00:00:00Z";
        let after_anchor = "2000-01-01T00:00:00Z";

        for i in 0..4 {
            let resp = send_msg(&app, &room_id, "sender", &format!("Msg {i}")).await;
            assert_eq!(resp.status(), StatusCode::CREATED);
        }

        // before=far future should include all messages.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{room_id}/messages?before={before_anchor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp.into_body()).await;
        assert_eq!(json["total"], 4);

        // after=far past should also include all messages.
        let resp2 = app
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{room_id}/messages?after={after_anchor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        let json2 = body_json(resp2.into_body()).await;
        assert_eq!(json2["total"], 4);
    }
}
