//! Incremental indexing pipeline for the agentd-index service.
//!
//! [`Indexer`] orchestrates the full lifecycle of repository indexing:
//!
//! 1. **File discovery** — walk the repository tree, filter by language, and
//!    respect `.gitignore` and configured ignore patterns.
//! 2. **File hashing** — compute SHA-256 of each file in parallel.
//! 3. **Change detection** — compare current hashes against stored hashes to
//!    identify added, modified, and deleted files.
//! 4. **Incremental re-indexing** — only chunk and embed files that changed;
//!    delete stale chunks for removed/modified files.
//! 5. **Batch embedding** — group chunks and embed in configurable batches to
//!    minimise round-trips to the embedding service.
//! 6. **Progress reporting** — emit [`IndexProgress`] events so callers can
//!    display real-time progress.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use index::indexer::{Indexer, IndexerConfig};
//! use index::store::NoOpEmbedding;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! // In production, use create_store() with real LanceDB + Ollama config.
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::chunking::semantic::{SemanticChunker, SemanticConfig};
use crate::chunking::types::CodeChunk;
use crate::store::traits::CodeStore;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the [`Indexer`].
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    /// Supported file extensions to index (e.g. `["rs", "py", "js", "ts"]`).
    pub extensions: Vec<String>,

    /// Directory / path segments to skip entirely (e.g. `".git"`, `"target"`).
    pub ignore_dirs: Vec<String>,

    /// Number of chunks to embed per batch call.
    ///
    /// Defaults to `32`.
    pub batch_size: usize,

    /// Maximum tokens per chunk passed to the semantic chunker.
    ///
    /// Defaults to `2000`.
    pub max_tokens_per_chunk: usize,

    /// Whether to respect `.gitignore` files found in the repository.
    ///
    /// Defaults to `true`.
    pub respect_gitignore: bool,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            extensions: vec![
                "rs".to_string(),
                "py".to_string(),
                "js".to_string(),
                "ts".to_string(),
            ],
            ignore_dirs: vec![
                ".git".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
                "dist".to_string(),
                ".agentd".to_string(),
            ],
            batch_size: 32,
            max_tokens_per_chunk: 2000,
            respect_gitignore: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Progress events
// ---------------------------------------------------------------------------

/// A progress update emitted during indexing.
#[derive(Debug, Clone)]
pub enum IndexProgress {
    /// Scanning the repository for source files.
    Scanning,
    /// Discovered `total` files matching the configured extensions.
    FilesDiscovered { total: usize },
    /// Detected which files have changed.
    ChangesDetected { added: usize, modified: usize, deleted: usize },
    /// Started processing a specific file.
    ProcessingFile { path: String, index: usize, total: usize },
    /// Finished embedding and storing a file's chunks.
    FileIndexed { path: String, chunks: usize },
    /// A file was skipped because its hash is unchanged.
    FileSkipped { path: String },
    /// A file's stale chunks were deleted.
    FileDeleted { path: String },
    /// Indexing complete.
    Done { files_indexed: usize, chunks_stored: usize },
}

// ---------------------------------------------------------------------------
// Change detection result
// ---------------------------------------------------------------------------

/// The outcome of comparing current file hashes against stored hashes.
#[derive(Debug, Default)]
pub struct ChangeSet {
    /// Files that are new and have no stored hash.
    pub added: Vec<PathBuf>,
    /// Files whose content hash differs from the stored hash.
    pub modified: Vec<PathBuf>,
    /// Files that were stored previously but no longer exist on disk.
    pub deleted: Vec<String>,
}

impl ChangeSet {
    /// Returns `true` when there are no changes to process.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }

    /// Total number of files that require action (add + modify + delete).
    pub fn total_affected(&self) -> usize {
        self.added.len() + self.modified.len() + self.deleted.len()
    }
}

// ---------------------------------------------------------------------------
// Indexer
// ---------------------------------------------------------------------------

/// Orchestrates incremental repository indexing.
pub struct Indexer {
    store: Arc<dyn CodeStore>,
    config: IndexerConfig,
    chunker: SemanticChunker,
}

impl Indexer {
    /// Create a new [`Indexer`] with the given store and configuration.
    pub fn new(store: Arc<dyn CodeStore>, config: IndexerConfig) -> Self {
        let chunker = SemanticChunker::new(SemanticConfig {
            max_tokens: config.max_tokens_per_chunk,
            overlap_lines: 10,
        });
        Self { store, config, chunker }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Index a repository at `repo_path` using `repo_id` as the identifier.
    ///
    /// Emits [`IndexProgress`] events on `progress_tx` (if provided).
    /// Returns `(files_indexed, chunks_stored)`.
    pub async fn index_repository(
        &self,
        repo_path: &Path,
        repo_id: &str,
        progress_tx: Option<mpsc::Sender<IndexProgress>>,
    ) -> anyhow::Result<(usize, usize)> {
        let send = |event: IndexProgress| {
            if let Some(ref tx) = progress_tx {
                let _ = tx.try_send(event);
            }
        };

        send(IndexProgress::Scanning);

        // 1. Discover source files.
        let files = self.discover_files(repo_path).await?;
        send(IndexProgress::FilesDiscovered { total: files.len() });
        info!("Discovered {} source files in '{}'", files.len(), repo_path.display());

        // 2. Hash all files in parallel.
        let current_hashes = self.hash_files(&files).await?;

        // 3. Load stored hashes and compute changeset.
        let stored_hashes = self.store.list_file_hashes(repo_id).await.unwrap_or_default();
        let changeset = compute_changeset(&files, &current_hashes, &stored_hashes, repo_path);

        send(IndexProgress::ChangesDetected {
            added: changeset.added.len(),
            modified: changeset.modified.len(),
            deleted: changeset.deleted.len(),
        });
        info!(
            "Change detection: +{} ~{} -{} files",
            changeset.added.len(),
            changeset.modified.len(),
            changeset.deleted.len()
        );

        if changeset.is_empty() {
            info!("No changes detected — nothing to index.");
            send(IndexProgress::Done { files_indexed: 0, chunks_stored: 0 });
            return Ok((0, 0));
        }

        // 4. Delete chunks for removed and modified files.
        for file_path in &changeset.deleted {
            self.store.delete_file_chunks(repo_id, file_path).await?;
            send(IndexProgress::FileDeleted { path: file_path.clone() });
        }
        for abs_path in &changeset.modified {
            let rel = relative_path(abs_path, repo_path);
            self.store.delete_file_chunks(repo_id, &rel).await?;
        }

        // 5. Index added + modified files.
        let to_index: Vec<&PathBuf> =
            changeset.added.iter().chain(changeset.modified.iter()).collect();
        let total_to_index = to_index.len();
        let mut files_indexed = 0usize;
        let mut chunks_stored = 0usize;

        // Skip over files that were neither added nor modified.
        for (idx, file) in files.iter().enumerate() {
            let rel = relative_path(file, repo_path);
            if !to_index.contains(&file) {
                send(IndexProgress::FileSkipped { path: rel });
                continue;
            }

            send(IndexProgress::ProcessingFile {
                path: rel.clone(),
                index: idx + 1,
                total: total_to_index,
            });

            let file_hash = current_hashes
                .get(file)
                .cloned()
                .unwrap_or_else(|| compute_hash_sync(file).unwrap_or_default());

            match self.index_file(repo_id, file, repo_path, &file_hash).await {
                Ok(n) => {
                    send(IndexProgress::FileIndexed { path: rel, chunks: n });
                    chunks_stored += n;
                    files_indexed += 1;
                }
                Err(e) => {
                    warn!("Failed to index '{}': {}", rel, e);
                }
            }
        }

        info!("Indexing complete: {} files, {} chunks stored", files_indexed, chunks_stored);
        send(IndexProgress::Done { files_indexed, chunks_stored });
        Ok((files_indexed, chunks_stored))
    }

    // -----------------------------------------------------------------------
    // File discovery
    // -----------------------------------------------------------------------

    /// Walk `root` and return all paths whose extension matches the configured
    /// `extensions`, respecting `.gitignore` and `ignore_dirs`.
    pub async fn discover_files(&self, root: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let config = self.config.clone();
        let root = root.to_path_buf();

        // Spawn blocking I/O on a dedicated thread to avoid blocking tokio.
        let files =
            tokio::task::spawn_blocking(move || discover_files_blocking(&root, &config)).await??;

        Ok(files)
    }

    // -----------------------------------------------------------------------
    // Hashing
    // -----------------------------------------------------------------------

    /// Hash all `files` in parallel using tokio tasks.
    ///
    /// Returns a map from absolute path → hex-encoded SHA-256 digest.
    pub async fn hash_files(&self, files: &[PathBuf]) -> anyhow::Result<HashMap<PathBuf, String>> {
        let mut handles = Vec::with_capacity(files.len());
        for path in files {
            let path = path.clone();
            handles.push(tokio::spawn(async move {
                let hash = hash_file(&path).await?;
                Ok::<(PathBuf, String), anyhow::Error>((path, hash))
            }));
        }

        let mut result = HashMap::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(Ok((path, hash))) => {
                    result.insert(path, hash);
                }
                Ok(Err(e)) => warn!("Failed to hash file: {}", e),
                Err(e) => warn!("Hashing task panicked: {}", e),
            }
        }
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // File indexing
    // -----------------------------------------------------------------------

    /// Read, chunk, embed, and store a single file.
    ///
    /// Returns the number of chunks stored.
    async fn index_file(
        &self,
        repo_id: &str,
        abs_path: &Path,
        repo_root: &Path,
        file_hash: &str,
    ) -> anyhow::Result<usize> {
        let rel = relative_path(abs_path, repo_root);
        let source = fs::read_to_string(abs_path).await?;

        // Detect language and chunk the file.
        let chunks = match self.chunker.chunk_path(abs_path, &source) {
            Ok(c) => c,
            Err(e) => {
                debug!("Skipping '{}': chunker error: {}", rel, e);
                return Ok(0);
            }
        };

        if chunks.is_empty() {
            debug!("No chunks extracted from '{}'", rel);
            return Ok(0);
        }

        // Store in batches to limit embedding round-trips.
        let mut total_stored = 0usize;
        for batch in chunks.chunks(self.config.batch_size) {
            let batch_vec: Vec<CodeChunk> = batch.to_vec();
            let n = batch_vec.len();
            self.store.store_chunks(repo_id, file_hash, batch_vec).await?;
            total_stored += n;
        }

        debug!("Indexed '{}': {} chunks", rel, total_stored);
        Ok(total_stored)
    }
}

// ---------------------------------------------------------------------------
// Standalone helpers
// ---------------------------------------------------------------------------

/// Compute the relative path of `abs` with respect to `root`.
///
/// Falls back to the absolute path string when stripping fails.
pub fn relative_path(abs: &Path, root: &Path) -> String {
    abs.strip_prefix(root).unwrap_or(abs).to_string_lossy().to_string()
}

/// Async SHA-256 hash of a file's contents.
pub async fn hash_file(path: &Path) -> anyhow::Result<String> {
    let bytes = fs::read(path).await?;
    Ok(hex_digest(&bytes))
}

/// Synchronous SHA-256 hash (for use in `spawn_blocking` contexts).
pub fn compute_hash_sync(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex_digest(&bytes))
}

/// Compute a hex-encoded SHA-256 digest of raw bytes.
pub fn hex_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Blocking file-system walk using the `ignore` crate.
///
/// Runs inside `spawn_blocking`; must not call async functions.
fn discover_files_blocking(root: &Path, config: &IndexerConfig) -> anyhow::Result<Vec<PathBuf>> {
    use ignore::WalkBuilder;

    let mut builder = WalkBuilder::new(root);
    builder.hidden(false); // include dotfiles unless gitignored
    builder.git_ignore(config.respect_gitignore);
    builder.git_global(config.respect_gitignore);

    let ext_set: std::collections::HashSet<&str> =
        config.extensions.iter().map(|s| s.as_str()).collect();
    let ignore_set: std::collections::HashSet<&str> =
        config.ignore_dirs.iter().map(|s| s.as_str()).collect();

    let mut files = Vec::new();
    for entry in builder.build().flatten() {
        let path = entry.path().to_path_buf();
        if !path.is_file() {
            continue;
        }

        // Skip paths whose components match any ignore_dirs entry.
        let skip = path
            .components()
            .any(|c| c.as_os_str().to_str().map(|s| ignore_set.contains(s)).unwrap_or(false));
        if skip {
            continue;
        }

        // Filter by extension.
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext_set.contains(ext) {
                files.push(path);
            }
        }
    }

    files.sort(); // deterministic ordering
    Ok(files)
}

/// Compare current file hashes against stored hashes to produce a [`ChangeSet`].
pub fn compute_changeset(
    current_files: &[PathBuf],
    current_hashes: &HashMap<PathBuf, String>,
    stored_hashes: &[(String, String)], // (file_path, file_hash)
    repo_root: &Path,
) -> ChangeSet {
    // Build a lookup map: relative_path → stored_hash.
    let stored: HashMap<String, String> =
        stored_hashes.iter().map(|(p, h)| (p.clone(), h.clone())).collect();

    // Build a set of current relative paths.
    let current_rel: std::collections::HashSet<String> =
        current_files.iter().map(|p| relative_path(p, repo_root)).collect();

    let mut changeset = ChangeSet::default();

    // Added + modified.
    for abs in current_files {
        let rel = relative_path(abs, repo_root);
        let current_hash = current_hashes.get(abs).cloned().unwrap_or_default();
        match stored.get(&rel) {
            None => changeset.added.push(abs.clone()),
            Some(stored_hash) if stored_hash != &current_hash => {
                changeset.modified.push(abs.clone())
            }
            _ => {} // unchanged
        }
    }

    // Deleted — stored paths that no longer exist on disk.
    for rel in stored.keys() {
        if !current_rel.contains(rel.as_str()) {
            changeset.deleted.push(rel.clone());
        }
    }

    changeset
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::fs;

    use crate::store::error::StoreResult;
    use crate::store::traits::{CodeStore, SearchResult};

    // ── NoOpStore test helper ─────────────────────────────────────────────

    /// A [`CodeStore`] that silently discards all writes and returns empty reads.
    ///
    /// Used in unit tests that need a store but don't need real storage.
    pub(crate) struct NoOpStore;

    #[async_trait]
    impl CodeStore for NoOpStore {
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
            chunks: Vec<CodeChunk>,
        ) -> StoreResult<Vec<String>> {
            Ok(chunks.iter().enumerate().map(|(i, _)| format!("id_{}", i)).collect())
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
            _limit: usize,
        ) -> StoreResult<Vec<SearchResult>> {
            Ok(vec![])
        }
        async fn update_summary(&self, _chunk_id: &str, _summary: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn list_unsummarized_chunks(
            &self,
            _repo_id: &str,
            _limit: usize,
        ) -> StoreResult<Vec<crate::store::traits::StoredChunk>> {
            Ok(vec![])
        }
        async fn sample_chunks(
            &self,
            _repo_id: &str,
            _limit: usize,
        ) -> StoreResult<Vec<crate::store::traits::StoredChunk>> {
            Ok(vec![])
        }
    }

    // ── hex_digest ────────────────────────────────────────────────────────

    #[test]
    fn hex_digest_is_deterministic() {
        let a = hex_digest(b"hello");
        let b = hex_digest(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn hex_digest_differs_for_different_input() {
        assert_ne!(hex_digest(b"hello"), hex_digest(b"world"));
    }

    #[test]
    fn hex_digest_is_64_chars() {
        // SHA-256 produces a 32-byte / 64-hex-char digest.
        let d = hex_digest(b"test");
        assert_eq!(d.len(), 64);
    }

    // ── relative_path ─────────────────────────────────────────────────────

    #[test]
    fn relative_path_strips_root() {
        let root = Path::new("/repo");
        let abs = Path::new("/repo/src/lib.rs");
        assert_eq!(relative_path(abs, root), "src/lib.rs");
    }

    #[test]
    fn relative_path_falls_back_to_abs() {
        let root = Path::new("/other");
        let abs = Path::new("/repo/src/lib.rs");
        assert_eq!(relative_path(abs, root), "/repo/src/lib.rs");
    }

    // ── compute_changeset ─────────────────────────────────────────────────

    #[test]
    fn changeset_detects_new_files() {
        let root = Path::new("/repo");
        let files = vec![PathBuf::from("/repo/src/lib.rs")];
        let mut hashes = HashMap::new();
        hashes.insert(PathBuf::from("/repo/src/lib.rs"), "abc".to_string());

        let cs = compute_changeset(&files, &hashes, &[], root);
        assert_eq!(cs.added.len(), 1);
        assert!(cs.modified.is_empty());
        assert!(cs.deleted.is_empty());
    }

    #[test]
    fn changeset_detects_modified_files() {
        let root = Path::new("/repo");
        let files = vec![PathBuf::from("/repo/src/lib.rs")];
        let mut hashes = HashMap::new();
        hashes.insert(PathBuf::from("/repo/src/lib.rs"), "new_hash".to_string());
        let stored = vec![("src/lib.rs".to_string(), "old_hash".to_string())];

        let cs = compute_changeset(&files, &hashes, &stored, root);
        assert!(cs.added.is_empty());
        assert_eq!(cs.modified.len(), 1);
        assert!(cs.deleted.is_empty());
    }

    #[test]
    fn changeset_detects_deleted_files() {
        let root = Path::new("/repo");
        let files: Vec<PathBuf> = vec![];
        let hashes: HashMap<PathBuf, String> = HashMap::new();
        let stored = vec![("src/lib.rs".to_string(), "hash".to_string())];

        let cs = compute_changeset(&files, &hashes, &stored, root);
        assert!(cs.added.is_empty());
        assert!(cs.modified.is_empty());
        assert_eq!(cs.deleted.len(), 1);
    }

    #[test]
    fn changeset_unchanged_files_not_reported() {
        let root = Path::new("/repo");
        let files = vec![PathBuf::from("/repo/src/lib.rs")];
        let mut hashes = HashMap::new();
        hashes.insert(PathBuf::from("/repo/src/lib.rs"), "same_hash".to_string());
        let stored = vec![("src/lib.rs".to_string(), "same_hash".to_string())];

        let cs = compute_changeset(&files, &hashes, &stored, root);
        assert!(cs.is_empty(), "unchanged file should not appear in changeset");
    }

    #[test]
    fn changeset_is_empty_when_no_changes() {
        let root = Path::new("/repo");
        let cs = compute_changeset(&[], &HashMap::new(), &[], root);
        assert!(cs.is_empty());
        assert_eq!(cs.total_affected(), 0);
    }

    // ── hash_file ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn hash_file_produces_consistent_result() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.rs");
        fs::write(&path, b"fn main() {}").await.unwrap();

        let h1 = hash_file(&path).await.unwrap();
        let h2 = hash_file(&path).await.unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[tokio::test]
    async fn hash_file_changes_with_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.rs");
        fs::write(&path, b"fn a() {}").await.unwrap();
        let h1 = hash_file(&path).await.unwrap();

        fs::write(&path, b"fn b() {}").await.unwrap();
        let h2 = hash_file(&path).await.unwrap();
        assert_ne!(h1, h2);
    }

    // ── discover_files_blocking ───────────────────────────────────────────

    #[tokio::test]
    async fn discover_files_finds_source_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("lib.rs"), b"pub fn f() {}").await.unwrap();
        fs::write(dir.path().join("main.py"), b"def f(): pass").await.unwrap();
        fs::write(dir.path().join("README.md"), b"# readme").await.unwrap();

        let config = IndexerConfig::default();
        let root = dir.path().to_path_buf();
        let files = tokio::task::spawn_blocking(move || discover_files_blocking(&root, &config))
            .await
            .unwrap()
            .unwrap();

        assert!(files.iter().any(|f| f.extension().and_then(|e| e.to_str()) == Some("rs")));
        assert!(files.iter().any(|f| f.extension().and_then(|e| e.to_str()) == Some("py")));
        assert!(!files.iter().any(|f| f.extension().and_then(|e| e.to_str()) == Some("md")));
    }

    #[tokio::test]
    async fn discover_files_respects_ignore_dirs() {
        let dir = TempDir::new().unwrap();
        let target_dir = dir.path().join("target");
        fs::create_dir(&target_dir).await.unwrap();
        fs::write(target_dir.join("build.rs"), b"fn main() {}").await.unwrap();
        fs::write(dir.path().join("lib.rs"), b"pub fn f() {}").await.unwrap();

        let config = IndexerConfig::default();
        let root = dir.path().to_path_buf();
        let files = tokio::task::spawn_blocking(move || discover_files_blocking(&root, &config))
            .await
            .unwrap()
            .unwrap();

        assert!(!files.iter().any(|f| f.starts_with(dir.path().join("target"))));
        assert_eq!(files.len(), 1);
    }

    // ── Indexer::hash_files ───────────────────────────────────────────────

    #[tokio::test]
    async fn indexer_hash_files_returns_all_hashes() {
        let dir = TempDir::new().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        fs::write(&a, b"fn a() {}").await.unwrap();
        fs::write(&b, b"fn b() {}").await.unwrap();

        let store = Arc::new(NoOpStore);
        let indexer = Indexer::new(store, IndexerConfig::default());
        let hashes = indexer.hash_files(&[a.clone(), b.clone()]).await.unwrap();

        assert_eq!(hashes.len(), 2);
        assert!(hashes.contains_key(&a));
        assert!(hashes.contains_key(&b));
    }
}
