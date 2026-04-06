//! REST API handlers for the agentd-index service.
//!
//! # Endpoints
//!
//! | Method | Path      | Description                             |
//! |--------|-----------|-----------------------------------------|
//! | `GET`  | `/health` | Service health check                    |
//! | `POST` | `/search` | Semantic vector search over code chunks |
//!
//! # State
//!
//! Full search functionality requires an [`AppState`] containing a live
//! [`CodeStore`].  Use [`create_router_with_state`] when a store is available,
//! or [`create_router`] for the health-only router (useful in tests).

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

use agentd_common::types::HealthResponse;

use crate::search::vector::VectorSearch;
use crate::search::{SearchError, SearchRequest, SearchStrategy};
use crate::store::CodeStore;

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Shared state injected into route handlers.
#[derive(Clone)]
pub struct AppState {
    /// The backing vector store used for search.
    pub store: Arc<dyn CodeStore>,
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

/// Creates the full Axum router, including the `POST /search` endpoint.
///
/// The provided `state` is injected into all stateful handlers via
/// Axum's [`State`] extractor.
pub fn create_router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/search", post(search_handler))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /health` — returns service health status.
async fn health_handler() -> impl IntoResponse {
    Json(HealthResponse::ok("agentd-index", env!("CARGO_PKG_VERSION")))
}

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
    // For now, always use VectorSearch regardless of `search_mode`.
    // Keyword / Hybrid strategies are wired in #950.
    let strategy = VectorSearch::new(Arc::clone(&state.store));

    match strategy.search(&request).await {
        Ok(response) => (StatusCode::OK, Json(json!(response))).into_response(),
        Err(SearchError::InvalidRequest(msg)) => {
            (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": msg }))).into_response()
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
            Ok(self.results.iter().cloned().take(limit).collect())
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

    fn search_app(results: Vec<SearchResult>) -> Router {
        let state = AppState { store: Arc::new(MockStore { results }) };
        create_router_with_state(state)
    }

    #[tokio::test]
    async fn search_returns_200_with_results() {
        let app = search_app(vec![make_result("my_fn", 0.9)]);
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
        let app = search_app(vec![]);
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
        let app = search_app(vec![]);
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
    async fn search_bad_json_returns_422() {
        let app = search_app(vec![]);
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
}
