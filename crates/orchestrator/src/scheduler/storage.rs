//! SeaORM-based persistent storage for workflows and dispatch logs.
//!
//! [`SchedulerStorage`] shares the same [`DatabaseConnection`] as
//! [`crate::storage::AgentStorage`] — the database schema (including the
//! `workflows` and `dispatch_log` tables) is managed by the single
//! [`crate::migration::Migrator`] that runs at startup.

use crate::{
    entity::{dispatch as dispatch_entity, workflow as workflow_entity},
    scheduler::types::{DispatchRecord, DispatchStatus, WorkflowConfig},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
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
        let source_config_json = serde_json::to_string(&workflow.source_config)?;
        let tool_policy_json = serde_json::to_string(&workflow.tool_policy).unwrap_or_default();

        let model = workflow_entity::ActiveModel {
            id: Set(workflow.id.to_string()),
            name: Set(workflow.name.clone()),
            agent_id: Set(workflow.agent_id.to_string()),
            source_type: Set(workflow.source_config.source_type().to_string()),
            source_config: Set(source_config_json),
            prompt_template: Set(workflow.prompt_template.clone()),
            poll_interval_secs: Set(workflow.poll_interval_secs as i64),
            enabled: Set(if workflow.enabled { 1 } else { 0 }),
            tool_policy: Set(tool_policy_json),
            created_at: Set(workflow.created_at.to_rfc3339()),
            updated_at: Set(workflow.updated_at.to_rfc3339()),
        };

        workflow_entity::Entity::insert(model).exec(&self.db).await?;
        Ok(workflow.id)
    }

    /// Retrieves a workflow by its UUID.
    pub async fn get_workflow(&self, id: &Uuid) -> Result<Option<WorkflowConfig>> {
        let model =
            workflow_entity::Entity::find_by_id(id.to_string()).one(&self.db).await?;
        match model {
            Some(m) => Ok(Some(model_to_workflow(m)?)),
            None => Ok(None),
        }
    }

    /// Lists all workflows ordered by creation time (newest first).
    pub async fn list_workflows(&self) -> Result<Vec<WorkflowConfig>> {
        let models: Vec<workflow_entity::Model> = workflow_entity::Entity::find()
            .order_by(workflow_entity::Column::CreatedAt, Order::Desc)
            .all(&self.db)
            .await?;
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

    /// Lists workflows with pagination; returns `(items, total_count)`.
    pub async fn list_workflows_paginated(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<WorkflowConfig>, usize)> {
        let total = workflow_entity::Entity::find().count(&self.db).await? as usize;

        let models: Vec<workflow_entity::Model> = workflow_entity::Entity::find()
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

        let total = dispatch_entity::Entity::find()
            .filter(condition.clone())
            .count(&self.db)
            .await? as usize;

        let models: Vec<dispatch_entity::Model> = dispatch_entity::Entity::find()
            .filter(condition)
            .order_by(dispatch_entity::Column::DispatchedAt, Order::Desc)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await?;

        let dispatches =
            models.into_iter().map(model_to_dispatch).collect::<Result<Vec<_>>>()?;
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
        source_config: serde_json::from_str(&model.source_config)?,
        prompt_template: model.prompt_template,
        poll_interval_secs: model.poll_interval_secs as u64,
        enabled: model.enabled != 0,
        tool_policy: serde_json::from_str::<ToolPolicy>(&model.tool_policy).unwrap_or_default(),
        created_at: DateTime::parse_from_rfc3339(&model.created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&model.updated_at)?.with_timezone(&Utc),
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
    use crate::scheduler::types::TaskSourceConfig;
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
            source_config: TaskSourceConfig::GithubIssues {
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
        let all = storage.list_workflows().await.unwrap();
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
}
