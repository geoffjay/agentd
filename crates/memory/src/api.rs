//! REST API handlers for the agentd-memory service.
//!
//! This module provides HTTP endpoints for managing memories through a RESTful
//! API backed by a [`VectorStore`] for semantic search and CRUD operations.
//!
//! # API Endpoints
//!
//! | Method | Path                       | Description                     |
//! |--------|----------------------------|---------------------------------|
//! | `GET`  | `/health`                  | Service health check            |
//! | `POST` | `/memories`                | Create a new memory             |
//! | `GET`  | `/memories`                | List memories (with filters)    |
//! | `GET`  | `/memories/:id`            | Retrieve a memory by ID         |
//! | `DELETE`| `/memories/:id`           | Delete a memory                 |
//! | `PUT`  | `/memories/:id/visibility` | Update visibility & share list  |
//! | `POST` | `/memories/search`         | Semantic similarity search      |
//!
//! # Examples
//!
//! ## Creating a Router
//!
//! ```rust,no_run
//! use memory::api::{create_router, ApiState};
//! use memory::store::VectorStore;
//! use std::sync::Arc;
//!
//! # async fn example(store: Arc<dyn VectorStore>) {
//! let state = ApiState { store };
//! let router = create_router(state);
//! # }
//! ```
//!
//! ## Making Requests
//!
//! ```bash
//! # Health check
//! curl http://localhost:17008/health
//!
//! # Create a memory
//! curl -X POST http://localhost:17008/memories \
//!   -H "Content-Type: application/json" \
//!   -d '{"content": "Paris is the capital of France.", "created_by": "agent-1"}'
//!
//! # Semantic search
//! curl -X POST http://localhost:17008/memories/search \
//!   -H "Content-Type: application/json" \
//!   -d '{"query": "capital of France", "limit": 5}'
//!
//! # Update visibility
//! curl -X PUT http://localhost:17008/memories/mem_123_abc/visibility \
//!   -H "Content-Type: application/json" \
//!   -d '{"visibility": "shared", "shared_with": ["agent-2"]}'
//! ```

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, info, warn};

use memory::store::VectorStore;
use memory::types::{
    CreateMemoryRequest, DeleteResponse, Memory, MemoryType, SearchRequest, SearchResponse,
    UpdateVisibilityRequest, VisibilityLevel,
};

pub use agentd_common::error::ApiError;
pub use agentd_common::types::{clamp_limit, PaginatedResponse};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Shared state passed to all API handlers.
///
/// Contains the vector store backend wrapped in an [`Arc`] for efficient
/// cloning across async handlers.
///
/// # Examples
///
/// ```rust,no_run
/// use memory::api::ApiState;
/// use memory::store::VectorStore;
/// use std::sync::Arc;
///
/// # async fn example(store: Arc<dyn VectorStore>) {
/// let state = ApiState { store };
/// # }
/// ```
#[derive(Clone)]
pub struct ApiState {
    /// Shared vector store backend for memory persistence and search.
    pub store: Arc<dyn VectorStore>,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Create and configure the Axum router with all memory API endpoints.
///
/// Sets up all HTTP routes and attaches the shared state. The router is
/// ready to be served by Axum's `serve` function.
///
/// # Routes
///
/// - `GET /health` — service health check (DB + LanceDB status)
/// - `POST /memories` — create a new memory
/// - `GET /memories` — list memories with optional filters
/// - `GET /memories/:id` — retrieve a single memory
/// - `DELETE /memories/:id` — delete a memory
/// - `PUT /memories/:id/visibility` — update visibility level
/// - `POST /memories/search` — semantic similarity search
pub fn create_router(state: ApiState) -> Router {
    let memories_router = Router::new()
        .route("/", axum::routing::get(list_memories).post(create_memory))
        .route("/search", axum::routing::post(search_memories))
        .route(
            "/{id}",
            axum::routing::get(get_memory).delete(delete_memory),
        )
        .route("/{id}/visibility", axum::routing::put(update_visibility));

    Router::new()
        .route("/health", axum::routing::get(health_check))
        .nest("/memories", memories_router)
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

/// `GET /health` — service health check.
///
/// Reports the overall health of the memory service including the vector
/// store backend status.
///
/// # Response
///
/// Returns HTTP 200 with JSON body:
/// ```json
/// {
///   "status": "ok",
///   "service": "agentd-memory",
///   "version": "0.2.0",
///   "details": { "vector_store": true }
/// }
/// ```
async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    let mut resp = agentd_common::types::HealthResponse::ok(
        "agentd-memory",
        env!("CARGO_PKG_VERSION"),
    );

    match state.store.health_check().await {
        Ok(healthy) => {
            resp = resp.with_detail("vector_store", serde_json::json!(healthy));
        }
        Err(e) => {
            warn!("Vector store health check failed: {}", e);
            resp = resp.with_detail("vector_store", serde_json::json!(false));
            resp = resp.with_detail("vector_store_error", serde_json::json!(e.to_string()));
        }
    }

    Json(resp)
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

/// `POST /memories` — create a new memory record.
///
/// Generates an embedding from the content, stores the record in the vector
/// database, and returns the fully-populated memory.
///
/// # Request Body
///
/// ```json
/// {
///   "content": "Paris is the capital of France.",
///   "type": "information",
///   "tags": ["geography"],
///   "created_by": "agent-1",
///   "visibility": "public"
/// }
/// ```
///
/// Only `content` and `created_by` are required; all other fields have
/// sensible defaults.
///
/// # Response
///
/// Returns HTTP 201 with the created [`Memory`] as JSON.
///
/// # Errors
///
/// - HTTP 400 — empty content or missing `created_by`
/// - HTTP 500 — embedding or storage failure
async fn create_memory(
    State(state): State<ApiState>,
    Json(req): Json<CreateMemoryRequest>,
) -> Result<(StatusCode, Json<Memory>), ApiError> {
    // Validate required fields
    if req.content.trim().is_empty() {
        return Err(ApiError::InvalidInput(
            "content must not be empty".to_string(),
        ));
    }
    if req.created_by.trim().is_empty() {
        return Err(ApiError::InvalidInput(
            "created_by must not be empty".to_string(),
        ));
    }

    debug!(
        created_by = %req.created_by,
        memory_type = %req.memory_type,
        tags = ?req.tags,
        "Creating memory"
    );

    let memory = state.store.create(req).await.map_err(ApiError::from)?;

    metrics::counter!(
        "memories_created_total",
        "type" => memory.memory_type.to_string(),
        "visibility" => memory.visibility.to_string()
    )
    .increment(1);

    info!(id = %memory.id, "Memory created");
    Ok((StatusCode::CREATED, Json(memory)))
}

// ---------------------------------------------------------------------------
// Get
// ---------------------------------------------------------------------------

/// `GET /memories/:id` — retrieve a single memory by ID.
///
/// # Path Parameters
///
/// - `id` — memory ID (e.g. `mem_1234567890_abcdef01`)
///
/// # Response
///
/// Returns HTTP 200 with the [`Memory`] as JSON.
///
/// # Errors
///
/// - HTTP 404 — memory not found
async fn get_memory(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Memory>, ApiError> {
    let memory = state
        .store
        .get(&id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(memory))
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

/// Query parameters for `GET /memories`.
#[derive(Debug, Deserialize)]
struct ListParams {
    /// Filter by memory type (`information`, `question`, `request`).
    #[serde(rename = "type")]
    memory_type: Option<String>,

    /// Filter by tag (comma-separated for multiple).
    tag: Option<String>,

    /// Filter by creator identity.
    created_by: Option<String>,

    /// Filter by visibility level.
    visibility: Option<String>,

    /// Maximum number of items to return (default: 50, max: 200).
    limit: Option<usize>,

    /// Number of items to skip (default: 0).
    offset: Option<usize>,
}

/// `GET /memories` — list memories with optional filters.
///
/// # Query Parameters
///
/// - `type` — filter by memory type (`information`, `question`, `request`)
/// - `tag` — filter by tag (comma-separated)
/// - `created_by` — filter by creator
/// - `visibility` — filter by visibility level
/// - `limit` — max items per page (default 50, max 200)
/// - `offset` — pagination offset (default 0)
///
/// # Response
///
/// Returns HTTP 200 with [`PaginatedResponse<Memory>`].
async fn list_memories(
    State(state): State<ApiState>,
    Query(params): Query<ListParams>,
) -> Result<Json<PaginatedResponse<Memory>>, ApiError> {
    let limit = clamp_limit(params.limit);
    let offset = params.offset.unwrap_or(0);

    // Parse optional filters
    let memory_type_filter = params
        .memory_type
        .map(|s| s.parse::<MemoryType>().map_err(|e| ApiError::InvalidInput(e)))
        .transpose()?;

    let visibility_filter = params
        .visibility
        .map(|s| {
            s.parse::<VisibilityLevel>()
                .map_err(|e| ApiError::InvalidInput(e))
        })
        .transpose()?;

    let tag_filter: Vec<String> = params
        .tag
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    // Fetch all and apply filters in-memory.
    // (A future MemoryStorage with SQL will support server-side filtering.)
    let all = state.store.list_all().await.map_err(ApiError::from)?;

    let filtered: Vec<Memory> = all
        .into_iter()
        .filter(|m| {
            memory_type_filter
                .as_ref()
                .map_or(true, |t| m.memory_type == *t)
        })
        .filter(|m| {
            visibility_filter
                .as_ref()
                .map_or(true, |v| m.visibility == *v)
        })
        .filter(|m| {
            tag_filter.is_empty() || tag_filter.iter().any(|t| m.tags.contains(t))
        })
        .filter(|m| {
            params
                .created_by
                .as_ref()
                .map_or(true, |c| m.created_by == *c)
        })
        .collect();

    let total = filtered.len();
    let items: Vec<Memory> = filtered.into_iter().skip(offset).take(limit).collect();

    Ok(Json(PaginatedResponse {
        items,
        total,
        limit,
        offset,
    }))
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

/// `DELETE /memories/:id` — delete a memory by ID.
///
/// # Path Parameters
///
/// - `id` — memory ID
///
/// # Response
///
/// Returns HTTP 200 with [`DeleteResponse`] indicating whether a record was
/// actually removed.
///
/// # Errors
///
/// - HTTP 500 — storage failure
async fn delete_memory(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, ApiError> {
    let deleted = state.store.delete(&id).await.map_err(ApiError::from)?;

    if deleted {
        metrics::counter!("memories_deleted_total").increment(1);
        info!(id = %id, "Memory deleted");
    }

    Ok(Json(DeleteResponse { deleted }))
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// `POST /memories/search` — semantic similarity search.
///
/// Embeds the query text and performs a vector similarity search, filtering
/// by visibility, tags, type, and date range.
///
/// # Request Body
///
/// ```json
/// {
///   "query": "capital of France",
///   "as_actor": "agent-1",
///   "type": "information",
///   "tags": ["geography"],
///   "limit": 5
/// }
/// ```
///
/// Only `query` is required.
///
/// # Response
///
/// Returns HTTP 200 with [`SearchResponse`] containing matching memories
/// ordered by similarity.
///
/// # Errors
///
/// - HTTP 400 — empty query
/// - HTTP 500 — embedding or search failure
async fn search_memories(
    State(state): State<ApiState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, ApiError> {
    if req.query.trim().is_empty() {
        return Err(ApiError::InvalidInput(
            "query must not be empty".to_string(),
        ));
    }

    debug!(
        query = %req.query,
        as_actor = ?req.as_actor,
        limit = req.limit,
        "Searching memories"
    );

    let memories = state.store.search(req).await.map_err(ApiError::from)?;
    let total = memories.len();

    metrics::counter!("memories_searched_total").increment(1);

    Ok(Json(SearchResponse { memories, total }))
}

// ---------------------------------------------------------------------------
// Update visibility
// ---------------------------------------------------------------------------

/// `PUT /memories/:id/visibility` — update visibility and share list.
///
/// # Path Parameters
///
/// - `id` — memory ID
///
/// # Request Body
///
/// ```json
/// {
///   "visibility": "shared",
///   "shared_with": ["agent-2", "agent-3"]
/// }
/// ```
///
/// # Response
///
/// Returns HTTP 200 with the updated [`Memory`].
///
/// # Errors
///
/// - HTTP 400 — invalid visibility level
/// - HTTP 404 — memory not found
/// - HTTP 500 — storage failure
async fn update_visibility(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateVisibilityRequest>,
) -> Result<Json<Memory>, ApiError> {
    debug!(id = %id, visibility = %req.visibility, "Updating memory visibility");

    let memory = state
        .store
        .update_visibility(&id, req.visibility, req.shared_with)
        .await
        .map_err(ApiError::from)?;

    info!(id = %id, visibility = %memory.visibility, "Visibility updated");
    Ok(Json(memory))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use memory::error::{StoreError, StoreResult};
    use memory::types::CreateMemoryRequest;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::{self, Request};
    use chrono::Utc;
    use tower::ServiceExt;

    // ── Mock vector store ─────────────────────────────────────────────────

    /// In-memory mock of [`VectorStore`] for API handler tests.
    struct MockVectorStore {
        memories: tokio::sync::RwLock<Vec<Memory>>,
    }

    impl MockVectorStore {
        fn new() -> Self {
            Self {
                memories: tokio::sync::RwLock::new(Vec::new()),
            }
        }

        fn with_memories(memories: Vec<Memory>) -> Self {
            Self {
                memories: tokio::sync::RwLock::new(memories),
            }
        }
    }

    #[async_trait]
    impl VectorStore for MockVectorStore {
        async fn initialize(&self) -> StoreResult<()> {
            Ok(())
        }

        async fn create(&self, req: CreateMemoryRequest) -> StoreResult<Memory> {
            let now = Utc::now();
            let memory = Memory {
                id: Memory::generate_id(),
                content: req.content,
                memory_type: req.memory_type,
                tags: req.tags,
                created_by: req.created_by,
                created_at: now,
                updated_at: now,
                owner: None,
                visibility: req.visibility,
                shared_with: req.shared_with,
                references: req.references,
            };
            self.memories.write().await.push(memory.clone());
            Ok(memory)
        }

        async fn get(&self, id: &str) -> StoreResult<Option<Memory>> {
            let memories = self.memories.read().await;
            Ok(memories.iter().find(|m| m.id == id).cloned())
        }

        async fn delete(&self, id: &str) -> StoreResult<bool> {
            let mut memories = self.memories.write().await;
            let len_before = memories.len();
            memories.retain(|m| m.id != id);
            Ok(memories.len() < len_before)
        }

        async fn search(&self, req: SearchRequest) -> StoreResult<Vec<Memory>> {
            let memories = self.memories.read().await;
            let results: Vec<Memory> = memories
                .iter()
                .filter(|m| m.content.contains(&req.query))
                .take(req.limit)
                .cloned()
                .collect();
            Ok(results)
        }

        async fn update_visibility(
            &self,
            id: &str,
            visibility: VisibilityLevel,
            shared_with: Option<Vec<String>>,
        ) -> StoreResult<Memory> {
            let mut memories = self.memories.write().await;
            let memory = memories
                .iter_mut()
                .find(|m| m.id == id)
                .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
            memory.visibility = visibility;
            if let Some(shared) = shared_with {
                memory.shared_with = shared;
            }
            memory.updated_at = Utc::now();
            Ok(memory.clone())
        }

        async fn health_check(&self) -> StoreResult<bool> {
            Ok(true)
        }

        async fn list_all(&self) -> StoreResult<Vec<Memory>> {
            Ok(self.memories.read().await.clone())
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn test_app(store: Arc<dyn VectorStore>) -> Router {
        create_router(ApiState { store })
    }

    fn sample_memory(id: &str, content: &str) -> Memory {
        let now = Utc::now();
        Memory {
            id: id.to_string(),
            content: content.to_string(),
            memory_type: MemoryType::Information,
            tags: vec!["test".to_string()],
            created_by: "agent-1".to_string(),
            created_at: now,
            updated_at: now,
            owner: None,
            visibility: VisibilityLevel::Public,
            shared_with: vec![],
            references: vec![],
        }
    }

    // ── Health check ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_health_check_returns_ok() {
        let app = test_app(Arc::new(MockVectorStore::new()));
        let resp = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "agentd-memory");
        assert_eq!(json["details"]["vector_store"], true);
    }

    // ── Create ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_memory_returns_201() {
        let app = test_app(Arc::new(MockVectorStore::new()));
        let body = serde_json::json!({
            "content": "Paris is the capital of France.",
            "created_by": "agent-1"
        });

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/memories")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["content"], "Paris is the capital of France.");
        assert!(json["id"].as_str().unwrap().starts_with("mem_"));
    }

    #[tokio::test]
    async fn test_create_memory_empty_content_returns_400() {
        let app = test_app(Arc::new(MockVectorStore::new()));
        let body = serde_json::json!({
            "content": "",
            "created_by": "agent-1"
        });

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/memories")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_create_memory_empty_created_by_returns_400() {
        let app = test_app(Arc::new(MockVectorStore::new()));
        let body = serde_json::json!({
            "content": "some content",
            "created_by": "  "
        });

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/memories")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // ── Get ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_memory_returns_200() {
        let mem = sample_memory("mem_1_abcdef01", "test content");
        let app = test_app(Arc::new(MockVectorStore::with_memories(vec![mem])));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/memories/mem_1_abcdef01")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["id"], "mem_1_abcdef01");
    }

    #[tokio::test]
    async fn test_get_memory_not_found_returns_404() {
        let app = test_app(Arc::new(MockVectorStore::new()));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/memories/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── List ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_memories_returns_paginated() {
        let memories = vec![
            sample_memory("mem_1_aaa", "first"),
            sample_memory("mem_2_bbb", "second"),
            sample_memory("mem_3_ccc", "third"),
        ];
        let app = test_app(Arc::new(MockVectorStore::with_memories(memories)));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/memories?limit=2&offset=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 3);
        assert_eq!(json["limit"], 2);
        assert_eq!(json["offset"], 0);
        assert_eq!(json["items"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_memories_with_type_filter() {
        let mut mem = sample_memory("mem_1_aaa", "a question");
        mem.memory_type = MemoryType::Question;
        let memories = vec![
            mem,
            sample_memory("mem_2_bbb", "some info"),
        ];
        let app = test_app(Arc::new(MockVectorStore::with_memories(memories)));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/memories?type=question")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["items"][0]["id"], "mem_1_aaa");
    }

    #[tokio::test]
    async fn test_list_memories_empty() {
        let app = test_app(Arc::new(MockVectorStore::new()));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/memories")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 0);
        assert_eq!(json["items"].as_array().unwrap().len(), 0);
    }

    // ── Delete ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_memory_returns_deleted_true() {
        let mem = sample_memory("mem_1_aaa", "to delete");
        let app = test_app(Arc::new(MockVectorStore::with_memories(vec![mem])));

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::DELETE)
                    .uri("/memories/mem_1_aaa")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["deleted"], true);
    }

    #[tokio::test]
    async fn test_delete_memory_not_found_returns_deleted_false() {
        let app = test_app(Arc::new(MockVectorStore::new()));

        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::DELETE)
                    .uri("/memories/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["deleted"], false);
    }

    // ── Search ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_search_memories_returns_results() {
        let memories = vec![
            sample_memory("mem_1_aaa", "Paris is the capital of France"),
            sample_memory("mem_2_bbb", "Berlin is the capital of Germany"),
        ];
        let app = test_app(Arc::new(MockVectorStore::with_memories(memories)));

        let body = serde_json::json!({"query": "Paris", "limit": 5});
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/memories/search")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["memories"][0]["id"], "mem_1_aaa");
    }

    #[tokio::test]
    async fn test_search_empty_query_returns_400() {
        let app = test_app(Arc::new(MockVectorStore::new()));

        let body = serde_json::json!({"query": ""});
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/memories/search")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // ── Update visibility ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_visibility_returns_updated_memory() {
        let mem = sample_memory("mem_1_aaa", "some content");
        let app = test_app(Arc::new(MockVectorStore::with_memories(vec![mem])));

        let body = serde_json::json!({
            "visibility": "shared",
            "shared_with": ["agent-2"]
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::PUT)
                    .uri("/memories/mem_1_aaa/visibility")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["visibility"], "shared");
        assert_eq!(json["shared_with"], serde_json::json!(["agent-2"]));
    }

    #[tokio::test]
    async fn test_update_visibility_not_found_returns_404() {
        let app = test_app(Arc::new(MockVectorStore::new()));

        let body = serde_json::json!({"visibility": "private"});
        let resp = app
            .oneshot(
                Request::builder()
                    .method(http::Method::PUT)
                    .uri("/memories/nonexistent/visibility")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
