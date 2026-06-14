//! SeaORM-based persistent storage for questions (agent-driven Q&A).
//!
//! Provides the [`QuestionStorage`] backend that persists questions to
//! an SQLite database using SeaORM entities and a migration-managed schema.
//!
//! # Database Location
//!
//! - Linux: `~/.local/share/agentd-ask/ask.db`
//! - macOS: `~/Library/Application Support/agentd-ask/ask.db`

use crate::{
    entity::question as question_entity,
    migration::Migrator,
    types::{CreateQuestionRequest, Question, QuestionStatus},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait, Order, QueryFilter,
    QueryOrder, QuerySelect,
};
use sea_orm_migration::prelude::MigratorTrait;
use std::path::Path;
use uuid::Uuid;

/// Persistent storage backend for questions using SeaORM + SQLite.
#[derive(Clone)]
pub struct QuestionStorage {
    db: DatabaseConnection,
}

impl QuestionStorage {
    /// Gets the platform-specific database file path.
    pub fn get_db_path() -> Result<std::path::PathBuf> {
        agentd_common::storage::get_db_path("agentd-ask", "ask.db")
    }

    /// Creates a new storage instance with the default database path.
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    /// Creates a new storage instance connected to `db_path`.
    pub async fn with_path(db_path: &Path) -> Result<Self> {
        let db = agentd_common::storage::create_connection(db_path).await?;
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }

    /// Creates an in-memory storage instance (for testing).
    ///
    /// Connects to `sqlite::memory:` — data is lost when the connection closes.
    /// Suitable for unit tests, integration tests, and one-shot tooling.
    #[allow(dead_code)]
    pub async fn in_memory() -> Result<Self> {
        use sea_orm::Database;
        let db = Database::connect("sqlite::memory:").await?;
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }

    /// Inserts a new question and returns its UUID.
    ///
    /// Delegates to [`Self::create_with_org`] with `org_id = None`.
    /// Kept for backward compatibility (used by tests and internal callers).
    #[allow(dead_code)]
    pub async fn create(&self, req: &CreateQuestionRequest) -> Result<Question> {
        self.create_with_org(req, None).await
    }

    /// Like [`Self::create`] but also records the `organization_id` at insert
    /// time so the question is immediately visible to tenant-scoped list queries.
    pub async fn create_with_org(
        &self,
        req: &CreateQuestionRequest,
        org_id: Option<&str>,
    ) -> Result<Question> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let priority = req.priority.unwrap_or_default();
        let expires_at =
            req.expires_in_seconds.map(|secs| now + chrono::Duration::seconds(secs as i64));

        let model = question_entity::ActiveModel {
            id: Set(id.to_string()),
            agent_id: Set(req.agent_id.clone()),
            workflow_id: Set(req.workflow_id.map(|u| u.to_string())),
            dispatch_id: Set(req.dispatch_id.map(|u| u.to_string())),
            category: Set(req.category.clone()),
            question: Set(req.question.clone()),
            context: Set(req.context.clone()),
            priority: Set(priority.as_str().to_string()),
            status: Set(QuestionStatus::Pending.as_str().to_string()),
            answer: Set(None),
            asked_at: Set(now.to_rfc3339()),
            answered_at: Set(None),
            expires_at: Set(expires_at.map(|t| t.to_rfc3339())),
            organization_id: Set(org_id.map(|s| s.to_string())),
        };

        question_entity::Entity::insert(model).exec(&self.db).await?;

        Ok(Question {
            id,
            agent_id: req.agent_id.clone(),
            workflow_id: req.workflow_id,
            dispatch_id: req.dispatch_id,
            category: req.category.clone(),
            question: req.question.clone(),
            context: req.context.clone(),
            priority,
            status: QuestionStatus::Pending,
            answer: None,
            asked_at: now,
            answered_at: None,
            expires_at,
        })
    }

    /// Retrieves a question by its UUID.
    pub async fn get(&self, question_id: &Uuid) -> Result<Option<Question>> {
        let model =
            question_entity::Entity::find_by_id(question_id.to_string()).one(&self.db).await?;
        model.map(model_to_question).transpose()
    }

    /// Lists questions with optional filters.
    #[allow(dead_code)]
    pub async fn list(
        &self,
        status: Option<QuestionStatus>,
        agent_id: Option<&str>,
        category: Option<&str>,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<Vec<Question>> {
        self.list_org(status, agent_id, category, None, limit, offset).await
    }

    /// Like [`list`] but also filters by `org_id` when provided.
    pub async fn list_org(
        &self,
        status: Option<QuestionStatus>,
        agent_id: Option<&str>,
        category: Option<&str>,
        org_id: Option<&str>,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<Vec<Question>> {
        let mut query =
            question_entity::Entity::find().order_by(question_entity::Column::AskedAt, Order::Desc);

        if let Some(s) = status {
            query = query.filter(question_entity::Column::Status.eq(s.as_str()));
        }
        if let Some(a) = agent_id {
            query = query.filter(question_entity::Column::AgentId.eq(a));
        }
        if let Some(c) = category {
            query = query.filter(question_entity::Column::Category.eq(c));
        }
        if let Some(oid) = org_id {
            // Include legacy NULL rows so pre-migration data is still visible
            // to authenticated tenants until backfill-tenant is run.
            query = query.filter(
                Condition::any()
                    .add(question_entity::Column::OrganizationId.eq(oid))
                    .add(question_entity::Column::OrganizationId.is_null()),
            );
        }
        if let Some(lim) = limit {
            query = query.limit(lim);
        }
        if let Some(off) = offset {
            query = query.offset(off);
        }

        let models = query.all(&self.db).await?;
        models.into_iter().map(model_to_question).collect()
    }

    /// Updates a question's status, answer, and answered_at timestamp.
    pub async fn update_status(
        &self,
        question_id: &Uuid,
        status: QuestionStatus,
        answer: Option<String>,
    ) -> Result<Question> {
        use sea_orm::ActiveModelTrait;

        let model = question_entity::Entity::find_by_id(question_id.to_string())
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Question {} not found", question_id))?;

        // Validate transition: only Pending questions can be answered/dismissed.
        let current_status: QuestionStatus = model.status.parse()?;
        if current_status != QuestionStatus::Pending {
            anyhow::bail!(
                "Question {} is already {} and cannot be updated",
                question_id,
                current_status
            );
        }

        let now = Utc::now();
        let mut active: question_entity::ActiveModel = model.into();
        active.status = Set(status.as_str().to_string());
        active.answer = Set(answer.clone());
        active.answered_at = Set(Some(now.to_rfc3339()));
        active.save(&self.db).await?;

        self.get(question_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Question {} not found after update", question_id))
    }

    /// Expires questions whose `expires_at` timestamp has passed.
    pub async fn expire_old(&self) -> Result<u64> {
        use sea_orm::{ActiveModelTrait, IntoActiveModel};

        let now = Utc::now().to_rfc3339();

        let expired = question_entity::Entity::find()
            .filter(question_entity::Column::Status.eq(QuestionStatus::Pending.as_str()))
            .filter(question_entity::Column::ExpiresAt.is_not_null())
            .filter(question_entity::Column::ExpiresAt.lt(now))
            .all(&self.db)
            .await?;

        let count = expired.len() as u64;
        for model in expired {
            let mut active = model.into_active_model();
            active.status = Set(QuestionStatus::Expired.as_str().to_string());
            active.save(&self.db).await?;
        }

        Ok(count)
    }
}

/// Converts a database model row to a [`Question`].
fn model_to_question(model: question_entity::Model) -> Result<Question> {
    Ok(Question {
        id: Uuid::parse_str(&model.id)?,
        agent_id: model.agent_id,
        workflow_id: model.workflow_id.as_deref().map(Uuid::parse_str).transpose()?,
        dispatch_id: model.dispatch_id.as_deref().map(Uuid::parse_str).transpose()?,
        category: model.category,
        question: model.question,
        context: model.context,
        priority: model.priority.parse()?,
        status: model.status.parse()?,
        answer: model.answer,
        asked_at: DateTime::parse_from_rfc3339(&model.asked_at)?.with_timezone(&Utc),
        answered_at: model
            .answered_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()?
            .map(|t| t.with_timezone(&Utc)),
        expires_at: model
            .expires_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()?
            .map(|t| t.with_timezone(&Utc)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::QuestionPriority;

    async fn make_storage() -> QuestionStorage {
        QuestionStorage::in_memory().await.unwrap()
    }

    fn make_request() -> CreateQuestionRequest {
        CreateQuestionRequest {
            agent_id: "test-agent".to_string(),
            workflow_id: None,
            dispatch_id: None,
            category: Some("health".to_string()),
            question: "What did you eat yesterday?".to_string(),
            context: None,
            priority: Some(QuestionPriority::Normal),
            expires_in_seconds: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let storage = make_storage().await;
        let req = make_request();
        let q = storage.create(&req).await.unwrap();

        assert_eq!(q.agent_id, "test-agent");
        assert_eq!(q.status, QuestionStatus::Pending);
        assert_eq!(q.priority, QuestionPriority::Normal);
        assert!(q.answer.is_none());

        let retrieved = storage.get(&q.id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, q.id);
        assert_eq!(retrieved.question, q.question);
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
        let mut req1 = make_request();
        req1.agent_id = "agent-1".to_string();
        let mut req2 = make_request();
        req2.agent_id = "agent-2".to_string();

        storage.create(&req1).await.unwrap();
        storage.create(&req2).await.unwrap();

        let all = storage.list(None, None, None, None, None).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let storage = make_storage().await;
        let req = make_request();
        let q = storage.create(&req).await.unwrap();

        storage
            .update_status(&q.id, QuestionStatus::Answered, Some("oatmeal".to_string()))
            .await
            .unwrap();

        let pending =
            storage.list(Some(QuestionStatus::Pending), None, None, None, None).await.unwrap();
        assert_eq!(pending.len(), 0);

        let answered =
            storage.list(Some(QuestionStatus::Answered), None, None, None, None).await.unwrap();
        assert_eq!(answered.len(), 1);
    }

    #[tokio::test]
    async fn test_list_by_agent_id() {
        let storage = make_storage().await;
        let mut req1 = make_request();
        req1.agent_id = "dietician".to_string();
        let mut req2 = make_request();
        req2.agent_id = "assistant".to_string();

        storage.create(&req1).await.unwrap();
        storage.create(&req2).await.unwrap();

        let results = storage.list(None, Some("dietician"), None, None, None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].agent_id, "dietician");
    }

    #[tokio::test]
    async fn test_update_status_answer() {
        let storage = make_storage().await;
        let req = make_request();
        let q = storage.create(&req).await.unwrap();

        let updated = storage
            .update_status(&q.id, QuestionStatus::Answered, Some("oatmeal".to_string()))
            .await
            .unwrap();

        assert_eq!(updated.status, QuestionStatus::Answered);
        assert_eq!(updated.answer, Some("oatmeal".to_string()));
        assert!(updated.answered_at.is_some());
    }

    #[tokio::test]
    async fn test_update_status_dismiss() {
        let storage = make_storage().await;
        let req = make_request();
        let q = storage.create(&req).await.unwrap();

        let updated = storage.update_status(&q.id, QuestionStatus::Dismissed, None).await.unwrap();

        assert_eq!(updated.status, QuestionStatus::Dismissed);
        assert!(updated.answer.is_none());
    }

    #[tokio::test]
    async fn test_update_already_answered_fails() {
        let storage = make_storage().await;
        let req = make_request();
        let q = storage.create(&req).await.unwrap();

        storage
            .update_status(&q.id, QuestionStatus::Answered, Some("yes".to_string()))
            .await
            .unwrap();

        let result =
            storage.update_status(&q.id, QuestionStatus::Answered, Some("no".to_string())).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_expire_old() {
        let storage = make_storage().await;

        // Create a question that expires in -1 second (already expired).
        let req = CreateQuestionRequest {
            agent_id: "test-agent".to_string(),
            workflow_id: None,
            dispatch_id: None,
            category: None,
            question: "Expired question?".to_string(),
            context: None,
            priority: None,
            expires_in_seconds: Some(0), // expires immediately
        };
        storage.create(&req).await.unwrap();

        // Wait a moment to ensure expiry.
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let expired = storage.expire_old().await.unwrap();
        assert_eq!(expired, 1);

        let pending =
            storage.list(Some(QuestionStatus::Pending), None, None, None, None).await.unwrap();
        assert_eq!(pending.len(), 0);
    }
}
