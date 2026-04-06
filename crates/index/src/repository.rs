//! Repository record management for the agentd-index service.
//!
//! [`RepoStore`] is a file-backed, in-memory registry of repositories that have
//! been registered with the index service.  Records are persisted to a JSON
//! file on every mutation so that the registry survives service restarts.
//!
//! # Storage format
//!
//! Records are stored as a JSON object keyed by repository ID:
//!
//! ```json
//! {
//!   "550e8400-e29b-41d4-a716-446655440000": {
//!     "id": "550e8400-e29b-41d4-a716-446655440000",
//!     "name": "agentd",
//!     "path": "/home/user/projects/agentd",
//!     "status": "ready",
//!     "created_at": "2024-01-01T00:00:00Z",
//!     "updated_at": "2024-01-01T00:00:00Z",
//!     "last_indexed": "2024-01-01T01:00:00Z",
//!     "error_message": null
//!   }
//! }
//! ```
//!
//! # Thread safety
//!
//! [`RepoStore`] uses a [`tokio::sync::RwLock`] internally and is safe to share
//! across Axum handlers via `Arc<RepoStore>`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// RepoStatus
// ---------------------------------------------------------------------------

/// The current processing status of a registered repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RepoStatus {
    /// Registered but not yet indexed.
    #[default]
    Pending,
    /// Currently being indexed.
    Indexing,
    /// Successfully indexed and ready for search.
    Ready,
    /// Indexing failed; see `error_message` for details.
    Error,
}

// ---------------------------------------------------------------------------
// RepoRecord
// ---------------------------------------------------------------------------

/// A registered repository entry in the [`RepoStore`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRecord {
    /// Unique repository identifier (UUID v4).
    pub id: String,

    /// Human-readable name for the repository.
    pub name: String,

    /// Absolute path to the repository root on disk.
    pub path: String,

    /// Current indexing status.
    pub status: RepoStatus,

    /// ISO 8601 creation timestamp.
    pub created_at: String,

    /// ISO 8601 last-updated timestamp.
    pub updated_at: String,

    /// ISO 8601 timestamp of the last successful index run, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_indexed: Option<String>,

    /// Human-readable error description when `status == Error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl RepoRecord {
    fn now() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        let now = Self::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            path: path.into(),
            status: RepoStatus::Pending,
            created_at: now.clone(),
            updated_at: now,
            last_indexed: None,
            error_message: None,
        }
    }
}

// ---------------------------------------------------------------------------
// AddRepoRequest
// ---------------------------------------------------------------------------

/// Request body for registering a new repository.
#[derive(Debug, Clone, Deserialize)]
pub struct AddRepoRequest {
    /// Human-readable name for the repository.
    pub name: String,

    /// Absolute or relative path to the repository root.
    pub path: String,
}

// ---------------------------------------------------------------------------
// RepoStore
// ---------------------------------------------------------------------------

/// File-backed registry of indexed repositories.
///
/// # Usage
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use index::repository::RepoStore;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let store = Arc::new(RepoStore::load("/var/lib/agentd-index/repos.json").await?);
///
/// let record = store.add("my-project", "/home/user/my-project").await?;
/// println!("Registered: {} ({})", record.name, record.id);
///
/// let all = store.list().await;
/// println!("Total repos: {}", all.len());
/// # Ok(())
/// # }
/// ```
pub struct RepoStore {
    data_file: PathBuf,
    records: RwLock<HashMap<String, RepoRecord>>,
}

impl RepoStore {
    /// Create a new, empty [`RepoStore`] backed by `data_file`.
    ///
    /// Use [`RepoStore::load`] to restore persisted records from an existing
    /// file.
    pub fn new(data_file: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self { data_file: data_file.into(), records: RwLock::new(HashMap::new()) })
    }

    /// Load a [`RepoStore`] from `data_file`, creating an empty store if the
    /// file does not exist.
    pub async fn load(data_file: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = data_file.into();
        let records: HashMap<String, RepoRecord> = if path.exists() {
            let bytes = tokio::fs::read(&path).await?;
            serde_json::from_slice(&bytes).unwrap_or_default()
        } else {
            HashMap::new()
        };
        debug!(path = %path.display(), count = records.len(), "Loaded repo store");
        Ok(Self { data_file: path, records: RwLock::new(records) })
    }

    // -----------------------------------------------------------------------
    // CRUD
    // -----------------------------------------------------------------------

    /// Register a new repository.
    ///
    /// Returns the created [`RepoRecord`].  The record's initial status is
    /// [`RepoStatus::Pending`].
    pub async fn add(&self, name: &str, path: &str) -> anyhow::Result<RepoRecord> {
        let record = RepoRecord::new(name, path);
        let mut guard = self.records.write().await;
        guard.insert(record.id.clone(), record.clone());
        self.persist(&guard).await?;
        Ok(record)
    }

    /// Return all registered repositories, sorted by `created_at`.
    pub async fn list(&self) -> Vec<RepoRecord> {
        let guard = self.records.read().await;
        let mut items: Vec<RepoRecord> = guard.values().cloned().collect();
        items.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        items
    }

    /// Return a single repository by ID, or `None` if not found.
    pub async fn get(&self, id: &str) -> Option<RepoRecord> {
        self.records.read().await.get(id).cloned()
    }

    /// Remove a repository by ID.
    ///
    /// Returns `true` if the record was found and removed, `false` otherwise.
    pub async fn remove(&self, id: &str) -> anyhow::Result<bool> {
        let mut guard = self.records.write().await;
        if guard.remove(id).is_none() {
            return Ok(false);
        }
        self.persist(&guard).await?;
        Ok(true)
    }

    /// Update the status (and optional error message) for a repository.
    ///
    /// Clears `error_message` when setting status to anything other than
    /// [`RepoStatus::Error`].  Returns `true` if the record was found.
    pub async fn update_status(
        &self,
        id: &str,
        status: RepoStatus,
        error: Option<String>,
    ) -> anyhow::Result<bool> {
        let mut guard = self.records.write().await;
        let Some(record) = guard.get_mut(id) else {
            return Ok(false);
        };
        record.status = status;
        record.updated_at = RepoRecord::now();
        record.error_message = if status == RepoStatus::Error { error } else { None };
        self.persist(&guard).await?;
        Ok(true)
    }

    /// Mark a repository as successfully indexed, updating `last_indexed`.
    pub async fn set_last_indexed(&self, id: &str) -> anyhow::Result<bool> {
        let mut guard = self.records.write().await;
        let Some(record) = guard.get_mut(id) else {
            return Ok(false);
        };
        let now = RepoRecord::now();
        record.status = RepoStatus::Ready;
        record.updated_at = now.clone();
        record.last_indexed = Some(now);
        record.error_message = None;
        self.persist(&guard).await?;
        Ok(true)
    }

    // -----------------------------------------------------------------------
    // Persistence
    // -----------------------------------------------------------------------

    /// Write the current record map to disk as JSON.
    async fn persist(&self, records: &HashMap<String, RepoRecord>) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec_pretty(records)?;
        // Write to a temp file then rename for atomicity.
        let tmp = self.data_file.with_extension("json.tmp");
        if let Some(parent) = self.data_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &self.data_file).await?;
        debug!(path = %self.data_file.display(), "Persisted repo store");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn make_store() -> (RepoStore, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("repos.json");
        let store = RepoStore::load(path).await.unwrap();
        (store, dir)
    }

    // ── RepoStatus serialization ───────────────────────────────────────────

    #[test]
    fn repo_status_default_is_pending() {
        assert_eq!(RepoStatus::default(), RepoStatus::Pending);
    }

    #[test]
    fn repo_status_serialization_roundtrip() {
        for (status, s) in [
            (RepoStatus::Pending, "\"pending\""),
            (RepoStatus::Indexing, "\"indexing\""),
            (RepoStatus::Ready, "\"ready\""),
            (RepoStatus::Error, "\"error\""),
        ] {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, s, "Serialize {status:?}");
            let parsed: RepoStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status, "Deserialize {s}");
        }
    }

    // ── RepoStore CRUD ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn add_returns_pending_record() {
        let (store, _dir) = make_store().await;
        let rec = store.add("agentd", "/home/user/agentd").await.unwrap();
        assert_eq!(rec.name, "agentd");
        assert_eq!(rec.path, "/home/user/agentd");
        assert_eq!(rec.status, RepoStatus::Pending);
        assert!(rec.last_indexed.is_none());
        assert!(!rec.id.is_empty());
    }

    #[tokio::test]
    async fn list_returns_all_repos_sorted() {
        let (store, _dir) = make_store().await;
        store.add("b-repo", "/b").await.unwrap();
        store.add("a-repo", "/a").await.unwrap();
        let list = store.list().await;
        assert_eq!(list.len(), 2);
        // sorted by created_at (first added is first)
        assert_eq!(list[0].name, "b-repo");
        assert_eq!(list[1].name, "a-repo");
    }

    #[tokio::test]
    async fn get_returns_record() {
        let (store, _dir) = make_store().await;
        let rec = store.add("my-repo", "/path").await.unwrap();
        let found = store.get(&rec.id).await.unwrap();
        assert_eq!(found.id, rec.id);
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let (store, _dir) = make_store().await;
        assert!(store.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn remove_deletes_record() {
        let (store, _dir) = make_store().await;
        let rec = store.add("repo", "/r").await.unwrap();
        assert!(store.remove(&rec.id).await.unwrap());
        assert!(store.get(&rec.id).await.is_none());
    }

    #[tokio::test]
    async fn remove_missing_returns_false() {
        let (store, _dir) = make_store().await;
        assert!(!store.remove("missing").await.unwrap());
    }

    #[tokio::test]
    async fn update_status_sets_indexing() {
        let (store, _dir) = make_store().await;
        let rec = store.add("repo", "/r").await.unwrap();
        assert!(store.update_status(&rec.id, RepoStatus::Indexing, None).await.unwrap());
        let found = store.get(&rec.id).await.unwrap();
        assert_eq!(found.status, RepoStatus::Indexing);
    }

    #[tokio::test]
    async fn update_status_error_sets_message() {
        let (store, _dir) = make_store().await;
        let rec = store.add("repo", "/r").await.unwrap();
        store
            .update_status(&rec.id, RepoStatus::Error, Some("disk full".to_string()))
            .await
            .unwrap();
        let found = store.get(&rec.id).await.unwrap();
        assert_eq!(found.status, RepoStatus::Error);
        assert_eq!(found.error_message.as_deref(), Some("disk full"));
    }

    #[tokio::test]
    async fn update_status_clears_error_on_non_error() {
        let (store, _dir) = make_store().await;
        let rec = store.add("repo", "/r").await.unwrap();
        store.update_status(&rec.id, RepoStatus::Error, Some("oops".to_string())).await.unwrap();
        store.update_status(&rec.id, RepoStatus::Indexing, None).await.unwrap();
        let found = store.get(&rec.id).await.unwrap();
        assert!(found.error_message.is_none());
    }

    #[tokio::test]
    async fn set_last_indexed_marks_ready() {
        let (store, _dir) = make_store().await;
        let rec = store.add("repo", "/r").await.unwrap();
        store.set_last_indexed(&rec.id).await.unwrap();
        let found = store.get(&rec.id).await.unwrap();
        assert_eq!(found.status, RepoStatus::Ready);
        assert!(found.last_indexed.is_some());
    }

    // ── Persistence roundtrip ──────────────────────────────────────────────

    #[tokio::test]
    async fn persist_and_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("repos.json");

        let id = {
            let store = RepoStore::load(&path).await.unwrap();
            let rec = store.add("repo", "/r").await.unwrap();
            rec.id
        };

        // Reload from disk.
        let store2 = RepoStore::load(&path).await.unwrap();
        let found = store2.get(&id).await;
        assert!(found.is_some(), "Record should survive reload");
        assert_eq!(found.unwrap().name, "repo");
    }

    #[tokio::test]
    async fn load_missing_file_is_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let store = RepoStore::load(path).await.unwrap();
        assert!(store.list().await.is_empty());
    }
}
