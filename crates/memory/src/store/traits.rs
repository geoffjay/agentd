//! [`VectorStore`] and [`EmbeddingService`] trait definitions.
//!
//! These traits decouple the memory service from specific storage backends
//! (LanceDB, ChromaDB, …) and embedding providers (OpenAI, Ollama, …),
//! making it straightforward to swap implementations or inject test doubles.

use async_trait::async_trait;

use crate::error::StoreResult;
use crate::types::{CreateMemoryRequest, Memory, SearchRequest, VisibilityLevel};

/// Async trait implemented by vector-database storage backends.
///
/// A `VectorStore` persists [`Memory`] records together with their embedding
/// vectors and provides semantic similarity search over those vectors.
///
/// All operations are async and return [`StoreResult`], which wraps
/// [`crate::error::StoreError`].
///
/// # Example implementation sketch
///
/// ```rust,ignore
/// use async_trait::async_trait;
/// use memory::store::VectorStore;
/// use memory::error::StoreResult;
/// use memory::types::{CreateMemoryRequest, Memory, SearchRequest, VisibilityLevel};
///
/// struct MyStore;
///
/// #[async_trait]
/// impl VectorStore for MyStore {
///     async fn initialize(&self) -> StoreResult<()> { Ok(()) }
///     // … implement remaining methods …
/// }
/// ```
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Persist a new memory derived from `request`.
    ///
    /// Generates a unique ID, timestamps, and stores the embedding vector.
    /// Returns the fully-populated [`Memory`] record.
    async fn create(&self, request: CreateMemoryRequest) -> StoreResult<Memory>;

    /// Retrieve a single memory by its ID.
    ///
    /// Returns `None` when no record with the given `id` exists.
    async fn get(&self, id: &str) -> StoreResult<Option<Memory>>;

    /// Remove a memory by ID.
    ///
    /// Returns `true` if a record was deleted, `false` if it was not found.
    async fn delete(&self, id: &str) -> StoreResult<bool>;

    /// Perform a semantic similarity search.
    ///
    /// Results are filtered by the `as_actor` field in `request` using the
    /// three-tier visibility model before being returned to the caller.
    async fn search(&self, request: SearchRequest) -> StoreResult<Vec<Memory>>;

    /// Change the visibility (and optional share list) of a memory.
    ///
    /// Returns the updated [`Memory`] record.
    async fn update_visibility(
        &self,
        id: &str,
        visibility: VisibilityLevel,
        shared_with: Option<Vec<String>>,
    ) -> StoreResult<Memory>;

    /// Return `true` when the backend is reachable and operational.
    async fn health_check(&self) -> StoreResult<bool>;

    /// Create collections, indexes, and any other one-time setup required
    /// by the backend.
    async fn initialize(&self) -> StoreResult<()>;

    /// Return all memory records (used for migration and backup).
    async fn list_all(&self) -> StoreResult<Vec<Memory>>;
}

/// Async trait for text-embedding providers.
///
/// An `EmbeddingService` converts text strings into fixed-dimension float
/// vectors suitable for storage in a vector database.
///
/// # Example
///
/// ```rust,ignore
/// use async_trait::async_trait;
/// use memory::store::EmbeddingService;
/// use memory::error::StoreResult;
///
/// struct MyEmbedder;
///
/// #[async_trait]
/// impl EmbeddingService for MyEmbedder {
///     async fn embed(&self, texts: &[String]) -> StoreResult<Vec<Vec<f32>>> {
///         // call your embedding API …
///         Ok(vec![])
///     }
///
///     fn dimension(&self, model: &str) -> usize {
///         match model {
///             "small" => 384,
///             _ => 1536,
///         }
///     }
/// }
/// ```
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Generate embedding vectors for the provided `texts`.
    ///
    /// Returns one vector per input text, preserving order.
    /// Returns an empty `Vec` when `texts` is empty.
    async fn embed(&self, texts: &[String]) -> StoreResult<Vec<Vec<f32>>>;

    /// Return the vector dimension produced by the named `model`.
    ///
    /// This is used to allocate the correct Arrow schema when
    /// initialising a LanceDB table.
    fn dimension(&self, model: &str) -> usize;
}
