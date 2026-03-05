use crate::storage::AgentStorage;
use crate::types::{Agent, AgentConfig, AgentStatus};
use crate::websocket::ConnectionRegistry;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
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
    ///
    /// If the agent config includes a prompt, it is NOT passed via `-p` (which
    /// would cause claude to exit after processing it). Instead, claude is
    /// started in long-running SDK mode and the initial prompt is sent via the
    /// WebSocket once the agent connects. This keeps the agent alive for
    /// follow-up messages.
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

        // Build the claude command (never uses -p; prompt sent via WebSocket).
        let ws_url = format!("{}/ws/{}", self.ws_base_url, agent.id);
        let claude_cmd = build_claude_command(&agent.config, &ws_url);

        // Send the command into the tmux session.
        if let Err(e) = self.tmux.send_command(&session_name, &claude_cmd) {
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

        // Register the agent's tool policy with the WebSocket registry.
        self.registry.set_policy(agent.id, agent.config.tool_policy.clone()).await;

        info!(
            agent_id = %agent.id,
            session = %session_name,
            "Agent spawned"
        );

        // If there's an initial prompt, send it via WebSocket once the agent
        // connects (poll briefly since the tmux/claude process needs a moment).
        if let Some(ref prompt) = agent.config.prompt {
            let registry = self.registry.clone();
            let agent_id = agent.id;
            let prompt = prompt.clone();
            tokio::spawn(async move {
                for attempt in 1..=30 {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    if registry.is_connected(&agent_id).await {
                        match registry.send_user_message(&agent_id, &prompt).await {
                            Ok(_) => {
                                info!(%agent_id, "Initial prompt sent via WebSocket");
                                return;
                            }
                            Err(e) => {
                                warn!(%agent_id, %e, "Failed to send initial prompt");
                                return;
                            }
                        }
                    }
                    if attempt % 5 == 0 {
                        info!(%agent_id, attempt, "Waiting for agent to connect...");
                    }
                }
                warn!(%agent_id, "Agent never connected, initial prompt not sent");
            });
        }

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
    #[allow(dead_code)]
    pub async fn list_agents(&self, status: Option<AgentStatus>) -> anyhow::Result<Vec<Agent>> {
        self.storage.list(status).await
    }

    /// List agents with pagination.
    pub async fn list_agents_paginated(
        &self,
        status: Option<AgentStatus>,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<(Vec<Agent>, usize)> {
        self.storage.list_paginated(status, limit, offset).await
    }

    /// Update an agent record in storage.
    pub async fn update_agent(&self, agent: &Agent) -> anyhow::Result<()> {
        self.storage.update(agent).await
    }

    /// Change the model for an agent.
    ///
    /// Updates the stored config. If `restart` is true and the agent is running,
    /// kills the current tmux session and re-launches Claude with the new model.
    pub async fn set_model(
        &self,
        id: &Uuid,
        model: Option<String>,
        restart: bool,
    ) -> anyhow::Result<Agent> {
        let mut agent =
            self.storage.get(id).await?.ok_or_else(|| anyhow::anyhow!("Agent not found"))?;

        agent.config.model = model.clone();
        agent.updated_at = Utc::now();
        self.storage.update(&agent).await?;

        info!(
            agent_id = %id,
            model = ?model,
            restart,
            "Agent model updated"
        );

        if restart && agent.status == AgentStatus::Running {
            agent = self.restart_agent(&agent).await?;
        }

        Ok(agent)
    }

    /// Restart a running agent: kill the current tmux session and re-launch Claude.
    ///
    /// Preserves the agent's ID, name, and config. The prompt is NOT re-sent
    /// since the agent is being restarted mid-lifecycle.
    async fn restart_agent(&self, agent: &Agent) -> anyhow::Result<Agent> {
        let mut agent = agent.clone();

        // Kill the existing tmux session.
        if let Some(ref session) = agent.tmux_session {
            if let Err(e) = self.tmux.kill_session(session) {
                warn!(agent_id = %agent.id, %e, "Failed to kill tmux session during restart");
            }
        }

        // Create a new tmux session.
        let session_name = format!("{}-{}", self.tmux.prefix(), agent.id);
        if let Err(e) = self.tmux.create_session(&session_name, &agent.config.working_dir, None) {
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to create tmux session on restart: {}", e));
        }

        // Build and send the claude command with the updated config.
        let ws_url = format!("{}/ws/{}", self.ws_base_url, agent.id);
        let claude_cmd = build_claude_command(&agent.config, &ws_url);

        if let Err(e) = self.tmux.send_command(&session_name, &claude_cmd) {
            let _ = self.tmux.kill_session(&session_name);
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to launch claude on restart: {}", e));
        }

        // Update state.
        agent.status = AgentStatus::Running;
        agent.tmux_session = Some(session_name.clone());
        agent.updated_at = Utc::now();
        self.storage.update(&agent).await?;

        // Re-register tool policy.
        self.registry.set_policy(agent.id, agent.config.tool_policy.clone()).await;

        info!(
            agent_id = %agent.id,
            session = %session_name,
            model = ?agent.config.model,
            "Agent restarted with new model"
        );

        Ok(agent)
    }
}

fn build_claude_command(config: &AgentConfig, ws_url: &str) -> String {
    let mut args = vec!["claude".to_string()];

    if config.interactive {
        // Interactive mode: no --sdk-url, no --print, no stream-json flags.
        // User can attach to the tmux session and interact directly.
    } else {
        args.push(format!("--sdk-url {}", ws_url));
        args.push("--print".to_string());
        args.push("--output-format stream-json".to_string());
        args.push("--input-format stream-json".to_string());
    }

    if let Some(ref model) = config.model {
        args.push(format!("--model {}", model));
    }

    if config.worktree {
        args.push("--worktree".to_string());
    }

    if let Some(ref system_prompt) = config.system_prompt {
        args.push(format!("--system-prompt '{}'", system_prompt.replace('\'', "'\\''")));
    }

    // NOTE: -p is intentionally NOT used here. Initial prompts are sent via
    // the WebSocket after the agent connects. Using -p causes claude to exit
    // after processing the single prompt, making the agent unable to receive
    // follow-up messages.

    let base = args.join(" ");

    match config.user.as_deref() {
        Some(user) => format!("sudo -u {} {}", user, base),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolPolicy;

    fn base_config() -> AgentConfig {
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
            env: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_build_claude_command_no_model() {
        let config = base_config();
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");
        assert!(!cmd.contains("--model"));
        assert!(cmd.contains("claude"));
        assert!(cmd.contains("--sdk-url"));
    }

    #[test]
    fn test_build_claude_command_with_model_alias() {
        let config = AgentConfig { model: Some("opus".to_string()), ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");
        assert!(cmd.contains("--model opus"));
    }

    #[test]
    fn test_build_claude_command_with_full_model_name() {
        let config = AgentConfig { model: Some("claude-sonnet-4-6".to_string()), ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");
        assert!(cmd.contains("--model claude-sonnet-4-6"));
    }

    #[test]
    fn test_build_claude_command_model_with_interactive() {
        let config =
            AgentConfig { model: Some("haiku".to_string()), interactive: true, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");
        assert!(cmd.contains("--model haiku"));
        assert!(!cmd.contains("--sdk-url"));
    }

    #[test]
    fn test_build_claude_command_model_with_sudo() {
        let config = AgentConfig {
            model: Some("sonnet".to_string()),
            user: Some("deploy".to_string()),
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");
        assert!(cmd.starts_with("sudo -u deploy"));
        assert!(cmd.contains("--model sonnet"));
    }
}
