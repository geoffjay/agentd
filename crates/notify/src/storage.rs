//! SeaORM-based persistent storage for notifications.
//!
//! Provides the [`NotificationStorage`] backend that persists notifications to
//! an SQLite database using SeaORM entities and a migration-managed schema.
//!
//! # Database Location
//!
//! - Linux: `~/.local/share/agentd-notify/notify.db`
//! - macOS: `~/Library/Application Support/agentd-notify/notify.db`
//!
//! # Schema
//!
//! Managed by [`crate::migration::Migrator`].  See
//! `migration/m20250305_000001_create_notifications_table.rs` for the full
//! column list.
//!
//! # Examples
//!
//! ```no_run
//! use notify::storage::NotificationStorage;
//! use notify::types::*;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let storage = NotificationStorage::new().await?;
//!
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
//!     Ok(())
//! }
//! ```

use crate::{
    entity::notification as notif_entity,
    migration::Migrator,
    types::{
        Notification, NotificationLifetime, NotificationPriority, NotificationSource,
        NotificationStatus,
    },
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult, Order,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
};
use sea_orm_migration::prelude::MigratorTrait;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Persistent storage backend for notifications using SeaORM + SQLite.
///
/// This struct provides a thread-safe, async interface to a SQLite database.
/// [`DatabaseConnection`] is `Clone + Send + Sync`.
///
/// # Examples
///
/// ```no_run
/// use notify::storage::NotificationStorage;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let storage = NotificationStorage::new().await?;
///     let storage_clone = storage.clone();
///     tokio::spawn(async move { let _ = storage_clone; });
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct NotificationStorage {
    db: DatabaseConnection,
}

impl NotificationStorage {
    /// Gets the platform-specific database file path.
    ///
    /// - **Linux**: `~/.local/share/agentd-notify/notify.db`
    /// - **macOS**: `~/Library/Application Support/agentd-notify/notify.db`
    pub fn get_db_path() -> Result<PathBuf> {
        agentd_common::storage::get_db_path("agentd-notify", "notify.db")
    }

    /// Creates a new storage instance with the default database path.
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    /// Creates a new storage instance connected to `db_path`.
    ///
    /// The file is created if it does not exist, and all pending SeaORM
    /// migrations are applied before returning.
    pub async fn with_path(db_path: &Path) -> Result<Self> {
        let db = agentd_common::storage::create_connection(db_path).await?;
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }

    /// Inserts a notification and returns its UUID.
    pub async fn add(&self, notification: &Notification) -> Result<Uuid> {
        let model = notif_entity::ActiveModel {
            id: Set(notification.id.to_string()),
            source_type: Set(format!("{:?}", notification.source)),
            source_data: Set(serde_json::to_string(&notification.source)?),
            lifetime_type: Set(match notification.lifetime {
                NotificationLifetime::Ephemeral { .. } => "Ephemeral".to_string(),
                NotificationLifetime::Persistent => "Persistent".to_string(),
            }),
            lifetime_expires_at: Set(match notification.lifetime {
                NotificationLifetime::Ephemeral { expires_at } => Some(expires_at.to_rfc3339()),
                NotificationLifetime::Persistent => None,
            }),
            priority: Set(format!("{:?}", notification.priority)),
            status: Set(format!("{:?}", notification.status)),
            title: Set(notification.title.clone()),
            message: Set(notification.message.clone()),
            requires_response: Set(if notification.requires_response { 1 } else { 0 }),
            response: Set(notification.response.clone()),
            created_at: Set(notification.created_at.to_rfc3339()),
            updated_at: Set(notification.updated_at.to_rfc3339()),
        };

        notif_entity::Entity::insert(model).exec(&self.db).await?;
        Ok(notification.id)
    }

    /// Retrieves a notification by its UUID.
    pub async fn get(&self, id: &Uuid) -> Result<Option<Notification>> {
        let model = notif_entity::Entity::find_by_id(id.to_string()).one(&self.db).await?;
        match model {
            Some(m) => Ok(Some(model_to_notification(m)?)),
            None => Ok(None),
        }
    }

    /// Updates the mutable fields of a notification (status, response, updated_at).
    pub async fn update(&self, notification: &Notification) -> Result<()> {
        use sea_orm::sea_query::Expr;
        let result = notif_entity::Entity::update_many()
            .col_expr(
                notif_entity::Column::Status,
                Expr::value(format!("{:?}", notification.status)),
            )
            .col_expr(notif_entity::Column::Response, Expr::value(notification.response.clone()))
            .col_expr(
                notif_entity::Column::UpdatedAt,
                Expr::value(notification.updated_at.to_rfc3339()),
            )
            .filter(notif_entity::Column::Id.eq(notification.id.to_string()))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            anyhow::bail!("Notification not found");
        }
        Ok(())
    }

    /// Permanently deletes a notification by UUID.
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        let result = notif_entity::Entity::delete_many()
            .filter(notif_entity::Column::Id.eq(id.to_string()))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            anyhow::bail!("Notification not found");
        }
        Ok(())
    }

    /// Lists all notifications, optionally filtered by status (newest first).
    #[allow(dead_code)]
    pub async fn list(
        &self,
        status_filter: Option<NotificationStatus>,
    ) -> Result<Vec<Notification>> {
        let mut query =
            notif_entity::Entity::find().order_by(notif_entity::Column::CreatedAt, Order::Desc);

        if let Some(status) = status_filter {
            query = query.filter(notif_entity::Column::Status.eq(format!("{:?}", status)));
        }

        let models: Vec<notif_entity::Model> = query.all(&self.db).await?;
        models.into_iter().map(model_to_notification).collect()
    }

    /// Lists notifications with pagination; returns `(items, total_count)`.
    pub async fn list_paginated(
        &self,
        status_filter: Option<NotificationStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Notification>, usize)> {
        let condition = match &status_filter {
            Some(s) => Condition::all().add(notif_entity::Column::Status.eq(format!("{:?}", s))),
            None => Condition::all(),
        };

        let total =
            notif_entity::Entity::find().filter(condition.clone()).count(&self.db).await? as usize;

        let models = notif_entity::Entity::find()
            .filter(condition)
            .order_by(notif_entity::Column::CreatedAt, Order::Desc)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await?;

        let notifications =
            models.into_iter().map(model_to_notification).collect::<Result<Vec<_>>>()?;
        Ok((notifications, total))
    }

    /// Lists actionable notifications with pagination.
    ///
    /// Actionable = Pending or Viewed and not expired.
    /// Ordered by priority (highest first), then creation time (newest first).
    pub async fn list_actionable_paginated(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Notification>, usize)> {
        let models: Vec<notif_entity::Model> = notif_entity::Entity::find()
            .filter(
                Condition::any()
                    .add(notif_entity::Column::Status.eq("Pending"))
                    .add(notif_entity::Column::Status.eq("Viewed")),
            )
            .order_by(notif_entity::Column::CreatedAt, Order::Desc)
            .all(&self.db)
            .await?;

        let mut notifications =
            models.into_iter().map(model_to_notification).collect::<Result<Vec<_>>>()?;

        // In-memory filter: drop ephemeral notifications whose expiry has passed
        notifications.retain(|n: &Notification| n.is_actionable());

        // In-memory sort: highest priority first, then newest creation time
        notifications.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then_with(|| b.created_at.cmp(&a.created_at))
        });

        let total = notifications.len();
        let items = notifications.into_iter().skip(offset).take(limit).collect();
        Ok((items, total))
    }

    /// Lists notification history with pagination.
    ///
    /// History = Dismissed, Responded, or Expired, ordered by update time (newest first).
    pub async fn list_history_paginated(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Notification>, usize)> {
        let condition = Condition::any()
            .add(notif_entity::Column::Status.eq("Dismissed"))
            .add(notif_entity::Column::Status.eq("Responded"))
            .add(notif_entity::Column::Status.eq("Expired"));

        let total =
            notif_entity::Entity::find().filter(condition.clone()).count(&self.db).await? as usize;

        let models = notif_entity::Entity::find()
            .filter(condition)
            .order_by(notif_entity::Column::UpdatedAt, Order::Desc)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await?;

        let notifications =
            models.into_iter().map(model_to_notification).collect::<Result<Vec<_>>>()?;
        Ok((notifications, total))
    }

    /// Lists all actionable notifications.
    #[allow(dead_code)]
    pub async fn list_actionable(&self) -> Result<Vec<Notification>> {
        let models: Vec<notif_entity::Model> = notif_entity::Entity::find()
            .filter(
                Condition::any()
                    .add(notif_entity::Column::Status.eq("Pending"))
                    .add(notif_entity::Column::Status.eq("Viewed")),
            )
            .order_by(notif_entity::Column::CreatedAt, Order::Desc)
            .all(&self.db)
            .await?;

        let mut notifications =
            models.into_iter().map(model_to_notification).collect::<Result<Vec<_>>>()?;
        notifications.retain(|n: &Notification| n.is_actionable());
        notifications.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then_with(|| b.created_at.cmp(&a.created_at))
        });
        Ok(notifications)
    }

    /// Lists all historical notifications (Dismissed, Responded, Expired).
    #[allow(dead_code)]
    pub async fn list_history(&self) -> Result<Vec<Notification>> {
        let models = notif_entity::Entity::find()
            .filter(
                Condition::any()
                    .add(notif_entity::Column::Status.eq("Dismissed"))
                    .add(notif_entity::Column::Status.eq("Responded"))
                    .add(notif_entity::Column::Status.eq("Expired")),
            )
            .order_by(notif_entity::Column::UpdatedAt, Order::Desc)
            .all(&self.db)
            .await?;

        models.into_iter().map(model_to_notification).collect()
    }

    /// Marks expired ephemeral notifications as `Expired`, then deletes them.
    ///
    /// Returns the number of rows deleted.
    pub async fn cleanup_expired(&self) -> Result<usize> {
        use sea_orm::sea_query::Expr;

        let now = Utc::now().to_rfc3339();

        // Step 1: mark still-Pending ephemeral rows as Expired
        notif_entity::Entity::update_many()
            .col_expr(notif_entity::Column::Status, Expr::value("Expired"))
            .col_expr(notif_entity::Column::UpdatedAt, Expr::value(now.clone()))
            .filter(
                Condition::all()
                    .add(notif_entity::Column::LifetimeType.eq("Ephemeral"))
                    .add(notif_entity::Column::LifetimeExpiresAt.lt(now))
                    .add(notif_entity::Column::Status.eq("Pending")),
            )
            .exec(&self.db)
            .await?;

        // Step 2: delete all Expired rows
        let result = notif_entity::Entity::delete_many()
            .filter(notif_entity::Column::Status.eq("Expired"))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected as usize)
    }

    /// Returns per-status counts: `Vec<(status, count)>`.
    #[allow(dead_code)]
    pub async fn count(&self) -> Result<Vec<(NotificationStatus, usize)>> {
        #[derive(Debug, FromQueryResult)]
        struct StatusCountRow {
            status: String,
            count: i64,
        }

        let rows = StatusCountRow::find_by_statement(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "SELECT status, COUNT(*) as count FROM notifications GROUP BY status".to_owned(),
        ))
        .all(&self.db)
        .await?;

        let mut counts = Vec::new();
        for row in rows {
            let status = match row.status.as_str() {
                "Pending" => NotificationStatus::Pending,
                "Viewed" => NotificationStatus::Viewed,
                "Responded" => NotificationStatus::Responded,
                "Dismissed" => NotificationStatus::Dismissed,
                "Expired" => NotificationStatus::Expired,
                _ => continue,
            };
            counts.push((status, row.count as usize));
        }

        Ok(counts)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a raw entity `Model` into the domain [`Notification`] type.
fn model_to_notification(model: notif_entity::Model) -> Result<Notification> {
    let source: NotificationSource = serde_json::from_str(&model.source_data)?;

    let lifetime = match model.lifetime_type.as_str() {
        "Ephemeral" => {
            let expires_at_str = model.lifetime_expires_at.ok_or_else(|| {
                anyhow::anyhow!("Missing lifetime_expires_at for Ephemeral notification")
            })?;
            NotificationLifetime::Ephemeral {
                expires_at: DateTime::parse_from_rfc3339(&expires_at_str)?.with_timezone(&Utc),
            }
        }
        "Persistent" => NotificationLifetime::Persistent,
        other => anyhow::bail!("Unknown lifetime_type: {other}"),
    };

    let priority = match model.priority.as_str() {
        "Low" => NotificationPriority::Low,
        "Normal" => NotificationPriority::Normal,
        "High" => NotificationPriority::High,
        "Urgent" => NotificationPriority::Urgent,
        _ => NotificationPriority::Normal,
    };

    let status = match model.status.as_str() {
        "Pending" => NotificationStatus::Pending,
        "Viewed" => NotificationStatus::Viewed,
        "Responded" => NotificationStatus::Responded,
        "Dismissed" => NotificationStatus::Dismissed,
        "Expired" => NotificationStatus::Expired,
        _ => NotificationStatus::Pending,
    };

    Ok(Notification {
        id: Uuid::parse_str(&model.id)?,
        source,
        lifetime,
        priority,
        status,
        title: model.title,
        message: model.message,
        requires_response: model.requires_response != 0,
        response: model.response,
        created_at: DateTime::parse_from_rfc3339(&model.created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&model.updated_at)?.with_timezone(&Utc),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
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

        let n1 = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::High,
            "High priority".to_string(),
            "Message".to_string(),
            false,
        );
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

    #[tokio::test]
    async fn test_priority_ordering() {
        let (storage, _temp) = create_test_storage().await;

        let priorities = [
            (NotificationPriority::Low, "Low task"),
            (NotificationPriority::High, "High task"),
            (NotificationPriority::Normal, "Normal task"),
            (NotificationPriority::Urgent, "Urgent task"),
        ];

        for (priority, title) in &priorities {
            let n = Notification::new(
                NotificationSource::System,
                NotificationLifetime::Persistent,
                *priority,
                title.to_string(),
                "test".to_string(),
                true,
            );
            storage.add(&n).await.unwrap();
        }

        let results = storage.list_actionable().await.unwrap();
        assert_eq!(results.len(), 4);

        // Correct order: Urgent > High > Normal > Low
        assert_eq!(results[0].priority, NotificationPriority::Urgent);
        assert_eq!(results[1].priority, NotificationPriority::High);
        assert_eq!(results[2].priority, NotificationPriority::Normal);
        assert_eq!(results[3].priority, NotificationPriority::Low);
    }

    #[tokio::test]
    async fn test_list_paginated() {
        let (storage, _temp) = create_test_storage().await;

        for i in 0..5 {
            let n = Notification::new(
                NotificationSource::System,
                NotificationLifetime::Persistent,
                NotificationPriority::Normal,
                format!("Notification {i}"),
                "body".to_string(),
                false,
            );
            storage.add(&n).await.unwrap();
        }

        let (page1, total) = storage.list_paginated(None, 3, 0).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(page1.len(), 3);

        let (page2, _) = storage.list_paginated(None, 3, 3).await.unwrap();
        assert_eq!(page2.len(), 2);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let (storage, _temp) = create_test_storage().await;

        let expired = Notification::new(
            NotificationSource::System,
            NotificationLifetime::ephemeral(chrono::Duration::milliseconds(-100)),
            NotificationPriority::Low,
            "Expired".to_string(),
            "body".to_string(),
            false,
        );
        let persistent = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Low,
            "Persistent".to_string(),
            "body".to_string(),
            false,
        );

        storage.add(&expired).await.unwrap();
        storage.add(&persistent).await.unwrap();

        let deleted = storage.cleanup_expired().await.unwrap();
        assert_eq!(deleted, 1);

        let (remaining, total) = storage.list_paginated(None, 50, 0).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(remaining[0].title, "Persistent");
    }

    #[tokio::test]
    async fn test_count() {
        let (storage, _temp) = create_test_storage().await;

        let n1 = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "First".to_string(),
            "body".to_string(),
            false,
        );
        let mut n2 = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "Second".to_string(),
            "body".to_string(),
            false,
        );
        n2.dismiss();

        storage.add(&n1).await.unwrap();
        storage.add(&n2).await.unwrap();
        storage.update(&n2).await.unwrap();

        let counts = storage.count().await.unwrap();
        let pending = counts.iter().find(|(s, _)| *s == NotificationStatus::Pending);
        let dismissed = counts.iter().find(|(s, _)| *s == NotificationStatus::Dismissed);
        assert_eq!(pending.map(|(_, c)| *c), Some(1));
        assert_eq!(dismissed.map(|(_, c)| *c), Some(1));
    }
}
