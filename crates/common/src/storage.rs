//! Shared SQLite storage utilities.
//!
//! Provides common database path resolution and connection pool creation
//! used by all agentd services that persist data to SQLite.
//!
//! # Examples
//!
//! ```rust,ignore
//! use agentd_common::storage::{get_db_path, create_pool};
//!
//! let db_path = get_db_path("notify", "notify.db")?;
//! let pool = create_pool(&db_path).await?;
//! ```

use anyhow::Result;
use directories::ProjectDirs;
use sqlx::sqlite::SqlitePool;
use std::path::{Path, PathBuf};

/// Resolve the platform-specific database file path for a service.
///
/// Uses the XDG base directory specification (via `directories` crate) to
/// determine the data directory, creates it if necessary, and returns the
/// full path to the database file.
///
/// # Arguments
///
/// * `project_name` — XDG project qualifier (e.g., `"agentd-notify"`)
/// * `db_filename` — Database filename (e.g., `"notify.db"`)
///
/// # Examples
///
/// ```rust,ignore
/// let path = agentd_common::storage::get_db_path("agentd-notify", "notify.db")?;
/// // macOS: ~/Library/Application Support/agentd-notify/notify.db
/// // Linux: ~/.local/share/agentd-notify/notify.db
/// ```
pub fn get_db_path(project_name: &str, db_filename: &str) -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("", "", project_name)
        .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;

    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir)?;

    Ok(data_dir.join(db_filename))
}

/// Create a SQLite connection pool for the given database path.
///
/// Opens the database file in read-write-create mode. The caller is
/// responsible for running schema migrations after obtaining the pool.
///
/// # Arguments
///
/// * `db_path` — Full path to the SQLite database file
pub async fn create_pool(db_path: &Path) -> Result<SqlitePool> {
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePool::connect(&db_url).await?;
    Ok(pool)
}

/// Create a temporary SQLite pool for testing.
///
/// Returns a pool connected to a temporary file-based database. The caller
/// should hold onto the `TempDir` to keep the database alive for the
/// duration of the test.
///
/// # Examples
///
/// ```rust,ignore
/// use agentd_common::storage::create_test_pool;
/// use tempfile::TempDir;
///
/// let (pool, _tmp) = create_test_pool().await;
/// // Use pool for test operations...
/// // Database is cleaned up when _tmp is dropped
/// ```
pub async fn create_test_pool() -> (SqlitePool, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePool::connect(&db_url).await.unwrap();
    (pool, temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_db_path_returns_path() {
        // Use a unique project name to avoid interfering with real data
        let path = get_db_path("agentd-test-common", "test.db").unwrap();
        assert!(path.to_string_lossy().contains("agentd-test-common"));
        assert!(path.to_string_lossy().ends_with("test.db"));
    }

    #[tokio::test]
    async fn test_create_pool() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();

        // Verify we can execute a basic query
        sqlx::query("SELECT 1").execute(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn test_create_test_pool() {
        let (pool, _tmp) = create_test_pool().await;
        sqlx::query("SELECT 1").execute(&pool).await.unwrap();
    }
}
