//! Vector similarity search strategy.
//!
//! [`VectorSearch`] embeds the query text via the configured embedding model
//! and performs ANN (approximate nearest-neighbour) search over the LanceDB
//! vector store.  Results are post-filtered by optional language, file path
//! pattern, and hierarchy level criteria before being returned.
//!
//! # Algorithm
//!
//! 1. Over-fetch `3 × limit` candidates from the store (reduces the impact of
//!    post-filtering eliminating results that would otherwise fill the quota).
//! 2. Apply optional filters: `language`, `file_pattern`, `hierarchy_level`.
//! 3. Return the first `limit` surviving results, preserving score order.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;

use crate::store::CodeStore;

use super::{
    matches_file_pattern, SearchError, SearchRequest, SearchResponse, SearchResultItem,
    SearchStrategy,
};

// ---------------------------------------------------------------------------
// VectorSearch
// ---------------------------------------------------------------------------

/// Semantic vector similarity search backed by the LanceDB [`CodeStore`].
///
/// Construct via [`VectorSearch::new`] and call via the [`SearchStrategy`]
/// trait.  The store is responsible for embedding the query; this struct
/// only drives the over-fetch / post-filter pipeline.
pub struct VectorSearch {
    store: Arc<dyn CodeStore>,
}

impl VectorSearch {
    /// Create a new `VectorSearch` backed by `store`.
    pub fn new(store: Arc<dyn CodeStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl SearchStrategy for VectorSearch {
    /// Execute a vector similarity search.
    ///
    /// Returns [`SearchError::InvalidRequest`] when the query is blank.
    async fn search(&self, request: &SearchRequest) -> Result<SearchResponse, SearchError> {
        if request.query.trim().is_empty() {
            return Err(SearchError::InvalidRequest("query must not be empty".to_string()));
        }

        let start = Instant::now();
        let limit = request.limit.unwrap_or(10).clamp(1, 100);
        // Over-fetch so post-filtering still yields `limit` results in the
        // common case where a fraction of candidates are filtered out.
        let over_fetch = limit * 3;

        let raw = self
            .store
            .search(&request.query, request.repo_id.as_deref(), over_fetch)
            .await
            .map_err(|e| SearchError::Backend(e.to_string()))?;

        // Post-filter, then take up to `limit` results.
        let results: Vec<SearchResultItem> = raw
            .into_iter()
            .filter(|r| {
                // Language filter — case-insensitive comparison.
                if let Some(lang) = &request.language {
                    if r.chunk.chunk.language.as_str() != lang.to_lowercase().as_str() {
                        return false;
                    }
                }
                // File path glob filter.
                if let Some(pattern) = &request.file_pattern {
                    if !matches_file_pattern(&r.chunk.chunk.file_path, pattern) {
                        return false;
                    }
                }
                // Hierarchy level filter — case-insensitive.
                if let Some(level) = &request.hierarchy_level {
                    if r.chunk.chunk.hierarchy_level.as_str() != level.to_lowercase().as_str() {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .map(|r| SearchResultItem {
                id: r.chunk.id,
                file_path: r.chunk.chunk.file_path,
                language: r.chunk.chunk.language.to_string(),
                chunk_type: r.chunk.chunk.chunk_type.to_string(),
                symbol_name: r.chunk.chunk.symbol_name,
                start_line: r.chunk.chunk.start_line,
                end_line: r.chunk.chunk.end_line,
                content: r.chunk.chunk.content,
                summary: r.chunk.summary,
                score: r.score,
                repo_id: r.chunk.repo_id,
            })
            .collect();

        let total = results.len();
        let query_time_ms = start.elapsed().as_millis() as u64;

        Ok(SearchResponse { results, total, query_time_ms })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::types::{ChunkType, CodeChunk, HierarchyLevel, Language};
    use crate::metadata::ChunkMetadata;
    use crate::store::error::StoreResult;
    use crate::store::{SearchResult, StoredChunk};
    use async_trait::async_trait;

    // ── MockStore ──────────────────────────────────────────────────────────

    struct MockStore {
        results: Vec<SearchResult>,
    }

    #[async_trait]
    impl CodeStore for MockStore {
        async fn initialize(&self) -> StoreResult<()> {
            Ok(())
        }
        async fn health_check(&self) -> StoreResult<bool> {
            Ok(true)
        }
        async fn store_chunks(
            &self,
            _repo_id: &str,
            _file_hash: &str,
            _chunks: Vec<CodeChunk>,
        ) -> StoreResult<Vec<String>> {
            Ok(vec![])
        }
        async fn delete_file_chunks(&self, _repo_id: &str, _file_path: &str) -> StoreResult<usize> {
            Ok(0)
        }
        async fn get_file_hash(
            &self,
            _repo_id: &str,
            _file_path: &str,
        ) -> StoreResult<Option<String>> {
            Ok(None)
        }
        async fn list_file_hashes(&self, _repo_id: &str) -> StoreResult<Vec<(String, String)>> {
            Ok(vec![])
        }
        async fn search(
            &self,
            _query: &str,
            _repo_id: Option<&str>,
            limit: usize,
        ) -> StoreResult<Vec<SearchResult>> {
            Ok(self.results.iter().take(limit).cloned().collect())
        }
        async fn update_summary(&self, _chunk_id: &str, _summary: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn list_unsummarized_chunks(
            &self,
            _repo_id: &str,
            _limit: usize,
        ) -> StoreResult<Vec<StoredChunk>> {
            Ok(vec![])
        }
    }

    // ── Helpers ────────────────────────────────────────────────────────────

    fn make_result(
        id: &str,
        file_path: &str,
        language: Language,
        level: HierarchyLevel,
        score: f32,
    ) -> SearchResult {
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
                    file_path: file_path.to_string(),
                    language,
                    chunk_type: ChunkType::Function,
                    start_line: 1,
                    end_line: 3,
                    symbol_name: Some(id.to_string()),
                    parent_symbol: None,
                    hierarchy_level: level,
                    metadata: ChunkMetadata::default(),
                },
            },
        }
    }

    fn req(query: &str) -> SearchRequest {
        SearchRequest {
            query: query.to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: None,
            search_mode: Default::default(),
        }
    }

    // ── Tests ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn empty_query_returns_error() {
        let search = VectorSearch::new(Arc::new(MockStore { results: vec![] }));
        let result = search.search(&req("   ")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn returns_top_k_results() {
        let results = vec![
            make_result("fn_a", "src/a.rs", Language::Rust, HierarchyLevel::Symbol, 0.9),
            make_result("fn_b", "src/b.rs", Language::Rust, HierarchyLevel::Symbol, 0.8),
            make_result("fn_c", "src/c.rs", Language::Rust, HierarchyLevel::Symbol, 0.7),
        ];
        let search = VectorSearch::new(Arc::new(MockStore { results }));
        let response =
            search.search(&SearchRequest { limit: Some(2), ..req("test") }).await.unwrap();
        assert_eq!(response.total, 2);
        assert_eq!(response.results[0].id, "fn_a");
        assert_eq!(response.results[1].id, "fn_b");
    }

    #[tokio::test]
    async fn default_limit_is_10() {
        let results: Vec<SearchResult> = (0..15)
            .map(|i| {
                make_result(
                    &format!("fn_{i}"),
                    &format!("src/f{i}.rs"),
                    Language::Rust,
                    HierarchyLevel::Symbol,
                    1.0 - i as f32 * 0.01,
                )
            })
            .collect();
        let search = VectorSearch::new(Arc::new(MockStore { results }));
        let response = search.search(&req("test")).await.unwrap();
        assert_eq!(response.total, 10);
    }

    #[tokio::test]
    async fn limit_clamped_to_100() {
        let results: Vec<SearchResult> = (0..200)
            .map(|i| {
                make_result(
                    &format!("fn_{i}"),
                    &format!("src/f{i}.rs"),
                    Language::Rust,
                    HierarchyLevel::Symbol,
                    1.0,
                )
            })
            .collect();
        let search = VectorSearch::new(Arc::new(MockStore { results }));
        let response =
            search.search(&SearchRequest { limit: Some(999), ..req("test") }).await.unwrap();
        assert_eq!(response.total, 100);
    }

    #[tokio::test]
    async fn language_filter_excludes_non_matching() {
        let results = vec![
            make_result("fn_a", "src/a.rs", Language::Rust, HierarchyLevel::Symbol, 0.9),
            make_result("fn_b", "src/b.py", Language::Python, HierarchyLevel::Symbol, 0.8),
        ];
        let search = VectorSearch::new(Arc::new(MockStore { results }));
        let response = search
            .search(&SearchRequest {
                language: Some("rust".to_string()),
                limit: Some(10),
                ..req("test")
            })
            .await
            .unwrap();
        assert_eq!(response.total, 1);
        assert_eq!(response.results[0].language, "rust");
    }

    #[tokio::test]
    async fn file_pattern_filter() {
        let results = vec![
            make_result(
                "fn_a",
                "src/auth/middleware.rs",
                Language::Rust,
                HierarchyLevel::Symbol,
                0.9,
            ),
            make_result("fn_b", "src/other/file.rs", Language::Rust, HierarchyLevel::Symbol, 0.8),
        ];
        let search = VectorSearch::new(Arc::new(MockStore { results }));
        let response = search
            .search(&SearchRequest {
                file_pattern: Some("src/auth/**".to_string()),
                limit: Some(10),
                ..req("test")
            })
            .await
            .unwrap();
        assert_eq!(response.total, 1);
        assert!(response.results[0].file_path.contains("auth"));
    }

    #[tokio::test]
    async fn hierarchy_level_filter() {
        let results = vec![
            make_result("fn_a", "src/a.rs", Language::Rust, HierarchyLevel::Symbol, 0.9),
            make_result("file_a", "src/a.rs", Language::Rust, HierarchyLevel::File, 0.8),
        ];
        let search = VectorSearch::new(Arc::new(MockStore { results }));
        let response = search
            .search(&SearchRequest {
                hierarchy_level: Some("file".to_string()),
                limit: Some(10),
                ..req("test")
            })
            .await
            .unwrap();
        assert_eq!(response.total, 1);
        assert_eq!(response.results[0].id, "file_a");
    }

    #[tokio::test]
    async fn response_fields_are_mapped_correctly() {
        let results =
            vec![make_result("my_fn", "src/lib.rs", Language::Rust, HierarchyLevel::Symbol, 0.95)];
        let search = VectorSearch::new(Arc::new(MockStore { results }));
        let response = search.search(&req("test")).await.unwrap();
        let item = &response.results[0];
        assert_eq!(item.id, "my_fn");
        assert_eq!(item.file_path, "src/lib.rs");
        assert_eq!(item.language, "rust");
        assert_eq!(item.chunk_type, "function");
        assert_eq!(item.symbol_name, Some("my_fn".to_string()));
        assert_eq!(item.start_line, 1);
        assert_eq!(item.end_line, 3);
        assert!((item.score - 0.95).abs() < 1e-5);
        assert_eq!(item.repo_id, "repo1");
    }

    #[tokio::test]
    async fn response_includes_query_timing() {
        let search = VectorSearch::new(Arc::new(MockStore { results: vec![] }));
        let response = search.search(&req("test")).await.unwrap();
        assert!(response.query_time_ms < 10_000);
    }

    #[tokio::test]
    async fn empty_store_returns_empty_results() {
        let search = VectorSearch::new(Arc::new(MockStore { results: vec![] }));
        let response = search.search(&req("test")).await.unwrap();
        assert_eq!(response.total, 0);
        assert!(response.results.is_empty());
    }
}
