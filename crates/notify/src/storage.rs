//! SQLite-based persistent storage for notifications.
//!
//! This module provides the [`NotificationStorage`] backend that persists notifications
//! to an SQLite database. It handles all CRUD operations, querying, filtering, and
//! automatic cleanup of expired notifications.
//!
//! # Database Location
//!
//! The database is stored in a platform-specific user data directory:
//! - Linux: `~/.local/share/agentd-notify/notify.db`
//! - macOS: `~/Library/Application Support/agentd-notify/notify.db`
//! - Windows: `C:\Users\<user>\AppData\Local\agentd-notify\notify.db`
//!
//! # Schema
//!
//! Notifications are stored in a single table with the following structure:
//! - `id`: TEXT PRIMARY KEY - UUID of the notification
//! - `source_type`: TEXT - Type of notification source (System, AgentHook, etc.)
//! - `source_data`: TEXT - JSON-serialized source data
//! - `lifetime_type`: TEXT - Either "Ephemeral" or "Persistent"
//! - `lifetime_expires_at`: TEXT - ISO8601 timestamp for ephemeral notifications
//! - `priority`: TEXT - Priority level (Low, Normal, High, Urgent)
//! - `status`: TEXT - Current status (Pending, Viewed, Responded, Dismissed, Expired)
//! - `title`: TEXT - Notification title
//! - `message`: TEXT - Notification message body
//! - `requires_response`: INTEGER - Boolean flag (0 or 1)
//! - `response`: TEXT - User's response text (nullable)
//! - `created_at`: TEXT - ISO8601 timestamp of creation
//! - `updated_at`: TEXT - ISO8601 timestamp of last update
//!
//! Indexes are created on `status` and `created_at` columns for efficient querying.
//!
//! # Examples
//!
//! ```no_run
//! use notify::storage::NotificationStorage;
//! use notify::notification::*;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create storage with default path
//!     let storage = NotificationStorage::new().await?;
//!
//!     // Create and store a notification
//!     let notification = Notification::new(
//!         NotificationSource::System,
//!         NotificationLifetime::Persistent,
//!         NotificationPriority::High,
//!         "Important Update".to_string(),
//!         "Please review the changes".to_string(),
//!         false,
//!     );
//!
//!     let id = storage.add(&notification).await?;
//!     println!("Stored notification: {}", id);
//!
//!     // Retrieve it
//!     let retrieved = storage.get(&id).await?;
//!     assert!(retrieved.is_some());
//!
//!     Ok(())
//! }
//! ```

use crate::notification::{
    Notification, NotificationLifetime, NotificationPriority, NotificationSource,
    NotificationStatus,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use sqlx::{sqlite::SqlitePool, Row};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Persistent storage backend for notifications using SQLite.
///
/// This struct provides a thread-safe, async interface to a SQLite database
/// for storing and retrieving notifications. It uses connection pooling via
/// [`SqlitePool`] for efficient concurrent access.
///
/// # Cloning
///
/// This type is cheaply cloneable as it wraps a connection pool. Cloning
/// creates a new handle to the same underlying database connection pool.
///
/// # Examples
///
/// ```no_run
/// use notify::storage::NotificationStorage;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     // Create with default path
///     let storage = NotificationStorage::new().await?;
///
///     // Clone for use in another task
///     let storage_clone = storage.clone();
///     tokio::spawn(async move {
///         // Use storage_clone...
///     });
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct NotificationStorage {
    /// SQLite connection pool for async database operations
    pool: SqlitePool,
}

impl NotificationStorage {
    /// Gets the platform-specific database file path.
    ///
    /// Uses the `directories` crate to determine the appropriate user data
    /// directory for the current platform, then creates the directory structure
    /// if it doesn't exist.
    ///
    /// # Platform Paths
    ///
    /// - **Linux**: `~/.local/share/agentd-notify/notify.db`
    /// - **macOS**: `~/Library/Application Support/agentd-notify/notify.db`
    /// - **Windows**: `C:\Users\<user>\AppData\Local\agentd-notify\notify.db`
    ///
    /// # Returns
    ///
    /// Returns the full path to the database file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Unable to determine the user's data directory
    /// - Unable to create the parent directories
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    ///
    /// let db_path = NotificationStorage::get_db_path().unwrap();
    /// println!("Database location: {:?}", db_path);
    /// ```
    pub fn get_db_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("", "", "agentd-notify")
            .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;

        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        Ok(data_dir.join("notify.db"))
    }

    /// Creates a new notification storage instance with the default database path.
    ///
    /// This is the recommended way to create a [`NotificationStorage`] instance.
    /// It automatically determines the platform-specific database location,
    /// creates the database if needed, and initializes the schema.
    ///
    /// # Returns
    ///
    /// Returns a new storage instance connected to the default database.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Unable to determine the database path
    /// - Unable to connect to the database
    /// - Unable to initialize the schema
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///     // Use storage...
    ///     Ok(())
    /// }
    /// ```
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    /// Creates a new notification storage instance with a custom database path.
    ///
    /// This method is useful for testing with temporary databases or when you
    /// need to specify a custom database location. The database file will be
    /// created if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the SQLite database file
    ///
    /// # Returns
    ///
    /// Returns a new storage instance connected to the specified database.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Unable to connect to the database
    /// - Unable to initialize the schema
    /// - The path is invalid or inaccessible
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    /// use std::path::Path;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let db_path = Path::new("/tmp/test.db");
    ///     let storage = NotificationStorage::with_path(db_path).await?;
    ///     // Use storage...
    ///     Ok(())
    /// }
    /// ```
    pub async fn with_path(db_path: &Path) -> Result<Self> {
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&db_url).await?;

        let storage = Self { pool };
        storage.init_schema().await?;

        Ok(storage)
    }

    /// Initializes the database schema with tables and indexes.
    ///
    /// Creates the notifications table if it doesn't exist and sets up indexes
    /// on the `status` and `created_at` columns for efficient querying. This
    /// method is called automatically by [`new`](Self::new) and
    /// [`with_path`](Self::with_path).
    ///
    /// # Errors
    ///
    /// Returns an error if the SQL commands fail to execute.
    async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                source_data TEXT NOT NULL,
                lifetime_type TEXT NOT NULL,
                lifetime_expires_at TEXT,
                priority TEXT NOT NULL,
                status TEXT NOT NULL,
                title TEXT NOT NULL,
                message TEXT NOT NULL,
                requires_response INTEGER NOT NULL,
                response TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create index on status for faster queries
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_status ON notifications(status)")
            .execute(&self.pool)
            .await?;

        // Create index on created_at for sorting
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_created_at ON notifications(created_at)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Adds a new notification to the database.
    ///
    /// Inserts the given notification into storage with all its fields.
    /// The notification's ID is generated before insertion and returned.
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification to store
    ///
    /// # Returns
    ///
    /// Returns the UUID of the stored notification.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database insert fails
    /// - JSON serialization of the source data fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    /// use notify::notification::*;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///
    ///     let notification = Notification::new(
    ///         NotificationSource::System,
    ///         NotificationLifetime::Persistent,
    ///         NotificationPriority::Normal,
    ///         "New Update".to_string(),
    ///         "Version 2.0 is available".to_string(),
    ///         false,
    ///     );
    ///
    ///     let id = storage.add(&notification).await?;
    ///     println!("Added notification with ID: {}", id);
    ///     Ok(())
    /// }
    /// ```
    pub async fn add(&self, notification: &Notification) -> Result<Uuid> {
        let id = notification.id.to_string();
        let source_data = serde_json::to_string(&notification.source)?;
        let lifetime_type = match notification.lifetime {
            NotificationLifetime::Ephemeral { .. } => "Ephemeral",
            NotificationLifetime::Persistent => "Persistent",
        };
        let lifetime_expires_at = match notification.lifetime {
            NotificationLifetime::Ephemeral { expires_at } => Some(expires_at.to_rfc3339()),
            NotificationLifetime::Persistent => None,
        };
        let priority = format!("{:?}", notification.priority);
        let status = format!("{:?}", notification.status);
        let requires_response = if notification.requires_response { 1 } else { 0 };
        let response = notification.response.as_deref();

        sqlx::query(
            r#"
            INSERT INTO notifications (
                id, source_type, source_data, lifetime_type, lifetime_expires_at,
                priority, status, title, message, requires_response, response,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(format!("{:?}", notification.source))
        .bind(&source_data)
        .bind(lifetime_type)
        .bind(lifetime_expires_at)
        .bind(&priority)
        .bind(&status)
        .bind(&notification.title)
        .bind(&notification.message)
        .bind(requires_response)
        .bind(response)
        .bind(notification.created_at.to_rfc3339())
        .bind(notification.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(notification.id)
    }

    /// Retrieves a notification by its ID.
    ///
    /// Fetches a single notification from the database by its unique identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - The UUID of the notification to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Some(Notification)` if found, or `None` if no notification
    /// exists with the given ID.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database query fails
    /// - The stored data cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    /// use uuid::Uuid;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///     let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    ///
    ///     match storage.get(&id).await? {
    ///         Some(notification) => println!("Found: {}", notification.title),
    ///         None => println!("Notification not found"),
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn get(&self, id: &Uuid) -> Result<Option<Notification>> {
        let row = sqlx::query("SELECT * FROM notifications WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_notification(&row)?)),
            None => Ok(None),
        }
    }

    /// Updates an existing notification in the database.
    ///
    /// Updates the status, response, and updated_at timestamp of a notification.
    /// Other fields are immutable after creation and cannot be changed.
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification with updated fields
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful update.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The notification doesn't exist in the database
    /// - The database update fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    /// use notify::notification::*;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///
    ///     // Get an existing notification
    ///     let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    ///     if let Some(mut notification) = storage.get(&id).await? {
    ///         // Dismiss it
    ///         notification.dismiss();
    ///
    ///         // Save changes
    ///         storage.update(&notification).await?;
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn update(&self, notification: &Notification) -> Result<()> {
        let status = format!("{:?}", notification.status);
        let response = notification.response.as_deref();

        let result = sqlx::query(
            r#"
            UPDATE notifications
            SET status = ?, response = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&status)
        .bind(response)
        .bind(notification.updated_at.to_rfc3339())
        .bind(notification.id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Notification not found");
        }

        Ok(())
    }

    /// Deletes a notification from the database.
    ///
    /// Permanently removes a notification by its ID. This operation cannot be undone.
    ///
    /// # Arguments
    ///
    /// * `id` - The UUID of the notification to delete
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful deletion.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The notification doesn't exist in the database
    /// - The database delete operation fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    /// use uuid::Uuid;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///     let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    ///
    ///     storage.delete(&id).await?;
    ///     println!("Notification deleted");
    ///     Ok(())
    /// }
    /// ```
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM notifications WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Notification not found");
        }

        Ok(())
    }

    /// Lists all notifications, optionally filtered by status.
    ///
    /// Retrieves all notifications from the database, ordered by creation time
    /// (newest first). Can optionally filter to only include notifications with
    /// a specific status.
    ///
    /// # Arguments
    ///
    /// * `status_filter` - Optional status to filter by. If `None`, returns all notifications.
    ///
    /// # Returns
    ///
    /// Returns a vector of notifications, ordered by creation time descending.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database query fails
    /// - Any notification data cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    /// use notify::notification::NotificationStatus;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///
    ///     // Get all notifications
    ///     let all = storage.list(None).await?;
    ///     println!("Total notifications: {}", all.len());
    ///
    ///     // Get only pending notifications
    ///     let pending = storage.list(Some(NotificationStatus::Pending)).await?;
    ///     println!("Pending notifications: {}", pending.len());
    ///     Ok(())
    /// }
    /// ```
    pub async fn list(
        &self,
        status_filter: Option<NotificationStatus>,
    ) -> Result<Vec<Notification>> {
        let rows = if let Some(status) = status_filter {
            let status_str = format!("{status:?}");
            sqlx::query("SELECT * FROM notifications WHERE status = ? ORDER BY created_at DESC")
                .bind(status_str)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query("SELECT * FROM notifications ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?
        };

        rows.iter().map(|row| self.row_to_notification(row)).collect()
    }

    /// Lists all actionable notifications.
    ///
    /// Returns notifications that can still be acted upon: those with Pending or
    /// Viewed status that have not expired. Results are ordered by priority
    /// (highest first), then by creation time (oldest first) to show the most
    /// urgent and longest-waiting notifications first.
    ///
    /// # Returns
    ///
    /// Returns a vector of actionable notifications, ordered by priority descending,
    /// then creation time ascending.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database query fails
    /// - Any notification data cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///
    ///     let actionable = storage.list_actionable().await?;
    ///     for notif in actionable {
    ///         println!("[{:?}] {}", notif.priority, notif.title);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn list_actionable(&self) -> Result<Vec<Notification>> {
        let rows = sqlx::query(
            "SELECT * FROM notifications WHERE status IN ('Pending', 'Viewed') ORDER BY priority DESC, created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut notifications: Vec<Notification> =
            rows.iter().map(|row| self.row_to_notification(row)).collect::<Result<Vec<_>>>()?;

        // Filter out expired notifications
        notifications.retain(|n| n.is_actionable());

        Ok(notifications)
    }

    /// Lists notification history.
    ///
    /// Returns notifications that are no longer actionable: those that have been
    /// dismissed, responded to, or expired. Results are ordered by update time
    /// (newest first) to show the most recently completed notifications first.
    ///
    /// # Returns
    ///
    /// Returns a vector of historical notifications, ordered by update time descending.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database query fails
    /// - Any notification data cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///
    ///     let history = storage.list_history().await?;
    ///     println!("Completed notifications: {}", history.len());
    ///     Ok(())
    /// }
    /// ```
    pub async fn list_history(&self) -> Result<Vec<Notification>> {
        let rows = sqlx::query(
            "SELECT * FROM notifications WHERE status IN ('Dismissed', 'Responded', 'Expired') ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(|row| self.row_to_notification(row)).collect()
    }

    /// Cleans up expired notifications.
    ///
    /// This method performs a two-step cleanup process:
    /// 1. Updates the status of expired ephemeral notifications to `Expired`
    /// 2. Deletes all notifications with `Expired` status
    ///
    /// This should be called periodically (e.g., every 5 minutes) to keep the
    /// database clean and remove notifications that are no longer relevant.
    ///
    /// # Returns
    ///
    /// Returns the number of notifications that were deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operations fail.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///
    ///     let deleted = storage.cleanup_expired().await?;
    ///     if deleted > 0 {
    ///         println!("Cleaned up {} expired notifications", deleted);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn cleanup_expired(&self) -> Result<usize> {
        // First, update status of expired notifications
        let now = Utc::now().to_rfc3339();
        let _result = sqlx::query(
            r#"
            UPDATE notifications
            SET status = 'Expired', updated_at = ?
            WHERE lifetime_type = 'Ephemeral'
            AND lifetime_expires_at < ?
            AND status = 'Pending'
            "#,
        )
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // Then delete expired notifications
        let delete_result = sqlx::query("DELETE FROM notifications WHERE status = 'Expired'")
            .execute(&self.pool)
            .await?;

        Ok(delete_result.rows_affected() as usize)
    }

    /// Gets counts of notifications grouped by status.
    ///
    /// Returns a vector of tuples containing each status and the number of
    /// notifications with that status. Useful for dashboard displays or
    /// summary statistics.
    ///
    /// # Returns
    ///
    /// Returns a vector of (status, count) tuples. Statuses with zero notifications
    /// are not included in the results.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use notify::storage::NotificationStorage;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let storage = NotificationStorage::new().await?;
    ///
    ///     let counts = storage.count().await?;
    ///     for (status, count) in counts {
    ///         println!("{:?}: {}", status, count);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[allow(dead_code)]
    pub async fn count(&self) -> Result<Vec<(NotificationStatus, usize)>> {
        let rows =
            sqlx::query("SELECT status, COUNT(*) as count FROM notifications GROUP BY status")
                .fetch_all(&self.pool)
                .await?;

        let mut counts = Vec::new();
        for row in rows {
            let status_str: String = row.get("status");
            let count: i64 = row.get("count");

            let status = match status_str.as_str() {
                "Pending" => NotificationStatus::Pending,
                "Viewed" => NotificationStatus::Viewed,
                "Responded" => NotificationStatus::Responded,
                "Dismissed" => NotificationStatus::Dismissed,
                "Expired" => NotificationStatus::Expired,
                _ => continue,
            };

            counts.push((status, count as usize));
        }

        Ok(counts)
    }

    /// Converts a SQLite row to a [`Notification`] struct.
    ///
    /// This private helper method deserializes database row data into a
    /// notification object, handling JSON deserialization, timestamp parsing,
    /// and enum conversions.
    ///
    /// # Arguments
    ///
    /// * `row` - Reference to a SQLite row from a query result
    ///
    /// # Returns
    ///
    /// Returns a fully populated [`Notification`] instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - JSON deserialization of source data fails
    /// - Timestamp parsing fails
    /// - UUID parsing fails
    /// - Unknown enum values are encountered
    fn row_to_notification(&self, row: &sqlx::sqlite::SqliteRow) -> Result<Notification> {
        let id: String = row.get("id");
        let source_data: String = row.get("source_data");
        let lifetime_type: String = row.get("lifetime_type");
        let lifetime_expires_at: Option<String> = row.get("lifetime_expires_at");
        let priority_str: String = row.get("priority");
        let status_str: String = row.get("status");
        let requires_response: i64 = row.get("requires_response");
        let response: Option<String> = row.get("response");
        let created_at: String = row.get("created_at");
        let updated_at: String = row.get("updated_at");

        let source: NotificationSource = serde_json::from_str(&source_data)?;

        let lifetime = match lifetime_type.as_str() {
            "Ephemeral" => {
                let expires_at = lifetime_expires_at.ok_or_else(|| {
                    anyhow::anyhow!("Missing expires_at for ephemeral notification")
                })?;
                NotificationLifetime::Ephemeral {
                    expires_at: DateTime::parse_from_rfc3339(&expires_at)?.with_timezone(&Utc),
                }
            }
            "Persistent" => NotificationLifetime::Persistent,
            _ => anyhow::bail!("Unknown lifetime type: {lifetime_type}"),
        };

        let priority = match priority_str.as_str() {
            "Low" => NotificationPriority::Low,
            "Normal" => NotificationPriority::Normal,
            "High" => NotificationPriority::High,
            "Urgent" => NotificationPriority::Urgent,
            _ => NotificationPriority::Normal,
        };

        let status = match status_str.as_str() {
            "Pending" => NotificationStatus::Pending,
            "Viewed" => NotificationStatus::Viewed,
            "Responded" => NotificationStatus::Responded,
            "Dismissed" => NotificationStatus::Dismissed,
            "Expired" => NotificationStatus::Expired,
            _ => NotificationStatus::Pending,
        };

        Ok(Notification {
            id: Uuid::parse_str(&id)?,
            source,
            lifetime,
            priority,
            status,
            title: row.get("title"),
            message: row.get("message"),
            requires_response: requires_response != 0,
            response,
            created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_storage() -> (NotificationStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = NotificationStorage::with_path(&db_path).await.unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_storage_add_get() {
        let (storage, _temp) = create_test_storage().await;
        let notification = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "Test".to_string(),
            "Message".to_string(),
            false,
        );

        let id = notification.id;
        storage.add(&notification).await.unwrap();

        let retrieved = storage.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_storage_update() {
        let (storage, _temp) = create_test_storage().await;
        let mut notification = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "Test".to_string(),
            "Message".to_string(),
            false,
        );

        let id = notification.id;
        storage.add(&notification).await.unwrap();

        notification.dismiss();
        storage.update(&notification).await.unwrap();

        let retrieved = storage.get(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, NotificationStatus::Dismissed);
    }

    #[tokio::test]
    async fn test_storage_delete() {
        let (storage, _temp) = create_test_storage().await;
        let notification = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "Test".to_string(),
            "Message".to_string(),
            false,
        );

        let id = notification.id;
        storage.add(&notification).await.unwrap();
        storage.delete(&id).await.unwrap();

        let retrieved = storage.get(&id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_storage_list_actionable() {
        let (storage, _temp) = create_test_storage().await;

        // Add actionable notification
        let n1 = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::High,
            "High priority".to_string(),
            "Message".to_string(),
            false,
        );

        // Add dismissed notification (not actionable)
        let mut n2 = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "Dismissed".to_string(),
            "Message".to_string(),
            false,
        );
        n2.dismiss();

        storage.add(&n1).await.unwrap();
        storage.add(&n2).await.unwrap();

        let actionable = storage.list_actionable().await.unwrap();
        assert_eq!(actionable.len(), 1);
        assert_eq!(actionable[0].title, "High priority");
    }
}
