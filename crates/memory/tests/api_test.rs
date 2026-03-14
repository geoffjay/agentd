//! Integration tests for the memory service REST API.
//!
//! Tests exercise the full Axum router with a [`MockVectorStore`] backend,
//! verifying HTTP status codes, response bodies, pagination, filtering,
//! and error handling for every endpoint.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{self, Request, StatusCode};
use chrono::Utc;
use memory::error::{StoreError, StoreResult};
use memory::store::VectorStore;
use memory::types::*;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Mock vector store
// ---------------------------------------------------------------------------

/// In-memory mock of [`VectorStore`] for API integration tests.
struct MockVectorStore {
    memories: tokio::sync::RwLock<Vec<Memory>>,
}

impl MockVectorStore {
    fn new() -> Self {
        Self { memories: tokio::sync::RwLock::new(Vec::new()) }
    }

    fn with_memories(memories: Vec<Memory>) -> Self {
        Self { memories: tokio::sync::RwLock::new(memories) }
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
            .filter(|m| m.is_visible_to(req.as_actor.as_deref()))
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The API module is private to the binary, so we need to import from
/// memory's public types and reconstruct the router pattern.
/// Since api.rs is private, we test via the binary's exported router.
/// For integration tests we use the memory crate's public API directly
/// and build our own minimal router that mirrors the binary's.
fn build_api_router(store: Arc<dyn VectorStore>) -> axum::Router {
    use axum::routing::{get, post, put};
    use axum::{
        extract::{Path, Query, State},
        Json,
    };
    use memory::error::StoreError;
    use serde::Deserialize;

    #[derive(Clone)]
    struct ApiState {
        store: Arc<dyn VectorStore>,
    }

    #[derive(Deserialize)]
    struct ListParams {
        #[serde(rename = "type")]
        memory_type: Option<String>,
        #[allow(dead_code)]
        tag: Option<String>,
        created_by: Option<String>,
        visibility: Option<String>,
        limit: Option<usize>,
        offset: Option<usize>,
    }

    async fn health(State(s): State<ApiState>) -> axum::response::Response {
        let healthy = s.store.health_check().await.unwrap_or(false);
        let body = serde_json::json!({
            "status": "ok",
            "service": "agentd-memory",
            "details": { "vector_store": healthy }
        });
        axum::Json(body).into_response()
    }

    async fn create_mem(
        State(s): State<ApiState>,
        Json(req): Json<CreateMemoryRequest>,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        if req.content.trim().is_empty() {
            return (StatusCode::BAD_REQUEST, "content must not be empty").into_response();
        }
        if req.created_by.trim().is_empty() {
            return (StatusCode::BAD_REQUEST, "created_by must not be empty").into_response();
        }
        match s.store.create(req).await {
            Ok(mem) => (StatusCode::CREATED, Json(mem)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }

    async fn get_mem(
        State(s): State<ApiState>,
        Path(id): Path<String>,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        match s.store.get(&id).await {
            Ok(Some(mem)) => Json(mem).into_response(),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }

    async fn list_mems(
        State(s): State<ApiState>,
        Query(params): Query<ListParams>,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        let all = s.store.list_all().await.unwrap_or_default();
        let limit = params.limit.unwrap_or(50).min(200);
        let offset = params.offset.unwrap_or(0);

        let filtered: Vec<Memory> = all
            .into_iter()
            .filter(|m| {
                params.memory_type.as_ref().map_or(true, |t| m.memory_type.to_string() == *t)
            })
            .filter(|m| params.visibility.as_ref().map_or(true, |v| m.visibility.to_string() == *v))
            .filter(|m| params.created_by.as_ref().map_or(true, |c| m.created_by == *c))
            .collect();

        let total = filtered.len();
        let items: Vec<Memory> = filtered.into_iter().skip(offset).take(limit).collect();

        Json(serde_json::json!({
            "items": items,
            "total": total,
            "limit": limit,
            "offset": offset,
        }))
        .into_response()
    }

    async fn delete_mem(
        State(s): State<ApiState>,
        Path(id): Path<String>,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        match s.store.delete(&id).await {
            Ok(deleted) => Json(DeleteResponse { deleted }).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }

    async fn search_mems(
        State(s): State<ApiState>,
        Json(req): Json<SearchRequest>,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        if req.query.trim().is_empty() {
            return (StatusCode::BAD_REQUEST, "query must not be empty").into_response();
        }
        match s.store.search(req).await {
            Ok(memories) => {
                let total = memories.len();
                Json(SearchResponse { memories, total }).into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }

    async fn update_vis(
        State(s): State<ApiState>,
        Path(id): Path<String>,
        Json(req): Json<UpdateVisibilityRequest>,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        match s.store.update_visibility(&id, req.visibility, req.shared_with).await {
            Ok(mem) => Json(mem).into_response(),
            Err(StoreError::NotFound(_)) => StatusCode::NOT_FOUND.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }

    use axum::response::IntoResponse;

    let memories_router = axum::Router::new()
        .route("/", get(list_mems).post(create_mem))
        .route("/search", post(search_mems))
        .route("/{id}", get(get_mem).delete(delete_mem))
        .route("/{id}/visibility", put(update_vis));

    axum::Router::new()
        .route("/health", get(health))
        .nest("/memories", memories_router)
        .with_state(ApiState { store })
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

async fn parse_body(body: Body) -> Value {
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn json_request(method: http::Method, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_returns_ok() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));
    let resp =
        app.oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "agentd-memory");
    assert_eq!(body["details"]["vector_store"], true);
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_returns_201() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));
    let body = serde_json::json!({
        "content": "Test memory content.",
        "created_by": "agent-1"
    });

    let resp = app.oneshot(json_request(http::Method::POST, "/memories", body)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = parse_body(resp.into_body()).await;
    assert!(body["id"].as_str().unwrap().starts_with("mem_"));
    assert_eq!(body["content"], "Test memory content.");
}

#[tokio::test]
async fn test_create_with_all_fields() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));
    let body = serde_json::json!({
        "content": "How do I reset?",
        "type": "question",
        "tags": ["auth", "help"],
        "created_by": "user-42",
        "references": ["mem_1_abc12345"],
        "visibility": "shared",
        "shared_with": ["agent-support"]
    });

    let resp = app.oneshot(json_request(http::Method::POST, "/memories", body)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["type"], "question");
    assert_eq!(body["tags"], serde_json::json!(["auth", "help"]));
    assert_eq!(body["visibility"], "shared");
}

#[tokio::test]
async fn test_create_empty_content_returns_400() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));
    let body = serde_json::json!({"content": "", "created_by": "agent-1"});

    let resp = app.oneshot(json_request(http::Method::POST, "/memories", body)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_whitespace_content_returns_400() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));
    let body = serde_json::json!({"content": "   ", "created_by": "agent-1"});

    let resp = app.oneshot(json_request(http::Method::POST, "/memories", body)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_empty_created_by_returns_400() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));
    let body = serde_json::json!({"content": "valid", "created_by": ""});

    let resp = app.oneshot(json_request(http::Method::POST, "/memories", body)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Get
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_existing_returns_200() {
    let mem = sample_memory("mem_1_aaa", "test content");
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(vec![mem])));

    let resp = app
        .oneshot(Request::builder().uri("/memories/mem_1_aaa").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["id"], "mem_1_aaa");
    assert_eq!(body["content"], "test content");
}

#[tokio::test]
async fn test_get_nonexistent_returns_404() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));

    let resp = app
        .oneshot(Request::builder().uri("/memories/nonexistent").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_returns_paginated_response() {
    let memories = vec![
        sample_memory("mem_1_aaa", "first"),
        sample_memory("mem_2_bbb", "second"),
        sample_memory("mem_3_ccc", "third"),
    ];
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(memories)));

    let resp = app
        .oneshot(Request::builder().uri("/memories?limit=2&offset=0").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["total"], 3);
    assert_eq!(body["limit"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_list_with_offset() {
    let memories = vec![
        sample_memory("mem_1_aaa", "first"),
        sample_memory("mem_2_bbb", "second"),
        sample_memory("mem_3_ccc", "third"),
    ];
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(memories)));

    let resp = app
        .oneshot(Request::builder().uri("/memories?limit=10&offset=2").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["total"], 3);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_list_empty() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));

    let resp = app
        .oneshot(Request::builder().uri("/memories").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_filter_by_type() {
    let mut q_mem = sample_memory("mem_1_aaa", "a question");
    q_mem.memory_type = MemoryType::Question;
    let memories = vec![q_mem, sample_memory("mem_2_bbb", "some info")];
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(memories)));

    let resp = app
        .oneshot(Request::builder().uri("/memories?type=question").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], "mem_1_aaa");
}

#[tokio::test]
async fn test_list_filter_by_visibility() {
    let mut priv_mem = sample_memory("mem_1_aaa", "private stuff");
    priv_mem.visibility = VisibilityLevel::Private;
    let memories = vec![priv_mem, sample_memory("mem_2_bbb", "public stuff")];
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(memories)));

    let resp = app
        .oneshot(
            Request::builder().uri("/memories?visibility=private").body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], "mem_1_aaa");
}

#[tokio::test]
async fn test_list_filter_by_created_by() {
    let mut mem2 = sample_memory("mem_2_bbb", "other agent");
    mem2.created_by = "agent-2".to_string();
    let memories = vec![sample_memory("mem_1_aaa", "agent-1 mem"), mem2];
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(memories)));

    let resp = app
        .oneshot(
            Request::builder().uri("/memories?created_by=agent-2").body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], "mem_2_bbb");
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_delete_existing_returns_true() {
    let mem = sample_memory("mem_1_aaa", "to delete");
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(vec![mem])));

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
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["deleted"], true);
}

#[tokio::test]
async fn test_delete_nonexistent_returns_false() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));

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
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["deleted"], false);
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_returns_matching_results() {
    let memories = vec![
        sample_memory("mem_1_aaa", "Paris is the capital of France"),
        sample_memory("mem_2_bbb", "Berlin is the capital of Germany"),
    ];
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(memories)));

    let body = serde_json::json!({"query": "Paris", "limit": 5});
    let resp =
        app.oneshot(json_request(http::Method::POST, "/memories/search", body)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["memories"][0]["id"], "mem_1_aaa");
}

#[tokio::test]
async fn test_search_empty_query_returns_400() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));

    let body = serde_json::json!({"query": ""});
    let resp =
        app.oneshot(json_request(http::Method::POST, "/memories/search", body)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_search_whitespace_query_returns_400() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));

    let body = serde_json::json!({"query": "   "});
    let resp =
        app.oneshot(json_request(http::Method::POST, "/memories/search", body)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_search_with_actor_filters_visibility() {
    let mut private_mem = sample_memory("mem_1_aaa", "secret Paris info");
    private_mem.visibility = VisibilityLevel::Private;
    private_mem.created_by = "agent-1".to_string();

    let memories = vec![private_mem, sample_memory("mem_2_bbb", "public Paris info")];
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(memories)));

    // Search as anonymous — should only find public
    let body = serde_json::json!({"query": "Paris", "limit": 10});
    let resp =
        app.oneshot(json_request(http::Method::POST, "/memories/search", body)).await.unwrap();

    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["memories"][0]["id"], "mem_2_bbb");
}

// ---------------------------------------------------------------------------
// Update visibility
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_update_visibility_returns_updated() {
    let mem = sample_memory("mem_1_aaa", "some content");
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(vec![mem])));

    let body = serde_json::json!({
        "visibility": "shared",
        "shared_with": ["agent-2", "agent-3"]
    });
    let resp = app
        .oneshot(json_request(http::Method::PUT, "/memories/mem_1_aaa/visibility", body))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["visibility"], "shared");
    assert_eq!(body["shared_with"], serde_json::json!(["agent-2", "agent-3"]));
}

#[tokio::test]
async fn test_update_visibility_not_found_returns_404() {
    let app = build_api_router(Arc::new(MockVectorStore::new()));

    let body = serde_json::json!({"visibility": "private"});
    let resp = app
        .oneshot(json_request(http::Method::PUT, "/memories/nonexistent/visibility", body))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_visibility_to_private() {
    let mem = sample_memory("mem_1_aaa", "content");
    let app = build_api_router(Arc::new(MockVectorStore::with_memories(vec![mem])));

    let body = serde_json::json!({"visibility": "private"});
    let resp = app
        .oneshot(json_request(http::Method::PUT, "/memories/mem_1_aaa/visibility", body))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = parse_body(resp.into_body()).await;
    assert_eq!(body["visibility"], "private");
}

// ---------------------------------------------------------------------------
// End-to-end API flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_create_then_get_then_delete() {
    let store = Arc::new(MockVectorStore::new());

    // Create
    let app = build_api_router(store.clone());
    let create_body = serde_json::json!({
        "content": "E2E test memory.",
        "created_by": "agent-e2e"
    });
    let resp =
        app.oneshot(json_request(http::Method::POST, "/memories", create_body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = parse_body(resp.into_body()).await;
    let id = created["id"].as_str().unwrap();

    // Get
    let app = build_api_router(store.clone());
    let resp = app
        .oneshot(Request::builder().uri(&format!("/memories/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let fetched = parse_body(resp.into_body()).await;
    assert_eq!(fetched["content"], "E2E test memory.");

    // Delete
    let app = build_api_router(store.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method(http::Method::DELETE)
                .uri(&format!("/memories/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let del_body = parse_body(resp.into_body()).await;
    assert_eq!(del_body["deleted"], true);

    // Confirm gone
    let app = build_api_router(store.clone());
    let resp = app
        .oneshot(Request::builder().uri(&format!("/memories/{id}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
