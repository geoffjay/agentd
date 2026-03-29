//! SeaORM-based persistent storage for questions.
//!
//! Provides the [`QuestionStorage`] backend that persists questions to
//! an SQLite database using SeaORM entities and a migration-managed schema.
//!
//! # Database Location
//!
//! - Linux: `~/.local/share/agentd-ask/ask.db`
//! - macOS: `~/Library/Application Support/agentd-ask/ask.db`
//!
//! # Schema
//!
//! Managed by [`crate::migration::Migrator`]. See
//! `migration/m20250328_000001_create_questions_table.rs` for the full
//! column list.
//!
//! # Examples
//!
//! ```no_run
//! use ask::storage::QuestionStorage;
//! use ask::types::{QuestionInfo, CheckType, QuestionStatus};
//! use chrono::Utc;
//! use uuid::Uuid;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let storage = QuestionStorage::new().await?;
//!
//!     let question = QuestionInfo {
//!         question_id: Uuid::new_v4(),
//!         notification_id: Uuid::new_v4(),
//!         check_type: CheckType::TmuxSessions,
//!         asked_at: Utc::now(),
//!         status: QuestionStatus::Pending,
//!         answer: None,
//!     };
//!
//!     storage.add(&question).await?;
//!     println!("Stored question: {}", question.question_id);
//!     Ok(())
//! }
//! ```

use crate::{
    entity::question as question_entity,
    migration::Migrator,
    types::{CheckType, QuestionInfo, QuestionStatus},
};
use anyhow::Result;
use chrono::DateTime;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder,
};
use sea_orm_migration::prelude::MigratorTrait;
use std::path::Path;
use uuid::Uuid;

/// Persistent storage backend for questions using SeaORM + SQLite.
///
/// This struct provides a thread-safe, async interface to a SQLite database.
/// [`DatabaseConnection`] is `Clone + Send + Sync`.
///
/// # Examples
///
/// ```no_run
/// use ask::storage::QuestionStorage;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let storage = QuestionStorage::new().await?;
///     let storage_clone = storage.clone();
///     tokio::spawn(async move { let _ = storage_clone; });
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct QuestionStorage {
    db: DatabaseConnection,
}

impl QuestionStorage {
    /// Gets the platform-specific database file path.
    ///
    /// - **Linux**: `~/.local/share/agentd-ask/ask.db`
    /// - **macOS**: `~/Library/Application Support/agentd-ask/ask.db`
    pub fn get_db_path() -> Result<std::path::PathBuf> {
        agentd_common::storage::get_db_path("agentd-ask", "ask.db")
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

    /// Creates an in-memory storage instance for testing.
    #[cfg(test)]
    pub async fn in_memory() -> Result<Self> {
        use sea_orm::Database;
        let db = Database::connect("sqlite::memory:").await?;
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }

    /// Inserts a question and returns its UUID.
    pub async fn add(&self, question: &QuestionInfo) -> Result<Uuid> {
        let model = question_entity::ActiveModel {
            id: Set(question.question_id.to_string()),
            notification_id: Set(question.notification_id.to_string()),
            check_type: Set(question.check_type.as_str().to_string()),
            asked_at: Set(question.asked_at.to_rfc3339()),
            status: Set(status_to_str(question.status).to_string()),
            answer: Set(question.answer.clone()),
        };

        question_entity::Entity::insert(model).exec(&self.db).await?;
        Ok(question.question_id)
    }

    /// Retrieves a question by its UUID.
    #[allow(dead_code)]
    pub async fn get(&self, question_id: &Uuid) -> Result<Option<QuestionInfo>> {
        let model =
            question_entity::Entity::find_by_id(question_id.to_string()).one(&self.db).await?;
        model.map(model_to_question).transpose()
    }

    /// Retrieves all questions, ordered by asked_at descending.
    #[allow(dead_code)]
    pub async fn list_all(&self) -> Result<Vec<QuestionInfo>> {
        let models = question_entity::Entity::find()
            .order_by(question_entity::Column::AskedAt, Order::Desc)
            .all(&self.db)
            .await?;

        models.into_iter().map(model_to_question).collect()
    }

    /// Retrieves all questions with a given status.
    #[allow(dead_code)]
    pub async fn list_by_status(&self, status: QuestionStatus) -> Result<Vec<QuestionInfo>> {
        let status_str = status_to_str(status);
        let models = question_entity::Entity::find()
            .filter(question_entity::Column::Status.eq(status_str))
            .order_by(question_entity::Column::AskedAt, Order::Desc)
            .all(&self.db)
            .await?;

        models.into_iter().map(model_to_question).collect()
    }

    /// Updates a question's status and optional answer.
    pub async fn update_status(
        &self,
        question_id: &Uuid,
        status: QuestionStatus,
        answer: Option<String>,
    ) -> Result<()> {
        use sea_orm::ActiveModelTrait;

        let model = question_entity::Entity::find_by_id(question_id.to_string())
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Question {} not found", question_id))?;

        let mut active: question_entity::ActiveModel = model.into();
        active.status = Set(status_to_str(status).to_string());
        active.answer = Set(answer);
        active.save(&self.db).await?;

        Ok(())
    }

    /// Deletes questions that are older than 24 hours and not pending.
    pub async fn cleanup_old(&self) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
        let cutoff_str = cutoff.to_rfc3339();

        let result = question_entity::Entity::delete_many()
            .filter(question_entity::Column::AskedAt.lt(cutoff_str))
            .filter(question_entity::Column::Status.ne(status_to_str(QuestionStatus::Pending)))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }
}

/// Converts a [`QuestionStatus`] to its string representation for storage.
fn status_to_str(status: QuestionStatus) -> &'static str {
    match status {
        QuestionStatus::Pending => "Pending",
        QuestionStatus::Answered => "Answered",
        QuestionStatus::Expired => "Expired",
    }
}

/// Parses a status string from storage back to [`QuestionStatus`].
#[allow(dead_code)]
fn str_to_status(s: &str) -> Result<QuestionStatus> {
    match s {
        "Pending" => Ok(QuestionStatus::Pending),
        "Answered" => Ok(QuestionStatus::Answered),
        "Expired" => Ok(QuestionStatus::Expired),
        other => anyhow::bail!("Unknown question status: {}", other),
    }
}

/// Parses a check_type string from storage back to [`CheckType`].
#[allow(dead_code)]
fn str_to_check_type(s: &str) -> Result<CheckType> {
    match s {
        "tmux_sessions" => Ok(CheckType::TmuxSessions),
        "service_health" => Ok(CheckType::ServiceHealth),
        other => anyhow::bail!("Unknown check type: {}", other),
    }
}

/// Converts a database model row to a [`QuestionInfo`].
#[allow(dead_code)]
fn model_to_question(model: question_entity::Model) -> Result<QuestionInfo> {
    Ok(QuestionInfo {
        question_id: Uuid::parse_str(&model.id)?,
        notification_id: Uuid::parse_str(&model.notification_id)?,
        check_type: str_to_check_type(&model.check_type)?,
        asked_at: DateTime::parse_from_rfc3339(&model.asked_at)?.with_timezone(&chrono::Utc),
        status: str_to_status(&model.status)?,
        answer: model.answer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    async fn make_storage() -> QuestionStorage {
        QuestionStorage::in_memory().await.unwrap()
    }

    fn make_question() -> QuestionInfo {
        QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        }
    }

    #[tokio::test]
    async fn test_add_and_get() {
        let storage = make_storage().await;
        let q = make_question();
        storage.add(&q).await.unwrap();
        let retrieved = storage.get(&q.question_id).await.unwrap().unwrap();
        assert_eq!(retrieved.question_id, q.question_id);
        assert_eq!(retrieved.check_type, q.check_type);
        assert_eq!(retrieved.status, q.status);
        assert!(retrieved.answer.is_none());
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let storage = make_storage().await;
        let result = storage.get(&Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_all() {
        let storage = make_storage().await;
        let q1 = make_question();
        let q2 = make_question();
        storage.add(&q1).await.unwrap();
        storage.add(&q2).await.unwrap();
        let all = storage.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let storage = make_storage().await;
        let q1 = make_question();
        let mut q2 = make_question();
        q2.status = QuestionStatus::Answered;
        q2.answer = Some("yes".to_string());
        storage.add(&q1).await.unwrap();
        storage.add(&q2).await.unwrap();

        let pending = storage.list_by_status(QuestionStatus::Pending).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].question_id, q1.question_id);

        let answered = storage.list_by_status(QuestionStatus::Answered).await.unwrap();
        assert_eq!(answered.len(), 1);
        assert_eq!(answered[0].question_id, q2.question_id);
    }

    #[tokio::test]
    async fn test_update_status() {
        let storage = make_storage().await;
        let q = make_question();
        storage.add(&q).await.unwrap();

        storage
            .update_status(&q.question_id, QuestionStatus::Answered, Some("yes".to_string()))
            .await
            .unwrap();

        let updated = storage.get(&q.question_id).await.unwrap().unwrap();
        assert_eq!(updated.status, QuestionStatus::Answered);
        assert_eq!(updated.answer, Some("yes".to_string()));
    }

    #[tokio::test]
    async fn test_cleanup_old() {
        let storage = make_storage().await;

        // Add an old answered question
        let mut old_answered = make_question();
        old_answered.asked_at = Utc::now() - chrono::Duration::hours(25);
        old_answered.status = QuestionStatus::Answered;
        old_answered.answer = Some("yes".to_string());
        storage.add(&old_answered).await.unwrap();

        // Add a recent pending question (should be kept)
        let recent_pending = make_question();
        storage.add(&recent_pending).await.unwrap();

        // Add an old pending question (should be kept — still actionable)
        let mut old_pending = make_question();
        old_pending.asked_at = Utc::now() - chrono::Duration::hours(25);
        storage.add(&old_pending).await.unwrap();

        let removed = storage.cleanup_old().await.unwrap();
        assert_eq!(removed, 1);

        let all = storage.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(storage.get(&old_answered.question_id).await.unwrap().is_none());
    }
}
