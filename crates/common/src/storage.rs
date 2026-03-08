//! Shared SQLite storage utilities.
//!
//! Provides common database path resolution and connection creation
//! used by all agentd services that persist data to SQLite.
//!
//! The module offers two APIs:
//!
//! - **SeaORM (preferred)** — [`create_connection`] / [`create_test_connection`]
//! - **SQLx (deprecated)** — [`create_pool`] / [`create_test_pool`] kept for
//!   incremental migration of downstream crates
//!
//! # Examples
//!
//! ```rust,ignore
//! use agentd_common::storage::{get_db_path, create_connection};
//!
//! let db_path = get_db_path("notify", "notify.db")?;
//! let conn = create_connection(&db_path).await?;
//! ```

use anyhow::Result;
use directories::ProjectDirs;
use sea_orm::{Database, DatabaseConnection};
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

/// Create a SeaORM [`DatabaseConnection`] for the given SQLite database path.
///
/// Opens the database file in read-write-create mode. The caller is
/// responsible for running schema migrations after obtaining the connection.
///
/// [`DatabaseConnection`] is `Clone + Send + Sync` and can be shared across
/// async tasks the same way a `SqlitePool` is.
///
/// # Arguments
///
/// * `db_path` — Full path to the SQLite database file
pub async fn create_connection(db_path: &Path) -> Result<DatabaseConnection> {
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let conn = Database::connect(&db_url).await?;
    Ok(conn)
}

/// Create a temporary SeaORM [`DatabaseConnection`] for testing.
///
/// Returns a connection to a temporary file-based SQLite database. The caller
/// should hold onto the `TempDir` to keep the database alive for the duration
/// of the test.
///
/// # Examples
///
/// ```rust,ignore
/// use agentd_common::storage::create_test_connection;
///
/// let (conn, _tmp) = create_test_connection().await;
/// // Use conn for test operations...
/// // Database is cleaned up when _tmp is dropped
/// ```
pub async fn create_test_connection() -> (DatabaseConnection, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let conn = Database::connect(&db_url).await.unwrap();
    (conn, temp_dir)
}

/// Create a SQLite connection pool for the given database path.
///
/// # Deprecation
///
/// This function is deprecated in favour of [`create_connection`] which
/// returns a SeaORM [`DatabaseConnection`]. It is kept during the migration
/// period to allow downstream crates (`notify`, `orchestrator`) to migrate
/// independently without a simultaneous breaking change.
///
/// # Arguments
///
/// * `db_path` — Full path to the SQLite database file
#[deprecated(since = "0.3.0", note = "Use `create_connection` instead (SeaORM)")]
pub async fn create_pool(db_path: &Path) -> Result<SqlitePool> {
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePool::connect(&db_url).await?;
    Ok(pool)
}

/// Create a temporary SQLite pool for testing.
///
/// # Deprecation
///
/// This function is deprecated in favour of [`create_test_connection`] which
/// returns a SeaORM [`DatabaseConnection`]. Kept for incremental migration.
///
/// # Examples
///
/// ```rust,ignore
/// use agentd_common::storage::create_test_pool;
///
/// #[allow(deprecated)]
/// let (pool, _tmp) = create_test_pool().await;
/// ```
#[deprecated(since = "0.3.0", note = "Use `create_test_connection` instead (SeaORM)")]
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
        let path = get_db_path("agentd-test-common", "test.db").unwrap();
        assert!(path.to_string_lossy().contains("agentd-test-common"));
        assert!(path.to_string_lossy().ends_with("test.db"));
    }

    #[tokio::test]
    async fn test_create_connection() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let conn = create_connection(&db_path).await.unwrap();

        // Verify the connection is live
        use sea_orm::ConnectionTrait;
        conn.execute_unprepared("SELECT 1").await.unwrap();
    }

    #[tokio::test]
    async fn test_create_test_connection() {
        let (conn, _tmp) = create_test_connection().await;
        use sea_orm::ConnectionTrait;
        conn.execute_unprepared("SELECT 1").await.unwrap();
    }

    #[tokio::test]
    async fn test_create_connection_creates_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("new.db");
        assert!(!db_path.exists());
        create_connection(&db_path).await.unwrap();
        assert!(db_path.exists());
    }

    #[allow(deprecated)]
    #[tokio::test]
    async fn test_create_pool_deprecated() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        sqlx::query("SELECT 1").execute(&pool).await.unwrap();
    }

    #[allow(deprecated)]
    #[tokio::test]
    async fn test_create_test_pool_deprecated() {
        let (pool, _tmp) = create_test_pool().await;
        sqlx::query("SELECT 1").execute(&pool).await.unwrap();
    }
}
