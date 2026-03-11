//! SeaORM-based persistent storage for agent records.
//!
//! Provides [`AgentStorage`], backed by a SQLite database via SeaORM.
//! The shared [`DatabaseConnection`] is also exposed for [`crate::scheduler::storage::SchedulerStorage`].

use crate::{
    entity::agent as agent_entity,
    entity::usage_session as session_entity,
    migration::Migrator,
    types::{
        Agent, AgentConfig, AgentStatus, AgentUsageStats, SessionUsage, ToolPolicy, UsageSnapshot,
    },
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
            session_id: Set(agent.session_id.clone()),
            tool_policy: Set(serde_json::to_string(&agent.config.tool_policy).unwrap_or_default()),
            backend_type: Set(agent.backend_type.clone()),
            model: Set(agent.config.model.clone()),
            env: Set(serde_json::to_string(&agent.config.env).unwrap_or_else(|_| "{}".to_string())),
            created_at: Set(agent.created_at.to_rfc3339()),
            updated_at: Set(agent.updated_at.to_rfc3339()),
            auto_clear_threshold: Set(agent.config.auto_clear_threshold.map(|v| v as i64)),
            network_policy: Set(
                agent.config.network_policy.as_ref().map(|p| p.to_string()),
            ),
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

    /// Updates the mutable fields of an agent (status, session_id, backend_type, tool_policy, model, updated_at).
    pub async fn update(&self, agent: &Agent) -> Result<()> {
        use sea_orm::sea_query::Expr;

        let result = agent_entity::Entity::update_many()
            .col_expr(agent_entity::Column::Status, Expr::value(agent.status.to_string()))
            .col_expr(agent_entity::Column::SessionId, Expr::value(agent.session_id.clone()))
            .col_expr(agent_entity::Column::BackendType, Expr::value(agent.backend_type.clone()))
            .col_expr(
                agent_entity::Column::ToolPolicy,
                Expr::value(serde_json::to_string(&agent.config.tool_policy).unwrap_or_default()),
            )
            .col_expr(agent_entity::Column::Model, Expr::value(agent.config.model.clone()))
            .col_expr(
                agent_entity::Column::AutoClearThreshold,
                Expr::value(agent.config.auto_clear_threshold.map(|v| v as i64)),
            )
            .col_expr(
                agent_entity::Column::NetworkPolicy,
                Expr::value(agent.config.network_policy.as_ref().map(|p| p.to_string())),
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

    // -----------------------------------------------------------------------
    // Usage session methods
    // -----------------------------------------------------------------------

    /// Returns `MAX(session_number) + 1` for the given agent, or `1` if no
    /// sessions exist yet.  Backend-agnostic (no raw SQL).
    async fn next_session_number(&self, agent_id_str: &str) -> Result<i32> {
        #[derive(Debug, sea_orm::FromQueryResult)]
        struct MaxSession {
            max_num: Option<i32>,
        }

        let result: Option<MaxSession> = session_entity::Entity::find()
            .filter(session_entity::Column::AgentId.eq(agent_id_str))
            .select_only()
            .column_as(session_entity::Column::SessionNumber.max(), "max_num")
            .into_model::<MaxSession>()
            .one(&self.db)
            .await?;

        Ok(result.and_then(|r| r.max_num).unwrap_or(0) + 1)
    }

    /// Upserts the active session row for `agent_id`.
    ///
    /// If an active session (where `ended_at IS NULL`) already exists, the
    /// snapshot values are *accumulated* into it and `result_count` is
    /// incremented by 1.  If no active session exists, a new one is created
    /// with `session_number = MAX(existing) + 1` and `started_at = now()`.
    pub async fn record_session_usage(
        &self,
        agent_id: &Uuid,
        snapshot: &UsageSnapshot,
    ) -> Result<()> {
        use sea_orm::sea_query::Expr;

        let agent_id_str = agent_id.to_string();

        // Find existing active session.
        let existing = session_entity::Entity::find()
            .filter(session_entity::Column::AgentId.eq(&agent_id_str))
            .filter(session_entity::Column::EndedAt.is_null())
            .one(&self.db)
            .await?;

        if let Some(row) = existing {
            // Accumulate into the existing row.
            session_entity::Entity::update_many()
                .col_expr(
                    session_entity::Column::InputTokens,
                    Expr::col(session_entity::Column::InputTokens)
                        .add(snapshot.input_tokens as i64),
                )
                .col_expr(
                    session_entity::Column::OutputTokens,
                    Expr::col(session_entity::Column::OutputTokens)
                        .add(snapshot.output_tokens as i64),
                )
                .col_expr(
                    session_entity::Column::CacheReadInputTokens,
                    Expr::col(session_entity::Column::CacheReadInputTokens)
                        .add(snapshot.cache_read_input_tokens as i64),
                )
                .col_expr(
                    session_entity::Column::CacheCreationInputTokens,
                    Expr::col(session_entity::Column::CacheCreationInputTokens)
                        .add(snapshot.cache_creation_input_tokens as i64),
                )
                .col_expr(
                    session_entity::Column::TotalCostUsd,
                    Expr::col(session_entity::Column::TotalCostUsd).add(snapshot.total_cost_usd),
                )
                .col_expr(
                    session_entity::Column::NumTurns,
                    Expr::col(session_entity::Column::NumTurns).add(snapshot.num_turns as i64),
                )
                .col_expr(
                    session_entity::Column::DurationMs,
                    Expr::col(session_entity::Column::DurationMs).add(snapshot.duration_ms as i64),
                )
                .col_expr(
                    session_entity::Column::DurationApiMs,
                    Expr::col(session_entity::Column::DurationApiMs)
                        .add(snapshot.duration_api_ms as i64),
                )
                .col_expr(
                    session_entity::Column::ResultCount,
                    Expr::col(session_entity::Column::ResultCount).add(1i32),
                )
                .filter(session_entity::Column::Id.eq(row.id))
                .exec(&self.db)
                .await?;
        } else {
            // Create a new session starting now.
            let next_number = self.next_session_number(&agent_id_str).await?;
            let now = Utc::now().to_rfc3339();
            let model = session_entity::ActiveModel {
                agent_id: Set(agent_id_str),
                session_number: Set(next_number),
                input_tokens: Set(snapshot.input_tokens as i64),
                output_tokens: Set(snapshot.output_tokens as i64),
                cache_read_input_tokens: Set(snapshot.cache_read_input_tokens as i64),
                cache_creation_input_tokens: Set(snapshot.cache_creation_input_tokens as i64),
                total_cost_usd: Set(snapshot.total_cost_usd),
                num_turns: Set(snapshot.num_turns as i64),
                duration_ms: Set(snapshot.duration_ms as i64),
                duration_api_ms: Set(snapshot.duration_api_ms as i64),
                result_count: Set(1),
                started_at: Set(now),
                ended_at: Set(None),
                ..Default::default()
            };
            session_entity::Entity::insert(model).exec(&self.db).await?;
        }

        Ok(())
    }

    /// Closes the active session for `agent_id` by setting `ended_at = now()`.
    ///
    /// No-op if no active session exists.
    pub async fn end_session(&self, agent_id: &Uuid) -> Result<()> {
        use sea_orm::sea_query::Expr;

        let agent_id_str = agent_id.to_string();
        let now = Utc::now().to_rfc3339();

        session_entity::Entity::update_many()
            .col_expr(session_entity::Column::EndedAt, Expr::value(Some(now)))
            .filter(session_entity::Column::AgentId.eq(&agent_id_str))
            .filter(session_entity::Column::EndedAt.is_null())
            .exec(&self.db)
            .await?;

        Ok(())
    }

    /// Starts a new session for `agent_id`.
    ///
    /// Queries the current maximum `session_number` for this agent and inserts
    /// a new empty row with `session_number + 1` and `started_at = now()`.
    pub async fn start_new_session(&self, agent_id: &Uuid) -> Result<()> {
        let agent_id_str = agent_id.to_string();
        let next_number = self.next_session_number(&agent_id_str).await?;
        let now = Utc::now().to_rfc3339();

        let model = session_entity::ActiveModel {
            agent_id: Set(agent_id_str),
            session_number: Set(next_number),
            input_tokens: Set(0),
            output_tokens: Set(0),
            cache_read_input_tokens: Set(0),
            cache_creation_input_tokens: Set(0),
            total_cost_usd: Set(0.0),
            num_turns: Set(0),
            duration_ms: Set(0),
            duration_api_ms: Set(0),
            result_count: Set(0),
            started_at: Set(now),
            ended_at: Set(None),
            ..Default::default()
        };

        session_entity::Entity::insert(model).exec(&self.db).await?;

        Ok(())
    }

    /// Returns aggregated usage statistics for `agent_id`.
    ///
    /// * `current_session` – the active (open) session, if any.
    /// * `cumulative` – sum across *all* sessions (open + closed).
    /// * `session_count` – total number of session rows for this agent.
    pub async fn get_usage_stats(&self, agent_id: &Uuid) -> Result<AgentUsageStats> {
        let agent_id_str = agent_id.to_string();

        // --- active session -------------------------------------------------
        let active = session_entity::Entity::find()
            .filter(session_entity::Column::AgentId.eq(&agent_id_str))
            .filter(session_entity::Column::EndedAt.is_null())
            .one(&self.db)
            .await?;

        let current_session = active.as_ref().map(model_to_session_usage).transpose()?;

        // --- all sessions ---------------------------------------------------
        let all_sessions: Vec<session_entity::Model> = session_entity::Entity::find()
            .filter(session_entity::Column::AgentId.eq(&agent_id_str))
            .all(&self.db)
            .await?;

        let session_count = all_sessions.len() as u32;

        // Aggregate across all sessions.
        let cumulative = if all_sessions.is_empty() {
            // No sessions yet — return a zero-value cumulative with now as start.
            SessionUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
                total_cost_usd: 0.0,
                num_turns: 0,
                duration_ms: 0,
                duration_api_ms: 0,
                result_count: 0,
                started_at: Utc::now(),
                ended_at: None,
            }
        } else {
            let mut input_tokens: u64 = 0;
            let mut output_tokens: u64 = 0;
            let mut cache_read: u64 = 0;
            let mut cache_creation: u64 = 0;
            let mut total_cost: f64 = 0.0;
            let mut num_turns: u64 = 0;
            let mut duration_ms: u64 = 0;
            let mut duration_api_ms: u64 = 0;
            let mut result_count: u32 = 0;
            let mut earliest_start: Option<DateTime<Utc>> = None;

            for row in &all_sessions {
                input_tokens += row.input_tokens.max(0) as u64;
                output_tokens += row.output_tokens.max(0) as u64;
                cache_read += row.cache_read_input_tokens.max(0) as u64;
                cache_creation += row.cache_creation_input_tokens.max(0) as u64;
                total_cost += row.total_cost_usd;
                num_turns += row.num_turns.max(0) as u64;
                duration_ms += row.duration_ms.max(0) as u64;
                duration_api_ms += row.duration_api_ms.max(0) as u64;
                result_count += row.result_count.max(0) as u32;

                let started = DateTime::parse_from_rfc3339(&row.started_at)
                    .map(|dt| dt.with_timezone(&Utc))?;
                match earliest_start {
                    None => earliest_start = Some(started),
                    Some(prev) if started < prev => earliest_start = Some(started),
                    _ => {}
                }
            }

            SessionUsage {
                input_tokens,
                output_tokens,
                cache_read_input_tokens: cache_read,
                cache_creation_input_tokens: cache_creation,
                total_cost_usd: total_cost,
                num_turns,
                duration_ms,
                duration_api_ms,
                result_count,
                started_at: earliest_start.unwrap_or_else(Utc::now),
                ended_at: None,
            }
        };

        Ok(AgentUsageStats { agent_id: *agent_id, current_session, cumulative, session_count })
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
            auto_clear_threshold: model.auto_clear_threshold.and_then(|v| u64::try_from(v).ok()),
            network_policy: model
                .network_policy
                .as_deref()
                .map(|s| s.parse())
                .transpose()
                .unwrap_or(None),
        },
        session_id: model.session_id,
        backend_type: model.backend_type,
        created_at: DateTime::parse_from_rfc3339(&model.created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&model.updated_at)?.with_timezone(&Utc),
    })
}

/// Convert a raw [`session_entity::Model`] into a domain [`SessionUsage`].
fn model_to_session_usage(model: &session_entity::Model) -> Result<SessionUsage> {
    let started_at = DateTime::parse_from_rfc3339(&model.started_at)?.with_timezone(&Utc);
    let ended_at = model
        .ended_at
        .as_deref()
        .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
        .transpose()?;

    Ok(SessionUsage {
        input_tokens: model.input_tokens.max(0) as u64,
        output_tokens: model.output_tokens.max(0) as u64,
        cache_read_input_tokens: model.cache_read_input_tokens.max(0) as u64,
        cache_creation_input_tokens: model.cache_creation_input_tokens.max(0) as u64,
        total_cost_usd: model.total_cost_usd,
        num_turns: model.num_turns.max(0) as u64,
        duration_ms: model.duration_ms.max(0) as u64,
        duration_api_ms: model.duration_api_ms.max(0) as u64,
        result_count: model.result_count.max(0) as u32,
        started_at,
        ended_at,
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
                network_policy: None,
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
        agent.session_id = Some("agentd-orch-test".to_string());
        agent.updated_at = Utc::now();
        storage.update(&agent).await.unwrap();

        let retrieved = storage.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, AgentStatus::Running);
        assert_eq!(retrieved.session_id, Some("agentd-orch-test".to_string()));
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

    // -----------------------------------------------------------------------
    // Usage session tests
    // -----------------------------------------------------------------------

    fn test_snapshot(input: u64, output: u64, cost: f64) -> UsageSnapshot {
        UsageSnapshot {
            input_tokens: input,
            output_tokens: output,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
            total_cost_usd: cost,
            num_turns: 1,
            duration_ms: 100,
            duration_api_ms: 50,
        }
    }

    #[tokio::test]
    async fn test_record_session_creates_first_session() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("usage-agent");
        storage.add(&agent).await.unwrap();

        let snap = test_snapshot(100, 50, 0.01);
        storage.record_session_usage(&agent.id, &snap).await.unwrap();

        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        assert_eq!(stats.session_count, 1);
        let current = stats.current_session.unwrap();
        assert_eq!(current.input_tokens, 100);
        assert_eq!(current.output_tokens, 50);
        assert_eq!(current.result_count, 1);
        assert_eq!(current.ended_at, None);
    }

    #[tokio::test]
    async fn test_record_session_accumulates() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("usage-agent-accum");
        storage.add(&agent).await.unwrap();

        storage.record_session_usage(&agent.id, &test_snapshot(100, 50, 0.01)).await.unwrap();
        storage.record_session_usage(&agent.id, &test_snapshot(200, 80, 0.02)).await.unwrap();

        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        // Still one session (upsert, not insert).
        assert_eq!(stats.session_count, 1);
        let current = stats.current_session.unwrap();
        assert_eq!(current.input_tokens, 300);
        assert_eq!(current.output_tokens, 130);
        assert!((current.total_cost_usd - 0.03).abs() < 1e-9);
        assert_eq!(current.result_count, 2);
    }

    #[tokio::test]
    async fn test_end_session() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("usage-agent-end");
        storage.add(&agent).await.unwrap();

        storage.record_session_usage(&agent.id, &test_snapshot(10, 5, 0.001)).await.unwrap();

        // Active session present before ending.
        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        assert!(stats.current_session.is_some());

        storage.end_session(&agent.id).await.unwrap();

        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        assert!(stats.current_session.is_none(), "session should be closed");
        assert_eq!(stats.session_count, 1);
    }

    #[tokio::test]
    async fn test_end_session_noop_when_no_active_session() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("usage-agent-noop");
        storage.add(&agent).await.unwrap();

        // Should not error even when there is no active session.
        storage.end_session(&agent.id).await.unwrap();

        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        assert!(stats.current_session.is_none());
        assert_eq!(stats.session_count, 0);
    }

    #[tokio::test]
    async fn test_start_new_session() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("usage-agent-new-session");
        storage.add(&agent).await.unwrap();

        // First session via record.
        storage.record_session_usage(&agent.id, &test_snapshot(10, 5, 0.001)).await.unwrap();
        storage.end_session(&agent.id).await.unwrap();

        // Start second session.
        storage.start_new_session(&agent.id).await.unwrap();

        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        assert_eq!(stats.session_count, 2);
        // New session is active but empty.
        let current = stats.current_session.unwrap();
        assert_eq!(current.input_tokens, 0);
        assert_eq!(current.result_count, 0);
    }

    #[tokio::test]
    async fn test_cumulative_aggregates_across_sessions() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("usage-agent-cumulative");
        storage.add(&agent).await.unwrap();

        // Session 1: record, then end.
        storage.record_session_usage(&agent.id, &test_snapshot(100, 40, 0.01)).await.unwrap();
        storage.end_session(&agent.id).await.unwrap();

        // Session 2: record via start_new_session + record.
        storage.start_new_session(&agent.id).await.unwrap();
        storage.record_session_usage(&agent.id, &test_snapshot(200, 60, 0.02)).await.unwrap();

        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        assert_eq!(stats.session_count, 2);

        // Current session is the open one (session 2).
        let current = stats.current_session.unwrap();
        assert_eq!(current.input_tokens, 200);

        // Cumulative should sum both sessions.
        assert_eq!(stats.cumulative.input_tokens, 300);
        assert_eq!(stats.cumulative.output_tokens, 100);
        assert!((stats.cumulative.total_cost_usd - 0.03).abs() < 1e-9);
        assert_eq!(stats.cumulative.result_count, 2);
    }

    #[tokio::test]
    async fn test_record_end_record_creates_session_2() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("usage-agent-reopen");
        storage.add(&agent).await.unwrap();

        // Session 1 via record, then end.
        storage.record_session_usage(&agent.id, &test_snapshot(100, 50, 0.01)).await.unwrap();
        storage.end_session(&agent.id).await.unwrap();

        // Session 2 via record again (no explicit start_new_session).
        storage.record_session_usage(&agent.id, &test_snapshot(200, 80, 0.02)).await.unwrap();

        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        assert_eq!(stats.session_count, 2, "should have two sessions");

        // The active session should be session 2 with the second snapshot's data.
        let current = stats.current_session.unwrap();
        assert_eq!(current.input_tokens, 200);
        assert_eq!(current.output_tokens, 80);
        assert_eq!(current.result_count, 1);
    }

    #[tokio::test]
    async fn test_get_usage_stats_no_sessions() {
        let (storage, _tmp) = create_test_storage().await;
        let agent = test_agent("usage-agent-empty");
        storage.add(&agent).await.unwrap();

        let stats = storage.get_usage_stats(&agent.id).await.unwrap();
        assert_eq!(stats.agent_id, agent.id);
        assert_eq!(stats.session_count, 0);
        assert!(stats.current_session.is_none());
        assert_eq!(stats.cumulative.input_tokens, 0);
        assert_eq!(stats.cumulative.result_count, 0);
    }
}
