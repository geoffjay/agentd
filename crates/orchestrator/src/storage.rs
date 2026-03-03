use crate::types::{Agent, AgentConfig, AgentStatus, ToolPolicy};
use anyhow::Result;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use sqlx::{sqlite::SqlitePool, Row};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone)]
pub struct AgentStorage {
    pool: SqlitePool,
}

impl AgentStorage {
    /// Access the underlying SQLite pool (used by SchedulerStorage).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn get_db_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("", "", "agentd-orchestrator")
            .ok_or_else(|| anyhow::anyhow!("Failed to determine project directories"))?;

        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        Ok(data_dir.join("orchestrator.db"))
    }

    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    pub async fn with_path(db_path: &Path) -> Result<Self> {
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&db_url).await?;

        let storage = Self { pool };
        storage.init_schema().await?;

        Ok(storage)
    }

    async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                status TEXT NOT NULL,
                working_dir TEXT NOT NULL,
                user TEXT,
                shell TEXT NOT NULL,
                interactive INTEGER NOT NULL DEFAULT 0,
                prompt TEXT,
                worktree INTEGER NOT NULL DEFAULT 0,
                system_prompt TEXT,
                tmux_session TEXT,
                tool_policy TEXT NOT NULL DEFAULT '{"mode":"allow_all"}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn add(&self, agent: &Agent) -> Result<Uuid> {
        sqlx::query(
            r#"
            INSERT INTO agents (id, name, status, working_dir, user, shell, interactive, prompt, worktree, system_prompt, tmux_session, tool_policy, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(agent.id.to_string())
        .bind(&agent.name)
        .bind(agent.status.to_string())
        .bind(&agent.config.working_dir)
        .bind(agent.config.user.as_deref())
        .bind(&agent.config.shell)
        .bind(agent.config.interactive)
        .bind(agent.config.prompt.as_deref())
        .bind(agent.config.worktree)
        .bind(agent.config.system_prompt.as_deref())
        .bind(agent.tmux_session.as_deref())
        .bind(serde_json::to_string(&agent.config.tool_policy).unwrap_or_default())
        .bind(agent.created_at.to_rfc3339())
        .bind(agent.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(agent.id)
    }

    pub async fn get(&self, id: &Uuid) -> Result<Option<Agent>> {
        let row = sqlx::query("SELECT * FROM agents WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(row_to_agent(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn update(&self, agent: &Agent) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE agents
            SET status = ?, tmux_session = ?, tool_policy = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(agent.status.to_string())
        .bind(agent.tmux_session.as_deref())
        .bind(serde_json::to_string(&agent.config.tool_policy).unwrap_or_default())
        .bind(agent.updated_at.to_rfc3339())
        .bind(agent.id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Agent not found");
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(&self, id: &Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Agent not found");
        }

        Ok(())
    }

    pub async fn list(&self, status_filter: Option<AgentStatus>) -> Result<Vec<Agent>> {
        let rows = if let Some(status) = status_filter {
            sqlx::query("SELECT * FROM agents WHERE status = ? ORDER BY created_at DESC")
                .bind(status.to_string())
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query("SELECT * FROM agents ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?
        };

        rows.iter().map(row_to_agent).collect()
    }
}

fn row_to_agent(row: &sqlx::sqlite::SqliteRow) -> Result<Agent> {
    let id: String = row.get("id");
    let status_str: String = row.get("status");
    let user: Option<String> = row.get("user");
    let interactive: bool = row.get::<i32, _>("interactive") != 0;
    let prompt: Option<String> = row.get("prompt");
    let worktree: bool = row.get::<i32, _>("worktree") != 0;
    let system_prompt: Option<String> = row.get("system_prompt");
    let tmux_session: Option<String> = row.get("tmux_session");
    let tool_policy_str: String = row.get("tool_policy");
    let tool_policy: ToolPolicy = serde_json::from_str(&tool_policy_str).unwrap_or_default();
    let created_at: String = row.get("created_at");
    let updated_at: String = row.get("updated_at");

    Ok(Agent {
        id: Uuid::parse_str(&id)?,
        name: row.get("name"),
        status: status_str.parse()?,
        config: AgentConfig {
            working_dir: row.get("working_dir"),
            user,
            shell: row.get("shell"),
            interactive,
            prompt,
            worktree,
            system_prompt,
            tool_policy,
        },
        tmux_session,
        created_at: DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&Utc),
    })
}

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
}
