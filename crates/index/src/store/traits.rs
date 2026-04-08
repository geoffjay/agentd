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

    /// Update the LLM-generated summary for a stored chunk.
    ///
    /// Used by the enrichment pipeline after generating a summary.
    async fn update_summary(&self, chunk_id: &str, summary: &str) -> StoreResult<()>;

    /// Return up to `limit` stored chunks that have no summary yet.
    ///
    /// Used by the background enrichment task to find chunks to process.
    async fn list_unsummarized_chunks(
        &self,
        repo_id: &str,
        limit: usize,
    ) -> StoreResult<Vec<StoredChunk>>;

    /// Return the total number of chunks stored for a repository.
    ///
    /// Used by the embeddings/sample endpoint to report the true total
    /// alongside the sampled subset, so the UI can display accurate
    /// "N / TOTAL chunks" statistics.
    async fn count_chunks(&self, repo_id: &str) -> StoreResult<usize>;

    /// Return up to `limit` stored chunks for a repository (any summary state).
    ///
    /// Used for embedding visualisation — returns a representative sample of
    /// all indexed chunks regardless of whether they have been summarised.
    async fn sample_chunks(&self, repo_id: &str, limit: usize) -> StoreResult<Vec<StoredChunk>>;

    /// Return all chunk IDs for a repository.
    ///
    /// Fetches only the `id` column, making it much cheaper than loading full
    /// chunks.  Used by the hex-bin density endpoint to project every chunk
    /// into the 2D space and aggregate into bins without materialising the
    /// full record data.
    async fn get_chunk_ids(&self, repo_id: &str) -> StoreResult<Vec<String>>;
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
    /// This returns a statically-known value from a lookup table and may
    /// be incorrect for models not in the table.  Prefer [`probe_dimension`]
    /// when an async call is possible.
    fn dimension(&self, model: &str) -> usize;

    /// Probe the actual embedding dimension by making a live API call.
    ///
    /// Embeds a single whitespace string and returns the length of the
    /// resulting vector.  This is the authoritative dimension to use when
    /// creating a schema or validating an existing one.
    ///
    /// Returns the statically-known dimension as a fallback when the API call
    /// fails (e.g. the embedding server is not yet running).
    async fn probe_dimension(&self) -> usize {
        match self.embed(&[" ".to_string()]).await {
            Ok(vecs) => {
                vecs.into_iter().next().map(|v| v.len()).unwrap_or_else(|| self.dimension(""))
            }
            Err(_) => self.dimension(""),
        }
    }
}
