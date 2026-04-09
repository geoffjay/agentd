//! SeaORM-based persistent storage for workflows and dispatch logs.
//!
//! [`SchedulerStorage`] shares the same [`DatabaseConnection`] as
//! [`crate::storage::AgentStorage`] — the database schema (including the
//! `workflows` and `dispatch_log` tables) is managed by the single
//! [`crate::migration::Migrator`] that runs at startup.

use crate::{
    entity::{
        dispatch as dispatch_entity, task_queue as queue_entity, workflow as workflow_entity,
    },
    scheduler::types::{DispatchRecord, DispatchStatus, WorkflowConfig},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, Order,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;

/// Persistent storage for workflows and dispatch logs.
///
/// Shares a [`DatabaseConnection`] with [`crate::storage::AgentStorage`];
/// the caller is responsible for running migrations before constructing this.
#[derive(Clone)]
pub struct SchedulerStorage {
    db: DatabaseConnection,
}

impl SchedulerStorage {
    /// Create a new [`SchedulerStorage`] backed by `db`.
    ///
    /// `db` is expected to already have the full schema applied (via
    /// [`crate::migration::Migrator::up`]).
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    // -- Workflow CRUD --

    /// Inserts a workflow and returns its UUID.
    pub async fn add_workflow(&self, workflow: &WorkflowConfig) -> Result<Uuid> {
        let trigger_config_json = serde_json::to_string(&workflow.trigger_config)?;
        let tool_policy_json = serde_json::to_string(&workflow.tool_policy).unwrap_or_default();

        let model = workflow_entity::ActiveModel {
            id: Set(workflow.id.to_string()),
            name: Set(workflow.name.clone()),
            agent_id: Set(workflow.agent_id.to_string()),
            trigger_type: Set(workflow.trigger_config.trigger_type().to_string()),
            trigger_config: Set(trigger_config_json),
            prompt_template: Set(workflow.prompt_template.clone()),
            poll_interval_secs: Set(workflow.poll_interval_secs as i64),
            enabled: Set(if workflow.enabled { 1 } else { 0 }),
            tool_policy: Set(tool_policy_json),
            created_at: Set(workflow.created_at.to_rfc3339()),
            updated_at: Set(workflow.updated_at.to_rfc3339()),
            project_id: Set(workflow.project_id.map(|id| id.to_string())),
        };

        workflow_entity::Entity::insert(model).exec(&self.db).await?;
        Ok(workflow.id)
    }

    /// Retrieves a workflow by its UUID.
    pub async fn get_workflow(&self, id: &Uuid) -> Result<Option<WorkflowConfig>> {
        let model = workflow_entity::Entity::find_by_id(id.to_string()).one(&self.db).await?;
        match model {
            Some(m) => Ok(Some(model_to_workflow(m)?)),
            None => Ok(None),
        }
    }

    /// Lists all workflows, optionally filtered by project (newest first).
    pub async fn list_workflows(&self, project_id: Option<Uuid>) -> Result<Vec<WorkflowConfig>> {
        let mut query = workflow_entity::Entity::find()
            .order_by(workflow_entity::Column::CreatedAt, Order::Desc);
        if let Some(pid) = project_id {
            query = query.filter(workflow_entity::Column::ProjectId.eq(pid.to_string()));
        }
        let models: Vec<workflow_entity::Model> = query.all(&self.db).await?;
        models.into_iter().map(model_to_workflow).collect()
    }

    /// Updates mutable workflow fields (name, prompt_template, poll_interval_secs, enabled, tool_policy, updated_at).
    pub async fn update_workflow(&self, workflow: &WorkflowConfig) -> Result<()> {
        use sea_orm::sea_query::Expr;
        let tool_policy_json = serde_json::to_string(&workflow.tool_policy).unwrap_or_default();

        let result = workflow_entity::Entity::update_many()
            .col_expr(workflow_entity::Column::Name, Expr::value(workflow.name.clone()))
            .col_expr(
                workflow_entity::Column::PromptTemplate,
                Expr::value(workflow.prompt_template.clone()),
            )
            .col_expr(
                workflow_entity::Column::PollIntervalSecs,
                Expr::value(workflow.poll_interval_secs as i64),
            )
            .col_expr(
                workflow_entity::Column::Enabled,
                Expr::value(if workflow.enabled { 1i32 } else { 0i32 }),
            )
            .col_expr(workflow_entity::Column::ToolPolicy, Expr::value(tool_policy_json))
            .col_expr(
                workflow_entity::Column::UpdatedAt,
                Expr::value(workflow.updated_at.to_rfc3339()),
            )
            .filter(workflow_entity::Column::Id.eq(workflow.id.to_string()))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            anyhow::bail!("Workflow not found");
        }

        Ok(())
    }

    /// Permanently deletes a workflow by UUID.
    pub async fn delete_workflow(&self, id: &Uuid) -> Result<()> {
        let result = workflow_entity::Entity::delete_many()
            .filter(workflow_entity::Column::Id.eq(id.to_string()))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            anyhow::bail!("Workflow not found");
        }

        Ok(())
    }

    // -- Dispatch log --

    /// Inserts a dispatch record.
    pub async fn add_dispatch(&self, record: &DispatchRecord) -> Result<()> {
        let model = dispatch_entity::ActiveModel {
            id: Set(record.id.to_string()),
            workflow_id: Set(record.workflow_id.to_string()),
            source_id: Set(record.source_id.clone()),
            agent_id: Set(record.agent_id.to_string()),
            prompt_sent: Set(record.prompt_sent.clone()),
            status: Set(record.status.to_string()),
            dispatched_at: Set(record.dispatched_at.to_rfc3339()),
            completed_at: Set(record.completed_at.map(|dt| dt.to_rfc3339())),
        };

        dispatch_entity::Entity::insert(model).exec(&self.db).await?;
        Ok(())
    }

    /// Updates the status and optional completion timestamp of a dispatch record.
    pub async fn update_dispatch_status(
        &self,
        id: &Uuid,
        status: DispatchStatus,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        use sea_orm::sea_query::Expr;

        let result = dispatch_entity::Entity::update_many()
            .col_expr(dispatch_entity::Column::Status, Expr::value(status.to_string()))
            .col_expr(
                dispatch_entity::Column::CompletedAt,
                Expr::value(completed_at.map(|dt| dt.to_rfc3339())),
            )
            .filter(dispatch_entity::Column::Id.eq(id.to_string()))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            anyhow::bail!("Dispatch record not found");
        }

        Ok(())
    }

    /// Returns `true` if the given `source_id` has already been dispatched for `workflow_id`.
    pub async fn is_dispatched(&self, workflow_id: &Uuid, source_id: &str) -> Result<bool> {
        let count = dispatch_entity::Entity::find()
            .filter(
                Condition::all()
                    .add(dispatch_entity::Column::WorkflowId.eq(workflow_id.to_string()))
                    .add(dispatch_entity::Column::SourceId.eq(source_id)),
            )
            .count(&self.db)
            .await?;
        Ok(count > 0)
    }

    /// Lists all dispatch records for a workflow, newest first.
    #[allow(dead_code)]
    pub async fn list_dispatches(&self, workflow_id: &Uuid) -> Result<Vec<DispatchRecord>> {
        let models: Vec<dispatch_entity::Model> = dispatch_entity::Entity::find()
            .filter(dispatch_entity::Column::WorkflowId.eq(workflow_id.to_string()))
            .order_by(dispatch_entity::Column::DispatchedAt, Order::Desc)
            .all(&self.db)
            .await?;
        models.into_iter().map(model_to_dispatch).collect()
    }

    /// Lists workflows with pagination, optionally filtered by project; returns `(items, total_count)`.
    pub async fn list_workflows_paginated(
        &self,
        limit: usize,
        offset: usize,
        project_id: Option<Uuid>,
    ) -> Result<(Vec<WorkflowConfig>, usize)> {
        let mut base = workflow_entity::Entity::find();
        if let Some(pid) = project_id {
            base = base.filter(workflow_entity::Column::ProjectId.eq(pid.to_string()));
        }

        let total = base.clone().count(&self.db).await? as usize;

        let models: Vec<workflow_entity::Model> = base
            .order_by(workflow_entity::Column::CreatedAt, Order::Desc)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await?;

        let workflows = models.into_iter().map(model_to_workflow).collect::<Result<Vec<_>>>()?;
        Ok((workflows, total))
    }

    /// Lists dispatch records for a workflow with pagination; returns `(items, total_count)`.
    pub async fn list_dispatches_paginated(
        &self,
        workflow_id: &Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<DispatchRecord>, usize)> {
        let condition =
            Condition::all().add(dispatch_entity::Column::WorkflowId.eq(workflow_id.to_string()));

        let total =
            dispatch_entity::Entity::find().filter(condition.clone()).count(&self.db).await?
                as usize;

        let models: Vec<dispatch_entity::Model> = dispatch_entity::Entity::find()
            .filter(condition)
            .order_by(dispatch_entity::Column::DispatchedAt, Order::Desc)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await?;

        let dispatches = models.into_iter().map(model_to_dispatch).collect::<Result<Vec<_>>>()?;
        Ok((dispatches, total))
    }

    /// Finds the active (`Dispatched`) dispatch record for an agent, if any.
    #[allow(dead_code)]
    pub async fn find_active_dispatch(&self, agent_id: &Uuid) -> Result<Option<DispatchRecord>> {
        let model = dispatch_entity::Entity::find()
            .filter(
                Condition::all()
                    .add(dispatch_entity::Column::AgentId.eq(agent_id.to_string()))
                    .add(dispatch_entity::Column::Status.eq("dispatched")),
            )
            .one(&self.db)
            .await?;

        match model {
            Some(m) => Ok(Some(model_to_dispatch(m)?)),
            None => Ok(None),
        }
    }

    /// Marks all in-flight (`dispatched`) dispatch records as `failed`.
    ///
    /// Used during startup recovery to handle records that were in-flight when
    /// the service was last interrupted.
    ///
    /// Returns the number of rows updated.
    pub async fn fail_inflight_dispatches(&self) -> Result<u64> {
        use sea_orm::sea_query::Expr;
        let now = Utc::now().to_rfc3339();

        let result = dispatch_entity::Entity::update_many()
            .col_expr(dispatch_entity::Column::Status, Expr::value("failed"))
            .col_expr(dispatch_entity::Column::CompletedAt, Expr::value(now))
            .filter(dispatch_entity::Column::Status.eq("dispatched"))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    // -- Queue operations --

    /// Inserts a task into the named queue and returns the task ID.
    pub async fn enqueue(
        &self,
        queue_name: &str,
        title: &str,
        body: Option<&str>,
        priority: i32,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let model = queue_entity::ActiveModel {
            id: Set(id.clone()),
            queue_name: Set(queue_name.to_string()),
            title: Set(title.to_string()),
            body: Set(body.map(str::to_string)),
            priority: Set(priority),
            status: Set(queue_entity::STATUS_PENDING.to_string()),
            visibility_timeout_at: Set(None),
            retry_count: Set(0),
            max_retries: Set(3),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        };

        queue_entity::Entity::insert(model).exec(&self.db).await?;
        Ok(id)
    }

    /// Atomically claims the highest-priority oldest pending task from the queue.
    ///
    /// The task is marked as `processing` with a visibility timeout. Returns
    /// `None` if the queue has no pending tasks.
    pub async fn dequeue(
        &self,
        queue_name: &str,
        visibility_timeout_secs: u64,
    ) -> Result<Option<queue_entity::Model>> {
        let timeout_at = (chrono::Utc::now()
            + chrono::Duration::seconds(visibility_timeout_secs as i64))
        .to_rfc3339();
        let now = chrono::Utc::now().to_rfc3339();
        let now_str = chrono::Utc::now().to_rfc3339();

        // Use a transaction to atomically claim the next task.
        let db = &self.db;
        let result = db
            .transaction::<_, Option<queue_entity::Model>, sea_orm::DbErr>(|txn| {
                let queue_name = queue_name.to_string();
                let timeout_at = timeout_at.clone();
                let now = now.clone();

                Box::pin(async move {
                    // Find the best candidate (highest priority, then oldest).
                    let candidate = queue_entity::Entity::find()
                        .filter(queue_entity::Column::QueueName.eq(&queue_name))
                        .filter(queue_entity::Column::Status.eq(queue_entity::STATUS_PENDING))
                        .order_by(queue_entity::Column::Priority, Order::Desc)
                        .order_by(queue_entity::Column::CreatedAt, Order::Asc)
                        .one(txn)
                        .await?;

                    let Some(row) = candidate else {
                        return Ok(None);
                    };

                    // Claim it.
                    let mut active: queue_entity::ActiveModel = row.into();
                    active.status = Set(queue_entity::STATUS_PROCESSING.to_string());
                    active.visibility_timeout_at = Set(Some(timeout_at));
                    active.updated_at = Set(now);
                    let updated = active.update(txn).await?;
                    Ok(Some(updated))
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!("Dequeue transaction failed: {}", e))?;

        let _ = now_str; // suppress unused warning
        Ok(result)
    }

    /// Marks a queue task as completed and removes it from the queue.
    pub async fn complete_queue_task(&self, id: &str) -> Result<()> {
        use sea_orm::sea_query::Expr;

        let now = chrono::Utc::now().to_rfc3339();
        let result = queue_entity::Entity::update_many()
            .col_expr(queue_entity::Column::Status, Expr::value(queue_entity::STATUS_COMPLETED))
            .col_expr(queue_entity::Column::UpdatedAt, Expr::value(now))
            .filter(queue_entity::Column::Id.eq(id))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            anyhow::bail!("Queue task not found: {}", id);
        }
        Ok(())
    }

    /// Increments retry count; requeues the task or moves it to dead letter.
    pub async fn fail_queue_task(&self, id: &str) -> Result<()> {
        use sea_orm::sea_query::Expr;

        let now = chrono::Utc::now().to_rfc3339();

        // Load the current task to check retry count.
        let task = queue_entity::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Queue task not found: {}", id))?;

        let new_retry = task.retry_count + 1;
        let new_status = if new_retry >= task.max_retries {
            queue_entity::STATUS_DEAD
        } else {
            queue_entity::STATUS_PENDING
        };

        queue_entity::Entity::update_many()
            .col_expr(queue_entity::Column::Status, Expr::value(new_status))
            .col_expr(queue_entity::Column::RetryCount, Expr::value(new_retry))
            .col_expr(
                queue_entity::Column::VisibilityTimeoutAt,
                Expr::value(Option::<String>::None),
            )
            .col_expr(queue_entity::Column::UpdatedAt, Expr::value(now))
            .filter(queue_entity::Column::Id.eq(id))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    /// Returns up to `limit` pending tasks without claiming them.
    pub async fn peek_queue(
        &self,
        queue_name: &str,
        limit: u64,
    ) -> Result<Vec<queue_entity::Model>> {
        let tasks = queue_entity::Entity::find()
            .filter(queue_entity::Column::QueueName.eq(queue_name))
            .filter(queue_entity::Column::Status.eq(queue_entity::STATUS_PENDING))
            .order_by(queue_entity::Column::Priority, Order::Desc)
            .order_by(queue_entity::Column::CreatedAt, Order::Asc)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(tasks)
    }

    /// Deletes all tasks in the named queue regardless of status.
    pub async fn purge_queue(&self, queue_name: &str) -> Result<u64> {
        let result = queue_entity::Entity::delete_many()
            .filter(queue_entity::Column::QueueName.eq(queue_name))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Returns task counts by status for the named queue.
    pub async fn queue_stats(&self, queue_name: &str) -> Result<QueueStats> {
        let all = queue_entity::Entity::find()
            .filter(queue_entity::Column::QueueName.eq(queue_name))
            .all(&self.db)
            .await?;

        let mut stats = QueueStats::default();
        for task in &all {
            match task.status.as_str() {
                queue_entity::STATUS_PENDING => stats.pending += 1,
                queue_entity::STATUS_PROCESSING => stats.processing += 1,
                queue_entity::STATUS_COMPLETED => stats.completed += 1,
                queue_entity::STATUS_FAILED => stats.failed += 1,
                queue_entity::STATUS_DEAD => stats.dead += 1,
                _ => {}
            }
        }
        Ok(stats)
    }
}

/// Counts of queue tasks by status.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct QueueStats {
    pub pending: u64,
    pub processing: u64,
    pub completed: u64,
    pub failed: u64,
    pub dead: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn model_to_workflow(model: workflow_entity::Model) -> Result<WorkflowConfig> {
    use crate::types::ToolPolicy;
    Ok(WorkflowConfig {
        id: Uuid::parse_str(&model.id)?,
        name: model.name,
        agent_id: Uuid::parse_str(&model.agent_id)?,
        trigger_config: serde_json::from_str(&model.trigger_config)?,
        prompt_template: model.prompt_template,
        poll_interval_secs: model.poll_interval_secs as u64,
        enabled: model.enabled != 0,
        tool_policy: serde_json::from_str::<ToolPolicy>(&model.tool_policy).unwrap_or_default(),
        created_at: DateTime::parse_from_rfc3339(&model.created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&model.updated_at)?.with_timezone(&Utc),
        project_id: model.project_id.map(|s| Uuid::parse_str(&s)).transpose()?,
    })
}

fn model_to_dispatch(model: dispatch_entity::Model) -> Result<DispatchRecord> {
    Ok(DispatchRecord {
        id: Uuid::parse_str(&model.id)?,
        workflow_id: Uuid::parse_str(&model.workflow_id)?,
        source_id: model.source_id,
        agent_id: Uuid::parse_str(&model.agent_id)?,
        prompt_sent: model.prompt_sent,
        status: model.status.parse()?,
        dispatched_at: DateTime::parse_from_rfc3339(&model.dispatched_at)?.with_timezone(&Utc),
        completed_at: model
            .completed_at
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::types::TriggerConfig;
    use crate::storage::AgentStorage;
    use tempfile::TempDir;

    async fn create_test_storage() -> (SchedulerStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        // Run migrations via AgentStorage (which applies all three tables)
        let agent_storage = AgentStorage::with_path(&db_path).await.unwrap();
        let storage = SchedulerStorage::new(agent_storage.db().clone());
        (storage, temp_dir)
    }

    fn test_workflow() -> WorkflowConfig {
        let now = Utc::now();
        WorkflowConfig {
            id: Uuid::new_v4(),
            name: "test-workflow".to_string(),
            agent_id: Uuid::new_v4(),
            trigger_config: TriggerConfig::GithubIssues {
                owner: "org".to_string(),
                repo: "repo".to_string(),
                labels: vec!["agent".to_string()],
                state: "open".to_string(),
            },
            prompt_template: "Fix: {{title}}".to_string(),
            poll_interval_secs: 60,
            enabled: true,
            tool_policy: Default::default(),
            created_at: now,
            updated_at: now,
            project_id: None,
        }
    }

    #[tokio::test]
    async fn test_workflow_crud() {
        let (storage, _tmp) = create_test_storage().await;
        let workflow = test_workflow();
        let id = workflow.id;

        // Add
        storage.add_workflow(&workflow).await.unwrap();

        // Get
        let retrieved = storage.get_workflow(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, "test-workflow");
        assert_eq!(retrieved.poll_interval_secs, 60);

        // List
        let all = storage.list_workflows(None).await.unwrap();
        assert_eq!(all.len(), 1);

        // Update
        let mut updated = retrieved;
        updated.enabled = false;
        updated.updated_at = Utc::now();
        storage.update_workflow(&updated).await.unwrap();
        let retrieved = storage.get_workflow(&id).await.unwrap().unwrap();
        assert!(!retrieved.enabled);

        // Delete
        storage.delete_workflow(&id).await.unwrap();
        assert!(storage.get_workflow(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_dispatch_log() {
        let (storage, _tmp) = create_test_storage().await;
        let workflow = test_workflow();
        storage.add_workflow(&workflow).await.unwrap();

        let record = DispatchRecord {
            id: Uuid::new_v4(),
            workflow_id: workflow.id,
            source_id: "42".to_string(),
            agent_id: workflow.agent_id,
            prompt_sent: "Fix: Login bug".to_string(),
            status: DispatchStatus::Dispatched,
            dispatched_at: Utc::now(),
            completed_at: None,
        };

        // Add dispatch
        storage.add_dispatch(&record).await.unwrap();

        // Check dispatched
        assert!(storage.is_dispatched(&workflow.id, "42").await.unwrap());
        assert!(!storage.is_dispatched(&workflow.id, "99").await.unwrap());

        // Find active
        let active = storage.find_active_dispatch(&workflow.agent_id).await.unwrap();
        assert!(active.is_some());

        // Update status
        storage
            .update_dispatch_status(&record.id, DispatchStatus::Completed, Some(Utc::now()))
            .await
            .unwrap();

        // No longer active
        let active = storage.find_active_dispatch(&workflow.agent_id).await.unwrap();
        assert!(active.is_none());

        // List dispatches
        let history = storage.list_dispatches(&workflow.id).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, DispatchStatus::Completed);
    }

    #[tokio::test]
    async fn test_fail_inflight() {
        let (storage, _tmp) = create_test_storage().await;
        let workflow = test_workflow();
        storage.add_workflow(&workflow).await.unwrap();

        let record = DispatchRecord {
            id: Uuid::new_v4(),
            workflow_id: workflow.id,
            source_id: "1".to_string(),
            agent_id: workflow.agent_id,
            prompt_sent: "test".to_string(),
            status: DispatchStatus::Dispatched,
            dispatched_at: Utc::now(),
            completed_at: None,
        };
        storage.add_dispatch(&record).await.unwrap();

        let count = storage.fail_inflight_dispatches().await.unwrap();
        assert_eq!(count, 1);

        let updated = storage.list_dispatches(&workflow.id).await.unwrap();
        assert_eq!(updated[0].status, DispatchStatus::Failed);
    }

    // -----------------------------------------------------------------------
    // Queue storage tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_enqueue_dequeue_fifo() {
        let (storage, _tmp) = create_test_storage().await;

        // Enqueue three tasks at the same priority (FIFO order expected).
        storage.enqueue("q1", "Task A", None, 0).await.unwrap();
        storage.enqueue("q1", "Task B", None, 0).await.unwrap();
        storage.enqueue("q1", "Task C", None, 0).await.unwrap();

        let first = storage.dequeue("q1", 60).await.unwrap().unwrap();
        assert_eq!(first.title, "Task A");

        let second = storage.dequeue("q1", 60).await.unwrap().unwrap();
        assert_eq!(second.title, "Task B");

        let third = storage.dequeue("q1", 60).await.unwrap().unwrap();
        assert_eq!(third.title, "Task C");

        // Queue is now empty.
        assert!(storage.dequeue("q1", 60).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let (storage, _tmp) = create_test_storage().await;

        // Enqueue tasks with different priorities.
        storage.enqueue("prio", "Low", None, 1).await.unwrap();
        storage.enqueue("prio", "High", None, 10).await.unwrap();
        storage.enqueue("prio", "Medium", None, 5).await.unwrap();

        // Higher priority should be dequeued first.
        let first = storage.dequeue("prio", 60).await.unwrap().unwrap();
        assert_eq!(first.title, "High");
        assert_eq!(first.priority, 10);

        let second = storage.dequeue("prio", 60).await.unwrap().unwrap();
        assert_eq!(second.title, "Medium");

        let third = storage.dequeue("prio", 60).await.unwrap().unwrap();
        assert_eq!(third.title, "Low");
    }

    #[tokio::test]
    async fn test_visibility_timeout_prevents_double_processing() {
        let (storage, _tmp) = create_test_storage().await;

        storage.enqueue("vis", "Task X", Some("body"), 0).await.unwrap();

        // Dequeue with a 60-second visibility timeout.
        let task = storage.dequeue("vis", 60).await.unwrap().unwrap();
        assert_eq!(task.title, "Task X");
        assert_eq!(task.status, "processing");

        // A second dequeue should return None (task is still in the processing window).
        let again = storage.dequeue("vis", 60).await.unwrap();
        assert!(again.is_none(), "Expected None — task should be invisible while processing");
    }

    #[tokio::test]
    async fn test_complete_queue_task() {
        let (storage, _tmp) = create_test_storage().await;

        let id = storage.enqueue("done", "Finish me", None, 0).await.unwrap();
        storage.dequeue("done", 60).await.unwrap().unwrap();

        storage.complete_queue_task(&id).await.unwrap();

        let stats = storage.queue_stats("done").await.unwrap();
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.processing, 0);
        assert_eq!(stats.pending, 0);
    }

    #[tokio::test]
    async fn test_fail_task_retry_and_dead_letter() {
        let (storage, _tmp) = create_test_storage().await;

        let id = storage.enqueue("retry", "Retry me", None, 0).await.unwrap();

        // Fail 3 times — default max_retries is 3, so after 3 failures it's dead.
        for _ in 0..3 {
            storage.dequeue("retry", 60).await.unwrap().unwrap();
            storage.fail_queue_task(&id).await.unwrap();
        }

        let stats = storage.queue_stats("retry").await.unwrap();
        assert_eq!(stats.dead, 1, "Task should be dead after exceeding max_retries");
        assert_eq!(stats.pending, 0);
    }

    #[tokio::test]
    async fn test_peek_queue() {
        let (storage, _tmp) = create_test_storage().await;

        storage.enqueue("peek", "First", None, 0).await.unwrap();
        storage.enqueue("peek", "Second", None, 0).await.unwrap();

        let items = storage.peek_queue("peek", 10).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "First");
        assert_eq!(items[1].title, "Second");

        // Peeking should not consume the tasks.
        let items_again = storage.peek_queue("peek", 10).await.unwrap();
        assert_eq!(items_again.len(), 2);
    }

    #[tokio::test]
    async fn test_peek_limit() {
        let (storage, _tmp) = create_test_storage().await;

        for i in 0..5 {
            storage.enqueue("lim", &format!("Task {i}"), None, 0).await.unwrap();
        }

        let items = storage.peek_queue("lim", 3).await.unwrap();
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn test_purge_queue() {
        let (storage, _tmp) = create_test_storage().await;

        storage.enqueue("purge", "A", None, 0).await.unwrap();
        storage.enqueue("purge", "B", None, 0).await.unwrap();
        storage.enqueue("purge", "C", None, 0).await.unwrap();

        let deleted = storage.purge_queue("purge").await.unwrap();
        assert_eq!(deleted, 3);

        let stats = storage.queue_stats("purge").await.unwrap();
        assert_eq!(stats.pending, 0);
    }

    #[tokio::test]
    async fn test_queue_stats_all_statuses() {
        let (storage, _tmp) = create_test_storage().await;

        // Enqueue 2 tasks.
        let id1 = storage.enqueue("stats", "T1", None, 0).await.unwrap();
        let id2 = storage.enqueue("stats", "T2", None, 0).await.unwrap();

        // Dequeue both — now processing.
        storage.dequeue("stats", 300).await.unwrap();
        storage.dequeue("stats", 300).await.unwrap();

        // Complete one.
        storage.complete_queue_task(&id1).await.unwrap();

        // Fail the other until it's dead.
        for _ in 0..3 {
            // Re-dequeue (after each fail, it goes back to pending except on final fail)
            if let Some(row) = storage.dequeue("stats", 300).await.unwrap() {
                let _ = row; // claim it
            }
            storage.fail_queue_task(&id2).await.unwrap();
        }

        let stats = storage.queue_stats("stats").await.unwrap();
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.dead, 1);
    }

    #[tokio::test]
    async fn test_queue_isolation() {
        let (storage, _tmp) = create_test_storage().await;

        storage.enqueue("q-a", "From A", None, 0).await.unwrap();
        storage.enqueue("q-b", "From B", None, 0).await.unwrap();

        // Dequeue from q-a should only return q-a tasks.
        let task = storage.dequeue("q-a", 60).await.unwrap().unwrap();
        assert_eq!(task.queue_name, "q-a");
        assert_eq!(task.title, "From A");

        // q-b is still untouched.
        let stats_b = storage.queue_stats("q-b").await.unwrap();
        assert_eq!(stats_b.pending, 1);
    }
}
