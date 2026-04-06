//! Search strategy traits and types for the agentd-index service.
//!
//! Defines the pluggable [`SearchStrategy`] interface and the shared request /
//! response types used by all search endpoints.
//!
//! # Strategies
//!
//! | Module              | Strategy       | Description                         |
//! |---------------------|----------------|-------------------------------------|
//! | [`vector`]          | `VectorSearch` | Pure semantic (ANN) similarity      |
//! | [`keyword`]         | `KeywordIndex` | BM25 full-text search via tantivy   |
//! | [`hybrid`]          | `HybridSearch` | Vector + BM25 combined with RRF     |
//! | `rerank`  (#951)    | `Reranker`     | Cross-encoder reranking             |
//! | `agentic` (#951)    | `AgenticSearch`| grep/find-based fallback            |

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod agentic;
pub mod hybrid;
pub mod keyword;
pub mod rerank;
pub mod vector;

// ---------------------------------------------------------------------------
// SearchMode
// ---------------------------------------------------------------------------

/// The search strategy to apply for a query.
///
/// `search_mode` is accepted as part of [`SearchRequest`].  Only `Vector`
/// is implemented in this issue; `Keyword` and `Hybrid` are added in #950.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// Pure vector (semantic) similarity search — the default.
    #[default]
    Vector,
    /// BM25 keyword-only search (added in #950).
    Keyword,
    /// Vector + BM25 hybrid via Reciprocal Rank Fusion (added in #950).
    Hybrid,
}

// ---------------------------------------------------------------------------
// SearchRequest
// ---------------------------------------------------------------------------

/// Request body for the `POST /search` endpoint.
///
/// All fields except `query` are optional filters applied after retrieval.
///
/// # Example
///
/// ```json
/// {
///   "query": "function that handles HTTP authentication",
///   "repo_id": "agentd",
///   "language": "rust",
///   "file_pattern": "src/auth/**",
///   "hierarchy_level": "symbol",
///   "limit": 10
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
    /// Natural language query or identifier to search for.
    pub query: String,

    /// Optional repository filter.  When set, only chunks from this repo are returned.
    #[serde(default)]
    pub repo_id: Option<String>,

    /// Optional language filter (e.g. `"rust"`, `"python"`).
    #[serde(default)]
    pub language: Option<String>,

    /// Optional glob pattern to filter results by file path (e.g. `"src/auth/**"`).
    #[serde(default)]
    pub file_pattern: Option<String>,

    /// Optional hierarchy level filter.
    ///
    /// Accepted values: `"symbol"`, `"file"`, `"directory"`, `"repository"`.
    #[serde(default)]
    pub hierarchy_level: Option<String>,

    /// Maximum number of results to return.
    ///
    /// Defaults to `10`.  Values are clamped to the range `[1, 100]`.
    #[serde(default)]
    pub limit: Option<usize>,

    /// Search strategy to use.  Defaults to [`SearchMode::Vector`].
    ///
    /// `Keyword` and `Hybrid` modes are added in #950.
    #[serde(default)]
    pub search_mode: SearchMode,
}

// ---------------------------------------------------------------------------
// SearchResultItem
// ---------------------------------------------------------------------------

/// A single result returned inside a [`SearchResponse`].
#[derive(Debug, Clone, Serialize)]
pub struct SearchResultItem {
    /// Unique chunk identifier (`chunk_<hash>_<seq>`).
    pub id: String,

    /// Source file path of the chunk.
    pub file_path: String,

    /// Programming language (`"rust"`, `"python"`, …).
    pub language: String,

    /// Syntactic kind of the chunk (`"function"`, `"struct"`, …).
    pub chunk_type: String,

    /// Top-level symbol name (function name, struct name, …), if present.
    pub symbol_name: Option<String>,

    /// One-based line number where the chunk starts.
    pub start_line: usize,

    /// One-based line number where the chunk ends (inclusive).
    pub end_line: usize,

    /// Full source text of the chunk.
    pub content: String,

    /// LLM-generated natural-language summary of the chunk, if available.
    pub summary: Option<String>,

    /// Similarity or relevance score — higher is more relevant.
    pub score: f32,

    /// Repository identifier the chunk belongs to.
    pub repo_id: String,
}

// ---------------------------------------------------------------------------
// SearchResponse
// ---------------------------------------------------------------------------

/// Response body returned by the `POST /search` endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    /// Ranked list of matching code chunks.
    pub results: Vec<SearchResultItem>,

    /// Number of results returned (equal to `results.len()`).
    pub total: usize,

    /// Wall-clock time taken to execute the query, in milliseconds.
    pub query_time_ms: u64,
}

// ---------------------------------------------------------------------------
// SearchError
// ---------------------------------------------------------------------------

/// Errors that can occur during a search operation.
#[derive(Debug, Error)]
pub enum SearchError {
    /// The query or filters were invalid.
    #[error("invalid search request: {0}")]
    InvalidRequest(String),

    /// The backing vector store returned an error.
    #[error("search backend error: {0}")]
    Backend(String),

    /// The keyword (tantivy) index returned an error.
    #[error("keyword index error: {0}")]
    KeywordIndex(String),
}

// ---------------------------------------------------------------------------
// SearchStrategy
// ---------------------------------------------------------------------------

/// Pluggable search strategy.
///
/// Implementations:
/// - [`vector::VectorSearch`] — semantic ANN similarity (this issue)
/// - `keyword::KeywordSearch` — BM25 full-text (#950)
/// - `hybrid::HybridSearch`  — vector + BM25 combined via RRF (#950)
/// - `rerank::Reranker`      — cross-encoder re-scoring (#951)
#[async_trait]
pub trait SearchStrategy: Send + Sync {
    /// Execute the search and return ranked results.
    async fn search(&self, request: &SearchRequest) -> Result<SearchResponse, SearchError>;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check whether `file_path` matches `pattern`.
///
/// Supports the following patterns without an external glob crate:
/// - `**` / `**/*` — matches everything
/// - `src/auth/**` — directory prefix (everything under that directory)
/// - `src/auth/*` — direct children only
/// - `*.rs` — extension wildcard
/// - `src/api.rs` — exact path
/// - Anything else — substring match against the path
pub fn matches_file_pattern(file_path: &str, pattern: &str) -> bool {
    if pattern.is_empty() || pattern == "**" || pattern == "**/*" {
        return true;
    }

    // No wildcards — exact path or substring match.
    if !pattern.contains('*') && !pattern.contains('?') {
        return file_path == pattern || file_path.ends_with(pattern);
    }

    // *.ext — extension-only wildcard.
    if let Some(ext) = pattern.strip_prefix("*.").or_else(|| pattern.strip_prefix("**.")) {
        if !ext.contains('/') && !ext.contains('*') {
            return file_path.ends_with(&format!(".{ext}"));
        }
    }

    // prefix/** — everything under a directory.
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return file_path.starts_with(&format!("{prefix}/")) || file_path == prefix;
    }

    // prefix/* — direct children only (no further slashes after prefix/).
    if let Some(prefix) = pattern.strip_suffix("/*") {
        let after = file_path.strip_prefix(&format!("{prefix}/")).unwrap_or("");
        return !after.is_empty() && !after.contains('/');
    }

    // Two-part wildcard: <prefix>*<suffix>
    let parts: Vec<&str> = pattern.splitn(2, '*').collect();
    if parts.len() == 2 {
        return file_path.starts_with(parts[0]) && file_path.ends_with(parts[1]);
    }

    // Fallback: substring match on the non-wildcard portion.
    let core = pattern.trim_matches('*').trim_matches('/');
    file_path.contains(core)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── SearchMode ─────────────────────────────────────────────────────────

    #[test]
    fn search_mode_default_is_vector() {
        assert_eq!(SearchMode::default(), SearchMode::Vector);
    }

    #[test]
    fn search_mode_serialization_roundtrip() {
        for (mode, s) in [
            (SearchMode::Vector, "\"vector\""),
            (SearchMode::Keyword, "\"keyword\""),
            (SearchMode::Hybrid, "\"hybrid\""),
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(json, s);
            let parsed: SearchMode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    // ── SearchRequest ──────────────────────────────────────────────────────

    #[test]
    fn search_request_minimal_deserialize() {
        let req: SearchRequest = serde_json::from_str(r#"{"query":"hello world"}"#).unwrap();
        assert_eq!(req.query, "hello world");
        assert_eq!(req.limit, None);
        assert_eq!(req.search_mode, SearchMode::Vector);
        assert!(req.repo_id.is_none());
        assert!(req.language.is_none());
        assert!(req.file_pattern.is_none());
        assert!(req.hierarchy_level.is_none());
    }

    #[test]
    fn search_request_full_deserialize() {
        let json = r#"{
            "query": "auth middleware",
            "repo_id": "agentd",
            "language": "rust",
            "file_pattern": "src/auth/**",
            "hierarchy_level": "symbol",
            "limit": 5,
            "search_mode": "hybrid"
        }"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.limit, Some(5));
        assert_eq!(req.search_mode, SearchMode::Hybrid);
        assert_eq!(req.language.as_deref(), Some("rust"));
    }

    // ── matches_file_pattern ───────────────────────────────────────────────

    #[test]
    fn pattern_empty_matches_any() {
        assert!(matches_file_pattern("src/main.rs", ""));
    }

    #[test]
    fn pattern_double_star_matches_any() {
        assert!(matches_file_pattern("a/b/c/d.rs", "**"));
        assert!(matches_file_pattern("a/b/c/d.rs", "**/*"));
    }

    #[test]
    fn pattern_exact_path() {
        assert!(matches_file_pattern("src/api.rs", "src/api.rs"));
        assert!(!matches_file_pattern("src/api.rs", "src/other.rs"));
    }

    #[test]
    fn pattern_directory_prefix() {
        assert!(matches_file_pattern("src/auth/middleware.rs", "src/auth/**"));
        assert!(matches_file_pattern("src/auth/nested/deep.rs", "src/auth/**"));
        assert!(!matches_file_pattern("src/other/file.rs", "src/auth/**"));
    }

    #[test]
    fn pattern_direct_children_only() {
        assert!(matches_file_pattern("src/auth/middleware.rs", "src/auth/*"));
        assert!(!matches_file_pattern("src/auth/nested/deep.rs", "src/auth/*"));
    }

    #[test]
    fn pattern_extension_wildcard() {
        assert!(matches_file_pattern("src/main.rs", "*.rs"));
        assert!(matches_file_pattern("lib.rs", "*.rs"));
        assert!(!matches_file_pattern("src/main.py", "*.rs"));
    }

    #[test]
    fn pattern_two_part_wildcard() {
        assert!(matches_file_pattern("src/auth_middleware.rs", "src/auth*"));
    }
}
