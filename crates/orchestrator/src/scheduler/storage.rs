use crate::scheduler::types::{DispatchRecord, DispatchStatus, WorkflowConfig};
use crate::types::ToolPolicy;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePool, Row};
use uuid::Uuid;

/// Persistent storage for workflows and dispatch logs.
#[derive(Clone)]
pub struct SchedulerStorage {
    pool: SqlitePool,
}

impl SchedulerStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                agent_id TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_config TEXT NOT NULL,
                prompt_template TEXT NOT NULL,
                poll_interval_secs INTEGER NOT NULL DEFAULT 60,
                enabled INTEGER NOT NULL DEFAULT 1,
                tool_policy TEXT NOT NULL DEFAULT '{"mode":"allow_all"}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS dispatch_log (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                source_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                prompt_sent TEXT NOT NULL,
                status TEXT NOT NULL,
                dispatched_at TEXT NOT NULL,
                completed_at TEXT,
                UNIQUE(workflow_id, source_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_dispatch_workflow ON dispatch_log(workflow_id)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_dispatch_status ON dispatch_log(status)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // -- Workflow CRUD --

    pub async fn add_workflow(&self, workflow: &WorkflowConfig) -> Result<Uuid> {
        let source_config_json = serde_json::to_string(&workflow.source_config)?;
        let tool_policy_json = serde_json::to_string(&workflow.tool_policy).unwrap_or_default();
        sqlx::query(
            r#"
            INSERT INTO workflows (id, name, agent_id, source_type, source_config, prompt_template, poll_interval_secs, enabled, tool_policy, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(workflow.id.to_string())
        .bind(&workflow.name)
        .bind(workflow.agent_id.to_string())
        .bind(workflow.source_config.source_type())
        .bind(&source_config_json)
        .bind(&workflow.prompt_template)
        .bind(workflow.poll_interval_secs as i64)
        .bind(workflow.enabled as i32)
        .bind(&tool_policy_json)
        .bind(workflow.created_at.to_rfc3339())
        .bind(workflow.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(workflow.id)
    }

    pub async fn get_workflow(&self, id: &Uuid) -> Result<Option<WorkflowConfig>> {
        let row = sqlx::query("SELECT * FROM workflows WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(row_to_workflow(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn list_workflows(&self) -> Result<Vec<WorkflowConfig>> {
        let rows = sqlx::query("SELECT * FROM workflows ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_workflow).collect()
    }

    pub async fn update_workflow(&self, workflow: &WorkflowConfig) -> Result<()> {
        let tool_policy_json = serde_json::to_string(&workflow.tool_policy).unwrap_or_default();
        let result = sqlx::query(
            r#"
            UPDATE workflows
            SET name = ?, prompt_template = ?, poll_interval_secs = ?, enabled = ?, tool_policy = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&workflow.name)
        .bind(&workflow.prompt_template)
        .bind(workflow.poll_interval_secs as i64)
        .bind(workflow.enabled as i32)
        .bind(&tool_policy_json)
        .bind(workflow.updated_at.to_rfc3339())
        .bind(workflow.id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Workflow not found");
        }

        Ok(())
    }

    pub async fn delete_workflow(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM workflows WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Workflow not found");
        }

        Ok(())
    }

    // -- Dispatch log --

    pub async fn add_dispatch(&self, record: &DispatchRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO dispatch_log (id, workflow_id, source_id, agent_id, prompt_sent, status, dispatched_at, completed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(record.id.to_string())
        .bind(record.workflow_id.to_string())
        .bind(&record.source_id)
        .bind(record.agent_id.to_string())
        .bind(&record.prompt_sent)
        .bind(record.status.to_string())
        .bind(record.dispatched_at.to_rfc3339())
        .bind(record.completed_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_dispatch_status(
        &self,
        id: &Uuid,
        status: DispatchStatus,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let result =
            sqlx::query("UPDATE dispatch_log SET status = ?, completed_at = ? WHERE id = ?")
                .bind(status.to_string())
                .bind(completed_at.map(|dt| dt.to_rfc3339()))
                .bind(id.to_string())
                .execute(&self.pool)
                .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Dispatch record not found");
        }

        Ok(())
    }

    /// Check if a task has already been dispatched for a given workflow.
    pub async fn is_dispatched(&self, workflow_id: &Uuid, source_id: &str) -> Result<bool> {
        let row = sqlx::query(
            "SELECT COUNT(*) as cnt FROM dispatch_log WHERE workflow_id = ? AND source_id = ?",
        )
        .bind(workflow_id.to_string())
        .bind(source_id)
        .fetch_one(&self.pool)
        .await?;

        let count: i32 = row.get("cnt");
        Ok(count > 0)
    }

    #[allow(dead_code)]
    pub async fn list_dispatches(&self, workflow_id: &Uuid) -> Result<Vec<DispatchRecord>> {
        let rows = sqlx::query(
            "SELECT * FROM dispatch_log WHERE workflow_id = ? ORDER BY dispatched_at DESC",
        )
        .bind(workflow_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_dispatch).collect()
    }

    /// List workflows with pagination.
    pub async fn list_workflows_paginated(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<WorkflowConfig>, usize)> {
        let count =
            sqlx::query("SELECT COUNT(*) as total FROM workflows").fetch_one(&self.pool).await?;
        let rows = sqlx::query("SELECT * FROM workflows ORDER BY created_at DESC LIMIT ? OFFSET ?")
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await?;

        let total: i64 = count.get("total");
        let workflows = rows.iter().map(row_to_workflow).collect::<Result<Vec<_>>>()?;
        Ok((workflows, total as usize))
    }

    /// List dispatch history with pagination.
    pub async fn list_dispatches_paginated(
        &self,
        workflow_id: &Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<DispatchRecord>, usize)> {
        let count = sqlx::query("SELECT COUNT(*) as total FROM dispatch_log WHERE workflow_id = ?")
            .bind(workflow_id.to_string())
            .fetch_one(&self.pool)
            .await?;

        let rows = sqlx::query(
            "SELECT * FROM dispatch_log WHERE workflow_id = ? ORDER BY dispatched_at DESC LIMIT ? OFFSET ?",
        )
        .bind(workflow_id.to_string())
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let total: i64 = count.get("total");
        let dispatches = rows.iter().map(row_to_dispatch).collect::<Result<Vec<_>>>()?;
        Ok((dispatches, total as usize))
    }

    /// Find the active (Dispatched) dispatch for a given agent.
    #[allow(dead_code)]
    pub async fn find_active_dispatch(&self, agent_id: &Uuid) -> Result<Option<DispatchRecord>> {
        let row = sqlx::query(
            "SELECT * FROM dispatch_log WHERE agent_id = ? AND status = 'dispatched' LIMIT 1",
        )
        .bind(agent_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row_to_dispatch(&row)?)),
            None => Ok(None),
        }
    }

    /// Mark all in-flight dispatches as failed (used during startup recovery).
    pub async fn fail_inflight_dispatches(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE dispatch_log SET status = 'failed', completed_at = ? WHERE status = 'dispatched'",
        )
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

fn row_to_workflow(row: &sqlx::sqlite::SqliteRow) -> Result<WorkflowConfig> {
    let id: String = row.get("id");
    let agent_id: String = row.get("agent_id");
    let source_config_json: String = row.get("source_config");
    let tool_policy_json: String = row.get("tool_policy");
    let enabled: bool = row.get::<i32, _>("enabled") != 0;
    let poll_interval: i64 = row.get("poll_interval_secs");
    let created_at: String = row.get("created_at");
    let updated_at: String = row.get("updated_at");

    Ok(WorkflowConfig {
        id: Uuid::parse_str(&id)?,
        name: row.get("name"),
        agent_id: Uuid::parse_str(&agent_id)?,
        source_config: serde_json::from_str(&source_config_json)?,
        prompt_template: row.get("prompt_template"),
        poll_interval_secs: poll_interval as u64,
        enabled,
        tool_policy: serde_json::from_str(&tool_policy_json).unwrap_or_default(),
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
    })
}

fn row_to_dispatch(row: &sqlx::sqlite::SqliteRow) -> Result<DispatchRecord> {
    let id: String = row.get("id");
    let workflow_id: String = row.get("workflow_id");
    let agent_id: String = row.get("agent_id");
    let status_str: String = row.get("status");
    let dispatched_at: String = row.get("dispatched_at");
    let completed_at: Option<String> = row.get("completed_at");

    Ok(DispatchRecord {
        id: Uuid::parse_str(&id)?,
        workflow_id: Uuid::parse_str(&workflow_id)?,
        source_id: row.get("source_id"),
        agent_id: Uuid::parse_str(&agent_id)?,
        prompt_sent: row.get("prompt_sent"),
        status: status_str.parse()?,
        dispatched_at: DateTime::parse_from_rfc3339(&dispatched_at)?.with_timezone(&Utc),
        completed_at: completed_at
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::types::TaskSourceConfig;
    use tempfile::TempDir;

    async fn create_test_storage() -> (SchedulerStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&db_url).await.unwrap();
        let storage = SchedulerStorage::new(pool);
        storage.init_schema().await.unwrap();
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
