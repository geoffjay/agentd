use crate::notification::{
    Notification, NotificationLifetime, NotificationPriority, NotificationSource,
    NotificationStatus,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use sqlx::{sqlite::SqlitePool, Row};
use std::path::PathBuf;
use uuid::Uuid;

/// Storage backend for notifications using SQLite
#[derive(Clone)]
pub struct NotificationStorage {
    pool: SqlitePool,
}

impl NotificationStorage {
    /// Get the database path using platform-specific directories
    pub fn get_db_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("", "", "agentd-notify")
            .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;

        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        Ok(data_dir.join("notify.db"))
    }

    /// Create a new notification storage with the default database path
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    /// Create a new notification storage with a custom database path
    pub async fn with_path(db_path: &PathBuf) -> Result<Self> {
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&db_url).await?;

        let storage = Self { pool };
        storage.init_schema().await?;

        Ok(storage)
    }

    /// Initialize the database schema
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

    /// Add a new notification
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
        let response = notification.response.as_ref().map(|r| r.as_str());

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

    /// Get a notification by ID
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

    /// Update an existing notification
    pub async fn update(&self, notification: &Notification) -> Result<()> {
        let status = format!("{:?}", notification.status);
        let response = notification.response.as_ref().map(|r| r.as_str());

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

    /// Delete a notification by ID
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

    /// Get all notifications, optionally filtered by status
    pub async fn list(&self, status_filter: Option<NotificationStatus>) -> Result<Vec<Notification>> {
        let rows = if let Some(status) = status_filter {
            let status_str = format!("{:?}", status);
            sqlx::query("SELECT * FROM notifications WHERE status = ? ORDER BY created_at DESC")
                .bind(status_str)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query("SELECT * FROM notifications ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?
        };

        rows.iter()
            .map(|row| self.row_to_notification(row))
            .collect()
    }

    /// Get all actionable notifications
    pub async fn list_actionable(&self) -> Result<Vec<Notification>> {
        let rows = sqlx::query(
            "SELECT * FROM notifications WHERE status IN ('Pending', 'Viewed') ORDER BY priority DESC, created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut notifications: Vec<Notification> = rows
            .iter()
            .map(|row| self.row_to_notification(row))
            .collect::<Result<Vec<_>>>()?;

        // Filter out expired notifications
        notifications.retain(|n| n.is_actionable());

        Ok(notifications)
    }

    /// List notification history (dismissed, responded, expired)
    pub async fn list_history(&self) -> Result<Vec<Notification>> {
        let rows = sqlx::query(
            "SELECT * FROM notifications WHERE status IN ('Dismissed', 'Responded', 'Expired') ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|row| self.row_to_notification(row))
            .collect()
    }

    /// Clean up expired notifications
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
        let delete_result = sqlx::query(
            "DELETE FROM notifications WHERE status = 'Expired'"
        )
        .execute(&self.pool)
        .await?;

        Ok(delete_result.rows_affected() as usize)
    }

    /// Get count of notifications by status
    pub async fn count(&self) -> Result<Vec<(NotificationStatus, usize)>> {
        let rows = sqlx::query("SELECT status, COUNT(*) as count FROM notifications GROUP BY status")
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

    /// Convert a SQLite row to a Notification
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
                let expires_at = lifetime_expires_at
                    .ok_or_else(|| anyhow::anyhow!("Missing expires_at for ephemeral notification"))?;
                NotificationLifetime::Ephemeral {
                    expires_at: DateTime::parse_from_rfc3339(&expires_at)?.with_timezone(&Utc),
                }
            }
            "Persistent" => NotificationLifetime::Persistent,
            _ => anyhow::bail!("Unknown lifetime type: {}", lifetime_type),
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
