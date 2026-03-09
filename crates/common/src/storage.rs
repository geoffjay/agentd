//! Shared SQLite storage utilities.
//!
//! Provides common database path resolution and SeaORM connection creation
//! used by all agentd services that persist data to SQLite.
//!
//! All database access goes through SeaORM's [`DatabaseConnection`], which
//! is `Clone + Send + Sync` and can be shared safely across async tasks.
//!
//! # Examples
//!
//! ```rust,ignore
//! use agentd_common::storage::{get_db_path, create_connection};
//!
//! let db_path = get_db_path("agentd-notify", "notify.db")?;
//! let conn = create_connection(&db_path).await?;
//! ```

use anyhow::Result;
use directories::ProjectDirs;
use sea_orm::{Database, DatabaseConnection};
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
/// async tasks without wrapping in `Arc`.
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
/// Returns a connection to a temporary file-based SQLite database alongside a
/// [`tempfile::TempDir`] that must be kept alive for the duration of the test
/// (dropping it deletes the database file).
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

/// Apply all pending SeaORM migrations for migrator `M` to the database at `db_path`.
///
/// Creates the database file if it does not exist.
pub async fn apply_migrations<M: sea_orm_migration::MigratorTrait>(db_path: &Path) -> Result<()> {
    let db = create_connection(db_path).await?;
    M::up(&db, None).await?;
    Ok(())
}

/// Return the status of all known migrations for migrator `M` at `db_path`.
///
/// Each entry is `(migration_name, is_applied)`.
pub async fn migration_status<M: sea_orm_migration::MigratorTrait>(
    db_path: &Path,
) -> Result<Vec<(String, bool)>> {
    let db = create_connection(db_path).await?;
    let statuses = M::get_migration_with_status(&db).await?;
    Ok(statuses
        .into_iter()
        .map(|m: sea_orm_migration::Migration| {
            let applied = m.status() == sea_orm_migration::MigrationStatus::Applied;
            (m.name().to_string(), applied)
        })
        .collect())
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
}
