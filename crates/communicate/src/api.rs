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
    CreateRoomRequest, PaginatedResponse, ParticipantResponse, RoomResponse, RoomType,
    UpdateRoomRequest,
};

pub use agentd_common::error::ApiError;

/// Shared application state injected into all route handlers.
#[derive(Clone)]
pub struct ApiState {
    pub storage: Arc<CommunicateStorage>,
}

/// Build the Axum router with all communicate API routes.
pub fn create_router(state: ApiState) -> Router {
    let rooms_router = Router::new()
        .route("/", post(create_room).get(list_rooms))
        .route("/{id}", get(get_room).put(update_room).delete(delete_room))
        .route("/{id}/participants", get(list_participants));

    Router::new().route("/health", get(health)).nest("/rooms", rooms_router).with_state(state)
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

/// `GET /rooms/{id}/participants` — list participants in a room.
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

        // First creation succeeds.
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

        // Second creation with same name returns 409.
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

        // Create 3 rooms.
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

        // Create room.
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .header("content-type", "application/json")
                    .body(create_room_body("to-delete"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::CREATED);
        let room_json = body_json(create_resp.into_body()).await;
        let id = room_json["id"].as_str().unwrap();

        // Delete room.
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

        // Verify gone.
        let get_resp = app
            .oneshot(Request::builder().uri(format!("/rooms/{id}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_room() {
        let (app, _temp) = build_test_app().await;

        // Create room.
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .header("content-type", "application/json")
                    .body(create_room_body("updateable"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::CREATED);
        let room_json = body_json(create_resp.into_body()).await;
        let id = room_json["id"].as_str().unwrap();

        // Update topic.
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
