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

    /// Terminate a running agent: kill tmux session and delete DB record.
    ///
    /// The record is deleted (not just updated to Stopped) so that `agent apply`
    /// can recreate an agent with the same name and `agent teardown` + `agent apply`
    /// forms a clean cycle without stale records accumulating in the database.
    pub async fn terminate_agent(&self, id: &Uuid) -> anyhow::Result<Agent> {
        let mut agent =
            self.storage.get(id).await?.ok_or_else(|| anyhow::anyhow!("Agent not found"))?;

        if let Some(ref session) = agent.tmux_session {
            if let Err(e) = self.tmux.kill_session(session) {
                warn!(agent_id = %id, %e, "Failed to kill tmux session");
            }
        }

        // Remove the record from storage entirely so the name can be reused.
        self.storage.delete(id).await?;

        // Set status on the returned value for callers that inspect it.
        agent.status = AgentStatus::Stopped;
        agent.updated_at = Utc::now();

        info!(agent_id = %id, "Agent terminated and record deleted");

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

/// Validate that an environment variable name is safe.
///
/// Only allows names matching `[A-Za-z_][A-Za-z0-9_]*`.  Names that fail
/// this check are silently dropped from the command to prevent shell
/// injection via malformed key names.
fn is_valid_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Shell-escape a value using single-quote escaping.
///
/// Produces `'value'`, with any embedded single-quote replaced by `'\''`
/// (close-quote, escaped-quote, reopen-quote).  This is safe for POSIX shells.
fn shell_escape_value(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Build a list of `KEY='value'` assignment strings for safe env injection.
///
/// Keys that fail name validation are silently skipped.
fn build_env_assignments(env: &std::collections::HashMap<String, String>) -> Vec<String> {
    let mut assignments: Vec<String> = env
        .iter()
        .filter(|(k, _)| is_valid_env_var_name(k))
        .map(|(k, v)| format!("{}={}", k, shell_escape_value(v)))
        .collect();
    // Sort for deterministic output (important for tests).
    assignments.sort();
    assignments
}

fn build_claude_command(config: &AgentConfig, ws_url: &str) -> String {
    let mut args = vec!["claude".to_string()];

    if config.interactive {
        // Interactive mode: no --sdk-url, no --print, no stream-json flags.
        // User can attach to the tmux session and interact directly.
    } else {
        args.push(format!("--sdk-url {}", ws_url));
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

    // NOTE: --print / -p is intentionally NOT used here. It causes claude to
    // exit after processing a single conversation, making the agent unable to
    // receive follow-up messages. In SDK mode (--sdk-url), the CLI stays alive
    // and processes multiple messages without --print.

    let base = args.join(" ");
    let env_assignments = build_env_assignments(&config.env);

    match config.user.as_deref() {
        Some(user) => {
            if env_assignments.is_empty() {
                format!("sudo -u {} {}", user, base)
            } else {
                // Pass env vars via `env` so they survive the sudo privilege
                // boundary regardless of sudoers env_keep configuration.
                format!("sudo -u {} env {} {}", user, env_assignments.join(" "), base)
            }
        }
        None => {
            if env_assignments.is_empty() {
                base
            } else {
                // Prefix the command with shell variable assignments.
                // The shell running inside the tmux session interprets these
                // as temporary env vars scoped to the claude invocation.
                format!("{} {}", env_assignments.join(" "), base)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolPolicy;
    use std::collections::HashMap;

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
            env: HashMap::new(),
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

    // -- env var injection tests --

    #[test]
    fn test_build_claude_command_with_env_vars() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-ant-test123".to_string());
        let config = AgentConfig { env, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");

        assert!(cmd.contains("ANTHROPIC_API_KEY='sk-ant-test123'"));
        // Env prefix must come before claude
        let env_pos = cmd.find("ANTHROPIC_API_KEY").unwrap();
        let claude_pos = cmd.find("claude").unwrap();
        assert!(env_pos < claude_pos, "env vars must appear before 'claude' in command");
    }

    #[test]
    fn test_build_claude_command_with_env_vars_and_sudo() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-ant-test".to_string());
        let config = AgentConfig { user: Some("deploy".to_string()), env, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");

        // For sudo, env vars are injected via `env` to cross the sudo boundary
        assert!(cmd.starts_with("sudo -u deploy env"));
        assert!(cmd.contains("ANTHROPIC_API_KEY='sk-ant-test'"));
        assert!(cmd.contains("claude"));
    }

    #[test]
    fn test_build_claude_command_env_value_shell_escaped() {
        let mut env = HashMap::new();
        // Value contains a single quote — must be properly escaped
        env.insert("MY_VAR".to_string(), "it's a value".to_string());
        let config = AgentConfig { env, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");

        // Single-quote escaping: ' → '\''
        assert!(cmd.contains("MY_VAR='it'\\''s a value'"));
    }

    #[test]
    fn test_build_claude_command_invalid_env_key_rejected() {
        let mut env = HashMap::new();
        // Malicious key attempting shell injection
        env.insert("BAD KEY; rm -rf /".to_string(), "value".to_string());
        env.insert("123STARTS_WITH_DIGIT".to_string(), "v".to_string());
        env.insert("GOOD_KEY".to_string(), "ok".to_string());
        let config = AgentConfig { env, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");

        assert!(cmd.contains("GOOD_KEY='ok'"));
        assert!(!cmd.contains("BAD KEY"));
        assert!(!cmd.contains("rm -rf"));
        assert!(!cmd.contains("123STARTS_WITH_DIGIT"));
    }

    #[test]
    fn test_build_claude_command_empty_env_no_prefix() {
        let config = base_config(); // env is empty
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");

        // Must start directly with claude (no env prefix)
        assert!(cmd.starts_with("claude"));
    }

    #[test]
    fn test_build_claude_command_env_with_interactive_mode() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://custom.api.example.com".to_string());
        let config = AgentConfig { interactive: true, env, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc");

        assert!(cmd.contains("ANTHROPIC_BASE_URL='https://custom.api.example.com'"));
        assert!(!cmd.contains("--sdk-url"));
        assert!(cmd.contains("claude"));
    }

    #[test]
    fn test_build_claude_command_multiple_env_vars_deterministic() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-key".to_string());
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://example.com".to_string());
        env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "tok-123".to_string());
        let config = AgentConfig { env, ..base_config() };
        let cmd1 = build_claude_command(&config, "ws://localhost:7006/ws/abc");
        let cmd2 = build_claude_command(&config, "ws://localhost:7006/ws/abc");

        // Output must be deterministic (sorted) across calls
        assert_eq!(cmd1, cmd2);
        // All three vars must appear
        assert!(cmd1.contains("ANTHROPIC_API_KEY="));
        assert!(cmd1.contains("ANTHROPIC_BASE_URL="));
        assert!(cmd1.contains("ANTHROPIC_AUTH_TOKEN="));
    }

    // -- is_valid_env_var_name tests --

    #[test]
    fn test_is_valid_env_var_name_valid() {
        assert!(is_valid_env_var_name("ANTHROPIC_API_KEY"));
        assert!(is_valid_env_var_name("MY_VAR"));
        assert!(is_valid_env_var_name("_PRIVATE"));
        assert!(is_valid_env_var_name("lower_case_ok"));
        assert!(is_valid_env_var_name("VAR123"));
        assert!(is_valid_env_var_name("A"));
    }

    #[test]
    fn test_is_valid_env_var_name_invalid() {
        assert!(!is_valid_env_var_name(""));
        assert!(!is_valid_env_var_name("123STARTS_WITH_DIGIT"));
        assert!(!is_valid_env_var_name("HAS SPACE"));
        assert!(!is_valid_env_var_name("HAS-DASH"));
        assert!(!is_valid_env_var_name("HAS=EQUALS"));
        assert!(!is_valid_env_var_name("BAD;SEMICOLON"));
        assert!(!is_valid_env_var_name("KEY\nNEWLINE"));
    }

    // -- shell_escape_value tests --

    #[test]
    fn test_shell_escape_value_simple() {
        assert_eq!(shell_escape_value("hello"), "'hello'");
        assert_eq!(shell_escape_value("sk-ant-api-key"), "'sk-ant-api-key'");
    }

    #[test]
    fn test_shell_escape_value_with_single_quote() {
        assert_eq!(shell_escape_value("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_shell_escape_value_with_special_chars() {
        // Dollar signs, backticks etc. are safe inside single quotes
        assert_eq!(shell_escape_value("$HOME`cmd`$(cmd)"), "'$HOME`cmd`$(cmd)'");
    }

    #[test]
    fn test_shell_escape_value_empty() {
        assert_eq!(shell_escape_value(""), "''");
    }
}
