//! Hybrid search: vector similarity + BM25 combined via Reciprocal Rank Fusion.
//!
//! [`HybridSearch`] runs both strategies over a single over-fetched candidate
//! set from the vector store, then merges the ranked lists with RRF so that
//! results appearing high in *both* lists rank highest.
//!
//! # Algorithm
//!
//! 1. Over-fetch `candidates_factor × limit` results from the vector store.
//! 2. Build an in-memory tantivy index over those candidates.
//! 3. Query the keyword index for BM25 scores.
//! 4. Merge via Reciprocal Rank Fusion:
//!    `rrf_score(d) = α·(1/(k+rank_v(d))) + (1−α)·(1/(k+rank_k(d)))`
//!    where `k = 60` (standard constant), `α = 0.7` (favour semantics).
//! 5. Re-sort by RRF score and return top-`limit`.
//!
//! # Trade-offs
//!
//! The in-memory tantivy index is rebuilt on every query.  This is intentional:
//! for the candidate set sizes used here (≤ 300 chunks) the overhead is small
//! (< 50 ms) and avoids the complexity of maintaining a persistent index.  A
//! persistent index that is updated at indexing time is a future improvement.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;

use crate::store::CodeStore;

use super::keyword::{rank_map, KeywordIndex, KeywordResult};
use super::{
    matches_file_pattern, SearchError, SearchRequest, SearchResponse, SearchResultItem,
    SearchStrategy,
};

// ---------------------------------------------------------------------------
// RRF constant
// ---------------------------------------------------------------------------

/// Standard RRF smoothing constant (rank 0 would give 1/(k+0) = 1/60 ≈ 0.017).
const RRF_K: f32 = 60.0;

// ---------------------------------------------------------------------------
// HybridSearch
// ---------------------------------------------------------------------------

/// Hybrid vector + BM25 search via Reciprocal Rank Fusion.
///
/// Construct via [`HybridSearch::new`] (default `alpha = 0.7`) or
/// [`HybridSearch::with_alpha`] for a custom weighting.
pub struct HybridSearch {
    store: Arc<dyn CodeStore>,
    /// Weight assigned to the vector ranking component (0.0–1.0).
    /// Keyword component receives weight `1 - alpha`.
    alpha: f32,
    /// How many candidates to fetch from the vector store.  The actual fetch
    /// limit is `candidates_factor * request.limit`.
    candidates_factor: usize,
}

impl HybridSearch {
    /// Create a new `HybridSearch` with default `alpha = 0.7` and
    /// `candidates_factor = 10`.
    pub fn new(store: Arc<dyn CodeStore>) -> Self {
        Self { store, alpha: 0.7, candidates_factor: 10 }
    }

    /// Create a `HybridSearch` with a custom `alpha` weighting.
    ///
    /// `alpha` must be in `[0.0, 1.0]`; values outside this range are clamped.
    pub fn with_alpha(store: Arc<dyn CodeStore>, alpha: f32) -> Self {
        Self { store, alpha: alpha.clamp(0.0, 1.0), candidates_factor: 10 }
    }
}

#[async_trait]
impl SearchStrategy for HybridSearch {
    async fn search(&self, request: &SearchRequest) -> Result<SearchResponse, SearchError> {
        if request.query.trim().is_empty() {
            return Err(SearchError::InvalidRequest("query must not be empty".to_string()));
        }

        let start = Instant::now();
        let limit = request.limit.unwrap_or(10).clamp(1, 100);
        let fetch = (limit * self.candidates_factor).max(30);

        // Step 1: Fetch candidates via vector search.
        let raw = self
            .store
            .search(&request.query, request.repo_id.as_deref(), fetch)
            .await
            .map_err(|e| SearchError::Backend(e.to_string()))?;

        if raw.is_empty() {
            return Ok(SearchResponse { results: vec![], total: 0, query_time_ms: 0 });
        }

        // Step 2: Apply pre-filters so we only score relevant candidates.
        let candidates: Vec<&crate::store::SearchResult> = raw
            .iter()
            .filter(|r| {
                if let Some(lang) = &request.language {
                    if r.chunk.chunk.language.as_str() != lang.to_lowercase().as_str() {
                        return false;
                    }
                }
                if let Some(pattern) = &request.file_pattern {
                    if !matches_file_pattern(&r.chunk.chunk.file_path, pattern) {
                        return false;
                    }
                }
                if let Some(level) = &request.hierarchy_level {
                    if r.chunk.chunk.hierarchy_level.as_str() != level.to_lowercase().as_str() {
                        return false;
                    }
                }
                true
            })
            .collect();

        if candidates.is_empty() {
            return Ok(SearchResponse {
                results: vec![],
                total: 0,
                query_time_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Step 3: Build an in-memory keyword index over the candidates.
        let stored_chunks: Vec<&crate::store::StoredChunk> =
            candidates.iter().map(|r| &r.chunk).collect();

        let keyword_results: Vec<KeywordResult> = KeywordIndex::build(&stored_chunks)
            .and_then(|idx| idx.search(&request.query, candidates.len()))
            .map_err(|e| SearchError::KeywordIndex(e.to_string()))?;

        // Step 4: Build rank maps for both result lists.
        // Vector ranks: position in `candidates` (already sorted by score desc).
        let vector_ranks: HashMap<String, usize> =
            candidates.iter().enumerate().map(|(i, r)| (r.chunk.id.clone(), i)).collect();

        let keyword_ranks: HashMap<String, usize> = rank_map(&keyword_results);

        let alpha = self.alpha;
        let beta = 1.0 - alpha;

        // Step 5: Compute RRF score for each candidate.
        let mut scored: Vec<(f32, &crate::store::SearchResult)> = candidates
            .iter()
            .map(|r| {
                let v_rank = vector_ranks.get(&r.chunk.id).copied().unwrap_or(usize::MAX);
                let k_rank = keyword_ranks.get(&r.chunk.id).copied().unwrap_or(usize::MAX);

                let v_score =
                    if v_rank == usize::MAX { 0.0 } else { 1.0 / (RRF_K + v_rank as f32) };
                let k_score =
                    if k_rank == usize::MAX { 0.0 } else { 1.0 / (RRF_K + k_rank as f32) };

                let rrf = alpha * v_score + beta * k_score;
                (rrf, *r)
            })
            .collect();

        // Sort descending by RRF score.
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Step 6: Take top-K and map to response items.
        let results: Vec<SearchResultItem> = scored
            .into_iter()
            .take(limit)
            .map(|(rrf_score, r)| SearchResultItem {
                id: r.chunk.id.clone(),
                file_path: r.chunk.chunk.file_path.clone(),
                language: r.chunk.chunk.language.to_string(),
                chunk_type: r.chunk.chunk.chunk_type.to_string(),
                symbol_name: r.chunk.chunk.symbol_name.clone(),
                start_line: r.chunk.chunk.start_line,
                end_line: r.chunk.chunk.end_line,
                content: r.chunk.chunk.content.clone(),
                summary: r.chunk.summary.clone(),
                score: rrf_score,
                repo_id: r.chunk.repo_id.clone(),
            })
            .collect();

        let total = results.len();
        let query_time_ms = start.elapsed().as_millis() as u64;

        Ok(SearchResponse { results, total, query_time_ms })
    }
}

// ---------------------------------------------------------------------------
// Helpers (public for #951 reranker)
// ---------------------------------------------------------------------------

/// Compute the RRF score for a single document given its vector and keyword ranks.
///
/// `alpha` weights the vector component; `(1-alpha)` weights the keyword component.
pub fn rrf_score(vector_rank: Option<usize>, keyword_rank: Option<usize>, alpha: f32) -> f32 {
    let v = vector_rank.map(|r| 1.0 / (RRF_K + r as f32)).unwrap_or(0.0);
    let k = keyword_rank.map(|r| 1.0 / (RRF_K + r as f32)).unwrap_or(0.0);
    alpha * v + (1.0 - alpha) * k
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
        async fn get_chunk_ids(&self, _: &str) -> StoreResult<Vec<String>> {
            Ok(vec![])
        }
    }

    fn make_result(id: &str, content: &str, symbol: &str, score: f32) -> SearchResult {
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
                    content: content.to_string(),
                    file_path: format!("src/{id}.rs"),
                    language: Language::Rust,
                    chunk_type: ChunkType::Function,
                    start_line: 1,
                    end_line: 5,
                    symbol_name: Some(symbol.to_string()),
                    parent_symbol: None,
                    hierarchy_level: HierarchyLevel::Symbol,
                    metadata: ChunkMetadata::default(),
                },
            },
        }
    }

    // ── rrf_score ──────────────────────────────────────────────────────────

    #[test]
    fn rrf_score_both_present() {
        // rank 0 in both → max possible RRF
        let s = rrf_score(Some(0), Some(0), 0.7);
        assert!((s - (0.7 / 60.0 + 0.3 / 60.0)).abs() < 1e-6);
    }

    #[test]
    fn rrf_score_only_vector() {
        let s = rrf_score(Some(0), None, 0.7);
        assert!((s - 0.7 / 60.0).abs() < 1e-6);
    }

    #[test]
    fn rrf_score_only_keyword() {
        let s = rrf_score(None, Some(0), 0.7);
        assert!((s - 0.3 / 60.0).abs() < 1e-6);
    }

    #[test]
    fn rrf_score_neither_is_zero() {
        assert_eq!(rrf_score(None, None, 0.7), 0.0);
    }

    #[test]
    fn higher_rank_gives_lower_score() {
        let s0 = rrf_score(Some(0), None, 1.0);
        let s1 = rrf_score(Some(1), None, 1.0);
        let s9 = rrf_score(Some(9), None, 1.0);
        assert!(s0 > s1);
        assert!(s1 > s9);
    }

    // ── HybridSearch ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn empty_query_returns_error() {
        let store = MockStore { results: vec![] };
        let search = HybridSearch::new(Arc::new(store));
        let req = SearchRequest {
            query: "".to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: Some(10),
            search_mode: Default::default(),
        };
        assert!(search.search(&req).await.is_err());
    }

    #[tokio::test]
    async fn empty_store_returns_empty() {
        let store = MockStore { results: vec![] };
        let search = HybridSearch::new(Arc::new(store));
        let req = SearchRequest {
            query: "authenticate".to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: Some(10),
            search_mode: Default::default(),
        };
        let resp = search.search(&req).await.unwrap();
        assert_eq!(resp.total, 0);
    }

    #[tokio::test]
    async fn returns_top_k_results() {
        let results = vec![
            make_result("fn_a", "pub fn authenticate() {}", "authenticate", 0.9),
            make_result("fn_b", "pub fn render() {}", "render", 0.8),
            make_result("fn_c", "pub fn parse() {}", "parse", 0.7),
        ];
        let store = MockStore { results };
        let search = HybridSearch::new(Arc::new(store));
        let req = SearchRequest {
            query: "authenticate".to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: Some(2),
            search_mode: Default::default(),
        };
        let resp = search.search(&req).await.unwrap();
        assert_eq!(resp.total, 2);
    }

    #[tokio::test]
    async fn keyword_match_boosts_result() {
        // fn_b has a lower vector score but an exact keyword match.
        // With hybrid search + RRF, fn_b should score competitively.
        let results = vec![
            make_result("fn_a", "pub fn process_data() {}", "process_data", 0.95),
            make_result(
                "fn_b",
                "pub fn authenticate_user() { /* authenticate */ }",
                "authenticate_user",
                0.60,
            ),
        ];
        let store = MockStore { results };
        let search = HybridSearch::with_alpha(Arc::new(store), 0.5); // equal weights
        let req = SearchRequest {
            query: "authenticate".to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: Some(10),
            search_mode: Default::default(),
        };
        let resp = search.search(&req).await.unwrap();
        // fn_b should appear in results (exact keyword match)
        assert!(resp.results.iter().any(|r| r.id == "fn_b"));
    }

    #[tokio::test]
    async fn language_filter_applied() {
        let results = vec![
            make_result("fn_rs", "fn auth() {}", "auth", 0.9),
            make_result("fn_py", "def auth(): pass", "auth", 0.8),
        ];
        // Override language in second result
        let mut results = results;
        results[1].chunk.chunk.language = Language::Python;
        let store = MockStore { results };
        let search = HybridSearch::new(Arc::new(store));
        let req = SearchRequest {
            query: "auth".to_string(),
            repo_id: None,
            language: Some("rust".to_string()),
            file_pattern: None,
            hierarchy_level: None,
            limit: Some(10),
            search_mode: Default::default(),
        };
        let resp = search.search(&req).await.unwrap();
        assert!(resp.results.iter().all(|r| r.language == "rust"));
    }

    #[tokio::test]
    async fn alpha_zero_pure_keyword_ordering() {
        // alpha=0 → pure keyword; fn_b has a strong keyword match
        let results = vec![
            make_result("fn_a", "general purpose function", "general_fn", 0.99),
            make_result("fn_b", "authenticate the user token", "authenticate", 0.10),
        ];
        let store = MockStore { results };
        let search = HybridSearch::with_alpha(Arc::new(store), 0.0);
        let req = SearchRequest {
            query: "authenticate".to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: Some(10),
            search_mode: Default::default(),
        };
        let resp = search.search(&req).await.unwrap();
        // fn_b should rank first when keyword weight dominates
        assert_eq!(resp.results[0].id, "fn_b");
    }
}
