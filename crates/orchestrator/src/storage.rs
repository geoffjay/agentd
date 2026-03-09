//! SeaORM-based persistent storage for agent records.
//!
//! Provides [`AgentStorage`], backed by a SQLite database via SeaORM.
//! The shared [`DatabaseConnection`] is also exposed for [`crate::scheduler::storage::SchedulerStorage`].

use crate::{
    entity::agent as agent_entity,
    migration::Migrator,
    types::{Agent, AgentConfig, AgentStatus, ToolPolicy},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use sea_orm_migration::prelude::MigratorTrait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Persistent storage backend for agent records using SeaORM + SQLite.
///
/// Holds a [`DatabaseConnection`] that is `Clone + Send + Sync`, so
/// [`AgentStorage`] itself can be cheaply cloned and shared across tasks.
///
/// The underlying database is also shared with [`crate::scheduler::storage::SchedulerStorage`]
/// via [`AgentStorage::db()`].
#[derive(Clone)]
pub struct AgentStorage {
    db: DatabaseConnection,
}

impl AgentStorage {
    /// Returns the platform-specific database file path.
    pub fn get_db_path() -> Result<PathBuf> {
        agentd_common::storage::get_db_path("agentd-orchestrator", "orchestrator.db")
    }

    /// Creates a new storage instance connected to the default path.
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    /// Creates a new storage instance connected to `db_path`.
    ///
    /// All pending SeaORM migrations (agents, workflows, dispatch_log) are
    /// applied before returning.
    pub async fn with_path(db_path: &Path) -> Result<Self> {
        let db = agentd_common::storage::create_connection(db_path).await?;
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }

    /// Exposes the underlying [`DatabaseConnection`] so that
    /// [`crate::scheduler::storage::SchedulerStorage`] can share the same
    /// connection without opening a second database file.
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Inserts an agent and returns its UUID.
    pub async fn add(&self, agent: &Agent) -> Result<Uuid> {
        let model = agent_entity::ActiveModel {
            id: Set(agent.id.to_string()),
            name: Set(agent.name.clone()),
            status: Set(agent.status.to_string()),
            working_dir: Set(agent.config.working_dir.clone()),
            user: Set(agent.config.user.clone()),
            shell: Set(agent.config.shell.clone()),
            interactive: Set(if agent.config.interactive { 1 } else { 0 }),
            prompt: Set(agent.config.prompt.clone()),
            worktree: Set(if agent.config.worktree { 1 } else { 0 }),
            system_prompt: Set(agent.config.system_prompt.clone()),
            tmux_session: Set(agent.tmux_session.clone()),
            tool_policy: Set(serde_json::to_string(&agent.config.tool_policy).unwrap_or_default()),
            model: Set(agent.config.model.clone()),
            env: Set(serde_json::to_string(&agent.config.env).unwrap_or_else(|_| "{}".to_string())),
            created_at: Set(agent.created_at.to_rfc3339()),
            updated_at: Set(agent.updated_at.to_rfc3339()),
            auto_clear_threshold: Set(agent.config.auto_clear_threshold.map(|v| v as i64)),
        };

        agent_entity::Entity::insert(model).exec(&self.db).await?;
        Ok(agent.id)
    }

    /// Retrieves an agent by its UUID.
    pub async fn get(&self, id: &Uuid) -> Result<Option<Agent>> {
        let model = agent_entity::Entity::find_by_id(id.to_string()).one(&self.db).await?;
        match model {
            Some(m) => Ok(Some(model_to_agent(m)?)),
            None => Ok(None),
        }
    }

    /// Updates the mutable fields of an agent (status, tmux_session, tool_policy, model, updated_at).
    pub async fn update(&self, agent: &Agent) -> Result<()> {
        use sea_orm::sea_query::Expr;

        let result = agent_entity::Entity::update_many()
            .col_expr(agent_entity::Column::Status, Expr::value(agent.status.to_string()))
            .col_expr(agent_entity::Column::TmuxSession, Expr::value(agent.tmux_session.clone()))
            .col_expr(
                agent_entity::Column::ToolPolicy,
                Expr::value(serde_json::to_string(&agent.config.tool_policy).unwrap_or_default()),
            )
            .col_expr(agent_entity::Column::Model, Expr::value(agent.config.model.clone()))
            .col_expr(
                agent_entity::Column::AutoClearThreshold,
                Expr::value(agent.config.auto_clear_threshold.map(|v| v as i64)),
            )
            .col_expr(agent_entity::Column::UpdatedAt, Expr::value(agent.updated_at.to_rfc3339()))
            .filter(agent_entity::Column::Id.eq(agent.id.to_string()))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            anyhow::bail!("Agent not found");
        }

        Ok(())
    }

    /// Permanently deletes an agent by UUID.
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        let result = agent_entity::Entity::delete_many()
            .filter(agent_entity::Column::Id.eq(id.to_string()))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            anyhow::bail!("Agent not found");
        }

        Ok(())
    }

    /// Lists all agents, optionally filtered by status (newest first).
    pub async fn list(&self, status_filter: Option<AgentStatus>) -> Result<Vec<Agent>> {
        let mut query =
            agent_entity::Entity::find().order_by(agent_entity::Column::CreatedAt, Order::Desc);

        if let Some(status) = status_filter {
            query = query.filter(agent_entity::Column::Status.eq(status.to_string()));
        }

        let models: Vec<agent_entity::Model> = query.all(&self.db).await?;
        models.into_iter().map(model_to_agent).collect()
    }

    /// Lists agents with pagination; returns `(items, total_count)`.
    pub async fn list_paginated(
        &self,
        status_filter: Option<AgentStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Agent>, usize)> {
        let condition = match &status_filter {
            Some(s) => Condition::all().add(agent_entity::Column::Status.eq(s.to_string())),
            None => Condition::all(),
        };

        let total =
            agent_entity::Entity::find().filter(condition.clone()).count(&self.db).await? as usize;

        let models: Vec<agent_entity::Model> = agent_entity::Entity::find()
            .filter(condition)
            .order_by(agent_entity::Column::CreatedAt, Order::Desc)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await?;

        let agents = models.into_iter().map(model_to_agent).collect::<Result<Vec<_>>>()?;
        Ok((agents, total))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a raw entity [`agent_entity::Model`] into the domain [`Agent`] type.
fn model_to_agent(model: agent_entity::Model) -> Result<Agent> {
    let tool_policy: ToolPolicy = serde_json::from_str(&model.tool_policy).unwrap_or_default();
    let env: HashMap<String, String> = serde_json::from_str(&model.env).unwrap_or_default();

    Ok(Agent {
        id: Uuid::parse_str(&model.id)?,
        name: model.name,
        status: model.status.parse()?,
        config: AgentConfig {
            working_dir: model.working_dir,
            user: model.user,
            shell: model.shell,
            interactive: model.interactive != 0,
            prompt: model.prompt,
            worktree: model.worktree != 0,
            system_prompt: model.system_prompt,
            tool_policy,
            model: model.model,
            env,
            auto_clear_threshold: model.auto_clear_threshold.map(|v| v as u64),
        },
        tmux_session: model.tmux_session,
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
    use tempfile::TempDir;

    async fn create_test_storage() -> (AgentStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = AgentStorage::with_path(&db_path).await.unwrap();
        (storage, temp_dir)
    }

    fn test_agent(name: &str) -> Agent {
        Agent::new(
            name.to_string(),
            AgentConfig {
                working_dir: "/tmp/test".to_string(),
                user: None,
                shell: "zsh".to_string(),
                interactive: false,
                prompt: None,
                worktree: false,
                system_prompt: None,
                tool_policy: ToolPolicy::default(),
                model: None,
                env: HashMap::new(),
                auto_clear_threshold: None,
            },
        )
    }

    #[tokio::test]
    async fn test_add_and_get() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("test-agent");
        let id = agent.id;

        storage.add(&agent).await.unwrap();
        let retrieved = storage.get(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.name, "test-agent");
        assert_eq!(retrieved.status, AgentStatus::Pending);
    }

    #[tokio::test]
    async fn test_update() {
        let (storage, _tmp) = create_test_storage().await;
        let mut agent = test_agent("test-agent");
        storage.add(&agent).await.unwrap();

        agent.status = AgentStatus::Running;
        agent.tmux_session = Some("agentd-orch-test".to_string());
        agent.updated_at = Utc::now();
        storage.update(&agent).await.unwrap();

        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, AgentStatus::Running);
        assert_eq!(retrieved.tmux_session, Some("agentd-orch-test".to_string()));
    }

    #[tokio::test]
    async fn test_delete() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("test-agent");
        let id = agent.id;
        storage.add(&agent).await.unwrap();
        storage.delete(&id).await.unwrap();
        assert!(storage.get(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_with_filter() {
        let (storage, _tmp) = create_test_storage().await;

        let mut a1 = test_agent("running-agent");
        a1.status = AgentStatus::Running;
        let a2 = test_agent("pending-agent");

        storage.add(&a1).await.unwrap();
        storage.add(&a2).await.unwrap();

        let running = storage.list(Some(AgentStatus::Running)).await.unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].name, "running-agent");

        let all = storage.list(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_update_persists_model() {
        let (storage, _tmp) = create_test_storage().await;
        let mut agent = test_agent("model-test");
        assert_eq!(agent.config.model, None);

        storage.add(&agent).await.unwrap();

        agent.config.model = Some("opus".to_string());
        agent.updated_at = chrono::Utc::now();
        storage.update(&agent).await.unwrap();

        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.model, Some("opus".to_string()));

        agent.config.model = Some("sonnet".to_string());
        agent.updated_at = chrono::Utc::now();
        storage.update(&agent).await.unwrap();

        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.model, Some("sonnet".to_string()));

        agent.config.model = None;
        agent.updated_at = chrono::Utc::now();
        storage.update(&agent).await.unwrap();

        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.model, None);
    }

    #[tokio::test]
    async fn test_add_with_model() {
        let (storage, _tmp) = create_test_storage().await;
        let mut agent = test_agent("model-agent");
        agent.config.model = Some("haiku".to_string());

        storage.add(&agent).await.unwrap();
        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.model, Some("haiku".to_string()));
    }

    #[tokio::test]
    async fn test_add_with_env() {
        let (storage, _tmp) = create_test_storage().await;
        let mut agent = test_agent("env-agent");
        agent.config.env.insert("ANTHROPIC_API_KEY".to_string(), "sk-test".to_string());
        agent
            .config
            .env
            .insert("ANTHROPIC_BASE_URL".to_string(), "https://example.com".to_string());

        storage.add(&agent).await.unwrap();
        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.env.get("ANTHROPIC_API_KEY"), Some(&"sk-test".to_string()));
        assert_eq!(
            retrieved.config.env.get("ANTHROPIC_BASE_URL"),
            Some(&"https://example.com".to_string())
        );
    }

    #[tokio::test]
    async fn test_add_with_empty_env() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("no-env-agent");

        storage.add(&agent).await.unwrap();
        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert!(retrieved.config.env.is_empty());
    }

    #[tokio::test]
    async fn test_add_with_auto_clear_threshold() {
        let (storage, _tmp) = create_test_storage().await;
        let mut agent = test_agent("threshold-agent");
        agent.config.auto_clear_threshold = Some(50_000);

        storage.add(&agent).await.unwrap();
        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.auto_clear_threshold, Some(50_000));
    }

    #[tokio::test]
    async fn test_auto_clear_threshold_defaults_to_none() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("no-threshold-agent");
        assert_eq!(agent.config.auto_clear_threshold, None);

        storage.add(&agent).await.unwrap();
        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.auto_clear_threshold, None);
    }

    #[tokio::test]
    async fn test_update_persists_auto_clear_threshold() {
        let (storage, _tmp) = create_test_storage().await;
        let mut agent = test_agent("threshold-update-agent");
        storage.add(&agent).await.unwrap();

        // Set a threshold and update.
        agent.config.auto_clear_threshold = Some(100_000);
        agent.updated_at = chrono::Utc::now();
        storage.update(&agent).await.unwrap();

        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.auto_clear_threshold, Some(100_000));

        // Clear the threshold and update.
        agent.config.auto_clear_threshold = None;
        agent.updated_at = chrono::Utc::now();
        storage.update(&agent).await.unwrap();

        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.config.auto_clear_threshold, None);
    }
}
