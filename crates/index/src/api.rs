//! REST API handlers for the agentd-index service.
//!
//! # Endpoints
//!
//! | Method   | Path                          | Description                              |
//! |----------|-------------------------------|------------------------------------------|
//! | `GET`    | `/health`                     | Service health check                     |
//! | `POST`   | `/search`                     | Semantic / hybrid vector search          |
//! | `POST`   | `/search/agentic`             | Grep-based fallback search               |
//! | `POST`   | `/repositories`               | Register a repository                    |
//! | `GET`    | `/repositories`               | List all repositories                    |
//! | `GET`    | `/repositories/:id`           | Get repository by ID                     |
//! | `DELETE` | `/repositories/:id`           | Remove a repository                      |
//! | `GET`    | `/repositories/:id/status`    | Get repository indexing status           |
//! | `POST`   | `/repositories/:id/reindex`   | Trigger re-indexing                      |
//!
//! # State
//!
//! Full functionality requires an [`AppState`] containing a live [`CodeStore`]
//! and a [`RepoStore`].  Use [`create_router_with_state`] for production, or
//! [`create_router`] for the health-only router (useful in tests).

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use agentd_common::types::HealthResponse;

use crate::repository::{AddRepoRequest, RepoStatus, RepoStore};
use crate::search::agentic::{AgenticSearch, AgenticSearchRequest};
use crate::search::hybrid::HybridSearch;
use crate::search::vector::VectorSearch;
use crate::search::{SearchError, SearchMode, SearchRequest, SearchStrategy};
use crate::store::CodeStore;

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Shared state injected into route handlers.
#[derive(Clone)]
pub struct AppState {
    /// The backing vector store used for search.
    pub store: Arc<dyn CodeStore>,

    /// File-backed repository registry.
    pub repo_store: Arc<RepoStore>,
}

// ---------------------------------------------------------------------------
// Embeddings sample types
// ---------------------------------------------------------------------------

/// Query parameters for `GET /repositories/{id}/embeddings/sample`.
#[derive(Debug, Deserialize)]
struct EmbeddingsSampleParams {
    /// Maximum number of chunks to return (default 2000, capped at 5000).
    limit: Option<usize>,
}

/// A single point in the 2D embedding projection.
#[derive(Debug, Serialize)]
struct EmbeddingPoint {
    /// X coordinate in the projected 2D space (range approximately −1.0 to 1.0).
    x: f32,
    /// Y coordinate in the projected 2D space (range approximately −1.0 to 1.0).
    y: f32,
    /// Source file path of the chunk.
    file_path: String,
    /// Programming language (e.g. "rust").
    language: String,
    /// Syntactic kind (e.g. "function").
    chunk_type: String,
    /// Top-level symbol name if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol_name: Option<String>,
}

/// Deterministic hash-based 2D projection for a chunk ID.
///
/// Maps a string ID to (x, y) coordinates in the range [−1, 1] using two
/// independent FNV-1a hash passes with alternating byte scheduling. This
/// provides a stable, uniform scatter without requiring actual embedding
/// vectors.
fn pseudo_project(id: &str) -> (f32, f32) {
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    let mut h1: u64 = FNV_OFFSET;
    let mut h2: u64 = FNV_OFFSET ^ 0xDEAD_BEEF_CAFE_BABE;
    for (i, b) in id.bytes().enumerate() {
        if i % 2 == 0 {
            h1 ^= b as u64;
            h1 = h1.wrapping_mul(FNV_PRIME);
        } else {
            h2 ^= b as u64;
            h2 = h2.wrapping_mul(FNV_PRIME);
        }
    }
    let x = ((h1 & 0xFFFF) as f32 / 32_767.5) - 1.0;
    let y = ((h2 & 0xFFFF) as f32 / 32_767.5) - 1.0;
    (x, y)
}

// ---------------------------------------------------------------------------
// Router constructors
// ---------------------------------------------------------------------------

/// Creates the Axum router with health-only routes (no store dependency).
///
/// Useful for lightweight tests that only exercise `GET /health`.
pub fn create_router() -> Router {
    Router::new().route("/health", get(health_handler))
}

/// Creates the full Axum router including search and repository management.
///
/// The provided `state` is injected into all stateful handlers via
/// Axum's [`State`] extractor.
pub fn create_router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/search", post(search_handler))
        .route("/search/agentic", post(agentic_search_handler))
        .route("/repositories", post(create_repo_handler))
        .route("/repositories", get(list_repos_handler))
        .route("/repositories/{id}", get(get_repo_handler))
        .route("/repositories/{id}", delete(delete_repo_handler))
        .route("/repositories/{id}/status", get(get_repo_status_handler))
        .route("/repositories/{id}/reindex", post(reindex_repo_handler))
        .route("/repositories/{id}/embeddings/sample", get(embeddings_sample_handler))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Health handler
// ---------------------------------------------------------------------------

/// `GET /health` — returns service health status.
async fn health_handler() -> impl IntoResponse {
    Json(HealthResponse::ok("agentd-index", env!("CARGO_PKG_VERSION")))
}

// ---------------------------------------------------------------------------
// Search handlers
// ---------------------------------------------------------------------------

/// `POST /search` — semantic vector search over indexed code chunks.
///
/// Accepts a [`SearchRequest`] JSON body and returns a ranked list of
/// matching [`crate::search::SearchResultItem`] values.
///
/// # Errors
///
/// Returns `422 Unprocessable Entity` for invalid requests (e.g. empty query)
/// and `500 Internal Server Error` for backend failures.
async fn search_handler(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> impl IntoResponse {
    // Dispatch to the appropriate search strategy based on `search_mode`.
    // Keyword-only mode falls back to hybrid with alpha=0 (pure BM25 ranking).
    let result = match request.search_mode {
        SearchMode::Hybrid | SearchMode::Keyword => {
            let alpha = if request.search_mode == SearchMode::Keyword { 0.0 } else { 0.7 };
            HybridSearch::with_alpha(Arc::clone(&state.store), alpha).search(&request).await
        }
        SearchMode::Vector => VectorSearch::new(Arc::clone(&state.store)).search(&request).await,
    };
    match result {
        Ok(response) => (StatusCode::OK, Json(json!(response))).into_response(),
        Err(SearchError::InvalidRequest(msg)) => {
            (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": msg }))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
            .into_response(),
    }
}

/// `POST /search/agentic` — grep-based fallback search over source files.
///
/// Accepts an [`AgenticSearchRequest`] and returns matching lines with context.
/// Does not require the vector index — searches files directly via `grep`.
///
/// # Use Cases
///
/// - Specific identifiers not yet indexed
/// - Low-confidence vector search results
/// - Verifying index results against raw source
async fn agentic_search_handler(
    State(state): State<AppState>,
    Json(request): Json<AgenticSearchRequest>,
) -> impl IntoResponse {
    // Root the agentic search in the process working directory.
    let _ = &state; // store not needed for grep-based search
    let search = AgenticSearch::new(std::env::current_dir().unwrap_or_else(|_| ".".into()));

    match search.search(&request).await {
        Ok(response) => (StatusCode::OK, Json(json!(response))).into_response(),
        Err(SearchError::InvalidRequest(msg)) => {
            (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": msg }))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Repository handlers
// ---------------------------------------------------------------------------

/// `POST /repositories` — register a new repository.
///
/// # Request body
///
/// ```json
/// { "name": "agentd", "path": "/home/user/agentd" }
/// ```
///
/// # Responses
///
/// - `201 Created` — repository registered; returns the [`RepoRecord`][crate::repository::RepoRecord].
/// - `500 Internal Server Error` — storage write failed.
async fn create_repo_handler(
    State(state): State<AppState>,
    Json(request): Json<AddRepoRequest>,
) -> impl IntoResponse {
    match state.repo_store.add(&request.name, &request.path).await {
        Ok(record) => (StatusCode::CREATED, Json(json!(record))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
            .into_response(),
    }
}

/// `GET /repositories` — list all registered repositories.
async fn list_repos_handler(State(state): State<AppState>) -> impl IntoResponse {
    let repos = state.repo_store.list().await;
    Json(json!({ "repositories": repos, "total": repos.len() }))
}

/// `GET /repositories/:id` — get a single repository by ID.
///
/// Returns `404 Not Found` if the ID is not registered.
async fn get_repo_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.repo_store.get(&id).await {
        Some(record) => (StatusCode::OK, Json(json!(record))).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "repository not found" })))
            .into_response(),
    }
}

/// `DELETE /repositories/:id` — remove a registered repository.
///
/// Returns `204 No Content` on success, `404 Not Found` if the ID is unknown.
async fn delete_repo_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.repo_store.remove(&id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({ "error": "repository not found" })))
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
            .into_response(),
    }
}

/// `GET /repositories/:id/status` — return just the indexing status.
///
/// Returns `404 Not Found` if the ID is unknown.
async fn get_repo_status_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.repo_store.get(&id).await {
        Some(record) => (
            StatusCode::OK,
            Json(json!({
                "id": record.id,
                "status": record.status,
                "last_indexed": record.last_indexed,
                "error_message": record.error_message,
            })),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "repository not found" })))
            .into_response(),
    }
}

/// `POST /repositories/:id/reindex` — mark a repository for re-indexing.
///
/// Sets the repository status to [`RepoStatus::Pending`] so that the background
/// watcher loop will pick it up and trigger a full re-index.
///
/// Returns `202 Accepted` on success, `404 Not Found` if the ID is unknown.
async fn reindex_repo_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.repo_store.update_status(&id, RepoStatus::Pending, None).await {
        Ok(true) => (StatusCode::ACCEPTED, Json(json!({ "status": "pending" }))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({ "error": "repository not found" })))
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Embeddings sample handler
// ---------------------------------------------------------------------------

/// `GET /repositories/{id}/embeddings/sample?limit=500`
///
/// Returns a sample of indexed chunks with deterministic 2D-projected
/// coordinates suitable for scatter-plot visualisation.
///
/// The projection is a hash-based pseudo-projection that provides a stable,
/// uniform scatter of points without requiring access to raw embedding
/// vectors.  Future versions can replace this with a real PCA or UMAP
/// projection once vector read APIs are available.
///
/// # Responses
///
/// - `200 OK` — `{ points, total_chunks, sampled }`
/// - `404 Not Found` — repository not registered
/// - `500 Internal Server Error` — store query failed
async fn embeddings_sample_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<EmbeddingsSampleParams>,
) -> impl IntoResponse {
    if state.repo_store.get(&id).await.is_none() {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "repository not found" })))
            .into_response();
    }

    let limit = params.limit.unwrap_or(2000).min(5000);

    let total = match state.store.count_chunks(&id).await {
        Ok(n) => n,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
                .into_response()
        }
    };

    match state.store.sample_chunks(&id, limit).await {
        Ok(chunks) => {
            let sampled = chunks.len();
            let points: Vec<EmbeddingPoint> = chunks
                .iter()
                .map(|c| {
                    let (x, y) = pseudo_project(&c.id);
                    EmbeddingPoint {
                        x,
                        y,
                        file_path: c.chunk.file_path.clone(),
                        language: c.chunk.language.to_string(),
                        chunk_type: c.chunk.chunk_type.to_string(),
                        symbol_name: c.chunk.symbol_name.clone(),
                    }
                })
                .collect();
            (
                StatusCode::OK,
                Json(json!({
                    "points": points,
                    "total_chunks": total,
                    "sampled": sampled,
                })),
            )
                .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() })))
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use hyper::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::chunking::types::{ChunkType, CodeChunk, HierarchyLevel, Language};
    use crate::metadata::ChunkMetadata;
    use crate::store::error::StoreResult;
    use crate::store::{SearchResult, StoredChunk};

    // ── MockStore ──────────────────────────────────────────────────────────

    struct MockStore {
        results: Vec<SearchResult>,
    }

    #[async_trait::async_trait]
    impl CodeStore for MockStore {
        async fn initialize(&self) -> StoreResult<()> {
            Ok(())
        }
        async fn health_check(&self) -> StoreResult<bool> {
            Ok(true)
        }
        async fn store_chunks(
            &self,
            _: &str,
            _: &str,
            _: Vec<CodeChunk>,
        ) -> StoreResult<Vec<String>> {
            Ok(vec![])
        }
        async fn delete_file_chunks(&self, _: &str, _: &str) -> StoreResult<usize> {
            Ok(0)
        }
        async fn get_file_hash(&self, _: &str, _: &str) -> StoreResult<Option<String>> {
            Ok(None)
        }
        async fn list_file_hashes(&self, _: &str) -> StoreResult<Vec<(String, String)>> {
            Ok(vec![])
        }
        async fn search(
            &self,
            _: &str,
            _: Option<&str>,
            limit: usize,
        ) -> StoreResult<Vec<SearchResult>> {
            Ok(self.results.iter().take(limit).cloned().collect())
        }
        async fn update_summary(&self, _: &str, _: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn list_unsummarized_chunks(
            &self,
            _: &str,
            _: usize,
        ) -> StoreResult<Vec<StoredChunk>> {
            Ok(vec![])
        }
        async fn count_chunks(&self, _: &str) -> StoreResult<usize> {
            Ok(0)
        }
        async fn sample_chunks(&self, _: &str, _: usize) -> StoreResult<Vec<StoredChunk>> {
            Ok(vec![])
        }
    }

    fn make_result(id: &str, score: f32) -> SearchResult {
        SearchResult {
            score,
            chunk: StoredChunk {
                id: id.to_string(),
                repo_id: "repo1".to_string(),
                file_hash: "abc".to_string(),
                summary: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
                chunk: CodeChunk {
                    content: format!("fn {id}() {{}}"),
                    file_path: format!("src/{id}.rs"),
                    language: Language::Rust,
                    chunk_type: ChunkType::Function,
                    start_line: 1,
                    end_line: 3,
                    symbol_name: Some(id.to_string()),
                    parent_symbol: None,
                    hierarchy_level: HierarchyLevel::Symbol,
                    metadata: ChunkMetadata::default(),
                },
            },
        }
    }

    fn make_app(results: Vec<SearchResult>) -> Router {
        let dir = tempfile::tempdir().unwrap();
        let repo_store = RepoStore::new(dir.path().join("repos.json"));
        let state = AppState { store: Arc::new(MockStore { results }), repo_store };
        create_router_with_state(state)
    }

    async fn make_app_with_repo() -> (Router, String, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let repo_store = RepoStore::new(dir.path().join("repos.json"));
        let repo = repo_store.add("test-repo", "/tmp/test-repo").await.unwrap();
        let state = AppState { store: Arc::new(MockStore { results: vec![] }), repo_store };
        (create_router_with_state(state), repo.id, dir)
    }

    // ── Health (no state needed) ───────────────────────────────────────────

    #[tokio::test]
    async fn health_returns_200() {
        let app = create_router();
        let request = Request::builder().method("GET").uri("/health").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_returns_ok_status() {
        let app = create_router();
        let request = Request::builder().method("GET").uri("/health").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "agentd-index");
    }

    // ── Search endpoint ────────────────────────────────────────────────────

    #[tokio::test]
    async fn search_returns_200_with_results() {
        let app = make_app(vec![make_result("my_fn", 0.9)]);
        let body = serde_json::json!({ "query": "authentication function" });
        let request = Request::builder()
            .method("POST")
            .uri("/search")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["results"][0]["id"], "my_fn");
    }

    #[tokio::test]
    async fn search_empty_query_returns_422() {
        let app = make_app(vec![]);
        let body = serde_json::json!({ "query": "" });
        let request = Request::builder()
            .method("POST")
            .uri("/search")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn search_returns_query_time_ms() {
        let app = make_app(vec![]);
        let body = serde_json::json!({ "query": "hello" });
        let request = Request::builder()
            .method("POST")
            .uri("/search")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["query_time_ms"].is_number());
    }

    #[tokio::test]
    async fn search_bad_json_returns_400() {
        let app = make_app(vec![]);
        let request = Request::builder()
            .method("POST")
            .uri("/search")
            .header("content-type", "application/json")
            .body(Body::from("not-json"))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        // Axum returns 400 Bad Request for malformed JSON bodies.
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // ── Repository endpoints ───────────────────────────────────────────────

    #[tokio::test]
    async fn create_repo_returns_201() {
        let app = make_app(vec![]);
        let body = serde_json::json!({ "name": "my-repo", "path": "/tmp/my-repo" });
        let request = Request::builder()
            .method("POST")
            .uri("/repositories")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["name"], "my-repo");
        assert_eq!(json["status"], "pending");
        assert!(json["id"].is_string());
    }

    #[tokio::test]
    async fn list_repos_returns_200() {
        let app = make_app(vec![]);
        let request =
            Request::builder().method("GET").uri("/repositories").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["repositories"].is_array());
        assert_eq!(json["total"], 0);
    }

    #[tokio::test]
    async fn get_repo_returns_record() {
        let (app, id, _dir) = make_app_with_repo().await;
        let request = Request::builder()
            .method("GET")
            .uri(format!("/repositories/{id}"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["id"], id);
    }

    #[tokio::test]
    async fn get_repo_missing_returns_404() {
        let app = make_app(vec![]);
        let request = Request::builder()
            .method("GET")
            .uri("/repositories/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_repo_returns_204() {
        let (app, id, _dir) = make_app_with_repo().await;
        let request = Request::builder()
            .method("DELETE")
            .uri(format!("/repositories/{id}"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delete_repo_missing_returns_404() {
        let app = make_app(vec![]);
        let request = Request::builder()
            .method("DELETE")
            .uri("/repositories/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_repo_status_returns_status_field() {
        let (app, id, _dir) = make_app_with_repo().await;
        let request = Request::builder()
            .method("GET")
            .uri(format!("/repositories/{id}/status"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "pending");
        assert_eq!(json["id"], id);
    }

    #[tokio::test]
    async fn reindex_repo_returns_202() {
        let (app, id, _dir) = make_app_with_repo().await;
        let request = Request::builder()
            .method("POST")
            .uri(format!("/repositories/{id}/reindex"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn reindex_repo_missing_returns_404() {
        let app = make_app(vec![]);
        let request = Request::builder()
            .method("POST")
            .uri("/repositories/nonexistent/reindex")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
