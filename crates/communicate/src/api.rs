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
    AddParticipantRequest, CreateRoomRequest, PaginatedResponse, ParticipantResponse, RoomResponse,
    RoomType, UpdateParticipantRoleRequest, UpdateRoomRequest,
};

pub use agentd_common::error::ApiError;

/// Shared application state injected into all route handlers.
#[derive(Clone)]
pub struct ApiState {
    pub storage: Arc<CommunicateStorage>,
}

/// Build the Axum router with all communicate API routes.
pub fn create_router(state: ApiState) -> Router {
    let participant_nested =
        Router::new().route("/", get(list_participants).post(add_participant)).route(
            "/{identifier}",
            get(get_participant).put(update_participant_role).delete(remove_participant),
        );

    let rooms_router = Router::new()
        .route("/", post(create_room).get(list_rooms))
        .route("/{id}", get(get_room).put(update_room).delete(delete_room))
        .nest("/{id}/participants", participant_nested);

    let participants_router =
        Router::new().route("/{identifier}/rooms", get(list_rooms_for_participant));

    Router::new()
        .route("/health", get(health))
        .nest("/rooms", rooms_router)
        .nest("/participants", participants_router)
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

    Ok((StatusCode::CREATED, Json(ParticipantResponse::from(participant))))
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
        let state = ApiState { storage };
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
}
