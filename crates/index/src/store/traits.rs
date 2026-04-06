//! [`CodeStore`] and [`EmbeddingService`] trait definitions.
//!
//! These traits decouple the index service from specific storage backends
//! and embedding providers, making it straightforward to swap implementations
//! or inject test doubles.

use async_trait::async_trait;

use super::error::StoreResult;
use crate::chunking::types::CodeChunk;

// ---------------------------------------------------------------------------
// StoredChunk — the persisted form of a CodeChunk
// ---------------------------------------------------------------------------

/// A [`CodeChunk`] that has been stored in the vector database.
///
/// Extends [`CodeChunk`] with storage-level metadata such as a unique ID,
/// repo identifier, optional LLM-generated summary, and file hash.
#[derive(Debug, Clone)]
pub struct StoredChunk {
    /// Unique chunk ID (`chunk_<hash>_<seq>` format).
    pub id: String,
    /// Repository identifier (e.g. remote URL or local path).
    pub repo_id: String,
    /// SHA-256 hash of the source file at index time.
    pub file_hash: String,
    /// Optional LLM-generated natural-language summary.
    pub summary: Option<String>,
    /// RFC3339 timestamp when this chunk was first indexed.
    pub created_at: String,
    /// RFC3339 timestamp when this chunk was last updated.
    pub updated_at: String,
    /// The underlying code chunk data.
    pub chunk: CodeChunk,
}

// ---------------------------------------------------------------------------
// SearchResult
// ---------------------------------------------------------------------------

/// A single result from a semantic similarity search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matching stored chunk.
    pub chunk: StoredChunk,
    /// Cosine similarity score (higher = more similar).
    pub score: f32,
}

// ---------------------------------------------------------------------------
// CodeStore trait
// ---------------------------------------------------------------------------

/// Async trait implemented by vector-database storage backends for code chunks.
///
/// A `CodeStore` persists [`StoredChunk`] records together with their
/// embedding vectors and provides semantic similarity search.
#[async_trait]
pub trait CodeStore: Send + Sync {
    /// Create the table schema and any required indexes.
    ///
    /// Safe to call multiple times — a no-op if the table already exists.
    async fn initialize(&self) -> StoreResult<()>;

    /// Return `true` when the backend is reachable and operational.
    async fn health_check(&self) -> StoreResult<bool>;

    /// Store a batch of chunks for the given repository.
    ///
    /// `repo_id` — the repository identifier (URL or local path).
    /// `file_hash` — SHA-256 hash of the source file the chunks came from.
    /// `chunks` — code chunks to store.
    ///
    /// Returns the IDs of the stored chunks.
    async fn store_chunks(
        &self,
        repo_id: &str,
        file_hash: &str,
        chunks: Vec<CodeChunk>,
    ) -> StoreResult<Vec<String>>;

    /// Delete all chunks belonging to a specific file within a repository.
    ///
    /// Used during incremental re-indexing to remove stale chunks before
    /// storing updated ones.  Returns the number of rows deleted.
    async fn delete_file_chunks(&self, repo_id: &str, file_path: &str) -> StoreResult<usize>;

    /// Retrieve the stored SHA-256 hash for a file, if it has been indexed.
    ///
    /// Returns `None` when the file has not yet been indexed.
    async fn get_file_hash(&self, repo_id: &str, file_path: &str) -> StoreResult<Option<String>>;

    /// Retrieve all distinct (file_path, file_hash) pairs indexed for a repo.
    ///
    /// Used during incremental indexing to detect deleted files.
    async fn list_file_hashes(&self, repo_id: &str) -> StoreResult<Vec<(String, String)>>;

    /// Semantic vector similarity search over stored chunks.
    ///
    /// Returns up to `limit` results ordered by descending similarity.
    async fn search(
        &self,
        query: &str,
        repo_id: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<SearchResult>>;
}

// ---------------------------------------------------------------------------
// EmbeddingService trait
// ---------------------------------------------------------------------------

/// Async trait for text-embedding providers.
///
/// Converts text strings into fixed-dimension float vectors suitable for
/// storage in a vector database.
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Generate embedding vectors for `texts`.
    ///
    /// Returns one vector per input text, preserving order.
    /// Returns an empty `Vec` when `texts` is empty.
    async fn embed(&self, texts: &[String]) -> StoreResult<Vec<Vec<f32>>>;

    /// Return the vector dimension produced by the named `model`.
    ///
    /// When `model` is empty the instance's configured model is used.
    fn dimension(&self, model: &str) -> usize;
}
