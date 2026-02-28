use crate::storage::AgentStorage;
use crate::types::{Agent, AgentConfig, AgentStatus};
use crate::websocket::ConnectionRegistry;
use chrono::Utc;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;
use wrap::tmux::TmuxManager;

/// Manages the lifecycle of AI agent processes.
pub struct AgentManager {
    storage: Arc<AgentStorage>,
    tmux: TmuxManager,
    registry: ConnectionRegistry,
    /// The base URL agents will use to connect back via WebSocket.
    ws_base_url: String,
}

impl AgentManager {
    pub fn new(
        storage: Arc<AgentStorage>,
        tmux: TmuxManager,
        registry: ConnectionRegistry,
        ws_base_url: String,
    ) -> Self {
        Self { storage, tmux, registry, ws_base_url }
    }

    pub fn registry(&self) -> &ConnectionRegistry {
        &self.registry
    }

    /// Spawn a new agent: create DB record, tmux session, and launch claude.
    pub async fn spawn_agent(&self, name: String, config: AgentConfig) -> anyhow::Result<Agent> {
        let mut agent = Agent::new(name, config);
        let session_name = format!("{}-{}", self.tmux.prefix(), agent.id);

        // Persist agent record.
        self.storage.add(&agent).await?;

        // Create tmux session in the agent's working directory.
        if let Err(e) = self.tmux.create_session(&session_name, &agent.config.working_dir, None) {
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to create tmux session: {}", e));
        }

        // Build the claude command.
        let ws_url = format!("{}/ws/{}", self.ws_base_url, agent.id);
        let claude_cmd = build_claude_command(&ws_url, agent.config.user.as_deref());

        // Send the command into the tmux session.
        if let Err(e) = self.tmux.send_command(&session_name, &claude_cmd) {
            // Cleanup the session we just created.
            let _ = self.tmux.kill_session(&session_name);
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to launch claude in session: {}", e));
        }

        // Mark as running.
        agent.status = AgentStatus::Running;
        agent.tmux_session = Some(session_name.clone());
        agent.updated_at = Utc::now();
        self.storage.update(&agent).await?;

        info!(
            agent_id = %agent.id,
            session = %session_name,
            "Agent spawned"
        );

        Ok(agent)
    }

    /// Terminate a running agent: kill tmux session, update DB.
    pub async fn terminate_agent(&self, id: &Uuid) -> anyhow::Result<Agent> {
        let mut agent =
            self.storage.get(id).await?.ok_or_else(|| anyhow::anyhow!("Agent not found"))?;

        if let Some(ref session) = agent.tmux_session {
            if let Err(e) = self.tmux.kill_session(session) {
                warn!(agent_id = %id, %e, "Failed to kill tmux session");
            }
        }

        agent.status = AgentStatus::Stopped;
        agent.updated_at = Utc::now();
        self.storage.update(&agent).await?;

        info!(agent_id = %id, "Agent terminated");

        Ok(agent)
    }

    /// Reconcile DB state with actual tmux sessions on startup.
    pub async fn reconcile(&self) -> anyhow::Result<()> {
        let agents = self.storage.list(Some(AgentStatus::Running)).await?;

        for mut agent in agents {
            let session_alive = agent
                .tmux_session
                .as_ref()
                .map(|s| self.tmux.session_exists(s).unwrap_or(false))
                .unwrap_or(false);

            if !session_alive {
                warn!(
                    agent_id = %agent.id,
                    "Agent marked running but tmux session is gone, marking failed"
                );
                agent.status = AgentStatus::Failed;
                agent.updated_at = Utc::now();
                if let Err(e) = self.storage.update(&agent).await {
                    error!(agent_id = %agent.id, %e, "Failed to update agent status");
                }
            }
        }

        Ok(())
    }

    /// Get an agent by ID (delegates to storage).
    pub async fn get_agent(&self, id: &Uuid) -> anyhow::Result<Option<Agent>> {
        self.storage.get(id).await
    }

    /// List agents with optional status filter.
    pub async fn list_agents(&self, status: Option<AgentStatus>) -> anyhow::Result<Vec<Agent>> {
        self.storage.list(status).await
    }
}

fn build_claude_command(ws_url: &str, run_as_user: Option<&str>) -> String {
    let base = format!(
        "claude --sdk-url {} --print --output-format stream-json --input-format stream-json",
        ws_url
    );

    match run_as_user {
        Some(user) => format!("sudo -u {} {}", user, base),
        None => base,
    }
}
