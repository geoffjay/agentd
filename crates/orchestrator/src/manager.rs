use crate::storage::AgentStorage;
use crate::types::{Agent, AgentConfig, AgentStatus, AgentUsageStats, ClearContextResponse};
use crate::websocket::ConnectionRegistry;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;
use wrap::backend::ExecutionBackend;

/// Manages the lifecycle of AI agent processes.
///
/// Uses an [`ExecutionBackend`] trait object to interact with the underlying
/// session manager (tmux, Docker, etc.), making the orchestrator
/// backend-agnostic.
pub struct AgentManager {
    storage: Arc<AgentStorage>,
    backend: Arc<dyn ExecutionBackend>,
    registry: ConnectionRegistry,
    /// The base URL agents will use to connect back via WebSocket.
    ws_base_url: String,
}

impl AgentManager {
    pub fn new(
        storage: Arc<AgentStorage>,
        backend: Arc<dyn ExecutionBackend>,
        registry: ConnectionRegistry,
        ws_base_url: String,
    ) -> Self {
        Self { storage, backend, registry, ws_base_url }
    }

    pub fn registry(&self) -> &ConnectionRegistry {
        &self.registry
    }

    /// Spawn a new agent: create DB record, backend session, and launch claude.
    ///
    /// If the agent config includes a prompt, it is NOT passed via `-p` (which
    /// would cause claude to exit after processing it). Instead, claude is
    /// started in long-running SDK mode and the initial prompt is sent via the
    /// WebSocket once the agent connects. This keeps the agent alive for
    /// follow-up messages.
    pub async fn spawn_agent(&self, name: String, config: AgentConfig) -> anyhow::Result<Agent> {
        let mut agent = Agent::new(name, config);
        let session_name = format!("{}-{}", self.backend.prefix(), agent.id);

        // Persist agent record.
        self.storage.add(&agent).await?;

        // Create a session in the agent's working directory.
        let session_config = wrap::backend::SessionConfig {
            session_name: session_name.clone(),
            working_dir: agent.config.working_dir.clone(),
            agent_type: "claude-code".into(),
            model_provider: "anthropic".into(),
            model_name: agent.config.model.clone().unwrap_or_default(),
            layout: None,
            network_policy: agent.config.network_policy.clone(),
        };

        if let Err(e) = self.backend.create_session(&session_config).await {
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to create session: {}", e));
        }

        // Build the claude command (never uses -p; prompt sent via WebSocket).
        let ws_url = self
            .backend
            .agent_ws_url(&session_name, Some(&session_config))
            .unwrap_or_else(|| format!("{}/ws/{}", self.ws_base_url, agent.id));
        let claude_cmd = build_claude_command(&agent.config, &ws_url);

        // Send the command into the session.
        if let Err(e) = self.backend.send_command(&session_name, &claude_cmd).await {
            let _ = self.backend.kill_session(&session_name).await;
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to launch claude in session: {}", e));
        }

        // Mark as running.
        agent.status = AgentStatus::Running;
        agent.session_id = Some(session_name.clone());
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

        if let Some(ref session) = agent.session_id {
            if let Err(e) = self.backend.kill_session(session).await {
                warn!(agent_id = %id, %e, "Failed to kill session");
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

    /// Reconcile DB state with actual backend sessions and WebSocket connections on startup.
    ///
    /// Handles agents marked as `Running` against the actual backend state:
    ///
    /// 1. **Session is gone** — the process/container died unexpectedly.
    ///    Check exit info to determine status: exit code 0 → `Stopped`,
    ///    non-zero or unknown → `Failed`.
    ///
    /// 2. **Session is alive but agent is not connected to the registry** —
    ///    the orchestrator was restarted and the in-memory `ConnectionRegistry`
    ///    was reset. The Claude process is still running but holds a stale
    ///    WebSocket connection. Kill the session and re-launch so it
    ///    establishes a fresh connection.
    ///
    /// 3. **Session is alive and agent is connected** — everything is fine,
    ///    nothing to do.
    ///
    /// After handling known agents, cleans up any orphaned backend sessions
    /// (containers/tmux sessions with the correct prefix but no matching
    /// DB record).
    pub async fn reconcile(&self) -> anyhow::Result<()> {
        let agents = self.storage.list(Some(AgentStatus::Running)).await?;
        let mut known_sessions: std::collections::HashSet<String> = std::collections::HashSet::new();

        for agent in &agents {
            if let Some(ref s) = agent.session_id {
                known_sessions.insert(s.clone());
            }
        }

        for agent in agents {
            let session_name = match agent.session_id.clone() {
                Some(s) => s,
                None => {
                    // No session ID — mark as Failed.
                    let mut agent = agent;
                    warn!(agent_id = %agent.id, "Agent marked running but has no session ID, marking failed");
                    agent.status = AgentStatus::Failed;
                    agent.updated_at = Utc::now();
                    let _ = self.storage.update(&agent).await;
                    continue;
                }
            };

            let session_alive = self.backend.session_exists(&session_name).await.unwrap_or(false);

            if !session_alive {
                // Case 1: session is gone — check exit info for diagnostics.
                let mut agent = agent;
                let exit_info = self.backend.session_exit_info(&session_name).await.ok().flatten();

                let new_status = match &exit_info {
                    Some(info) if info.exit_code == 0 => {
                        info!(
                            agent_id = %agent.id,
                            session = %session_name,
                            "Agent session exited cleanly (exit code 0), marking stopped"
                        );
                        AgentStatus::Stopped
                    }
                    Some(info) => {
                        warn!(
                            agent_id = %agent.id,
                            session = %session_name,
                            exit_code = info.exit_code,
                            error = ?info.error,
                            "Agent session exited with error, marking failed"
                        );
                        AgentStatus::Failed
                    }
                    None => {
                        warn!(
                            agent_id = %agent.id,
                            session = %session_name,
                            "Agent marked running but session is gone, marking failed"
                        );
                        AgentStatus::Failed
                    }
                };

                agent.status = new_status;
                agent.updated_at = Utc::now();
                if let Err(e) = self.storage.update(&agent).await {
                    error!(agent_id = %agent.id, %e, "Failed to update agent status");
                }
            } else if !self.registry.is_connected(&agent.id).await {
                // Case 2: session alive but WebSocket connection is stale.
                // Check health before restarting.
                let health = self.backend.session_health(&session_name).await.unwrap_or(
                    wrap::backend::SessionHealth::Unknown,
                );

                warn!(
                    agent_id = %agent.id,
                    session = %session_name,
                    health = %health,
                    "Agent has live session but is not connected to registry, restarting"
                );

                if let Err(e) = self.restart_agent(&agent).await {
                    error!(agent_id = %agent.id, %e, "Failed to restart stale agent during reconcile");
                }
            }
            // Case 3: alive and connected — nothing to do.
        }

        // Clean up orphaned backend sessions (sessions with our prefix but
        // no matching DB record).
        self.cleanup_orphaned_sessions(&known_sessions).await;

        Ok(())
    }

    /// Remove backend sessions that are labeled with this backend's prefix
    /// but have no corresponding agent record in the database.
    async fn cleanup_orphaned_sessions(
        &self,
        known_sessions: &std::collections::HashSet<String>,
    ) {
        let backend_sessions = match self.backend.list_sessions().await {
            Ok(s) => s,
            Err(e) => {
                warn!(%e, "Failed to list backend sessions for orphan cleanup");
                return;
            }
        };

        for session in backend_sessions {
            if !known_sessions.contains(&session) {
                warn!(
                    session = %session,
                    "Found orphaned backend session with no DB record, removing"
                );
                if let Err(e) = self.backend.kill_session(&session).await {
                    error!(session = %session, %e, "Failed to clean up orphaned session");
                } else {
                    info!(session = %session, "Orphaned session cleaned up");
                }
            }
        }
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

    /// Clear the agent's conversation context by ending the current session,
    /// restarting the Claude process, and opening a new session row.
    ///
    /// Steps:
    /// 1. Snapshot the current usage stats (so the caller knows what was cleared).
    /// 2. End the active session in storage (`ended_at = now()`).
    /// 3. Restart the agent process (kills tmux, relaunches Claude with same UUID).
    /// 4. Start a fresh session row in storage.
    /// 5. Return a [`ClearContextResponse`] with the pre-clear stats and the new
    ///    session number.
    ///
    /// If the agent is running and the restart fails, the session is still ended
    /// and a new one is still opened. Note: `restart_agent` always attempts
    /// `kill_session` before any hard-failure return, so in practice the old
    /// process is dead before we reach this point. The narrow exception is if
    /// `kill_session` itself fails (it only warns), in which case the old
    /// process *may* still hold context. We still advance the session counter
    /// to keep storage consistent.
    pub async fn clear_context(&self, id: &Uuid) -> anyhow::Result<ClearContextResponse> {
        let agent =
            self.storage.get(id).await?.ok_or_else(|| anyhow::anyhow!("Agent not found"))?;

        // 1. Capture the current usage stats before we mutate anything.
        let stats = self.storage.get_usage_stats(id).await?;
        let session_usage = stats.current_session;

        // 2. Close the active session (context is about to be wiped).
        self.storage.end_session(id).await?;

        // 3. Only restart the agent process if it is currently running.
        //    For non-running agents we just rotate the session without
        //    spawning a new process.
        if agent.status == AgentStatus::Running {
            if let Err(e) = self.restart_agent(&agent).await {
                error!(agent_id = %id, %e, "Failed to restart agent during clear_context; session bookkeeping will still proceed");
            }
        } else {
            warn!(agent_id = %id, status = %agent.status, "clear_context called on non-running agent; skipping process restart");
        }

        // 4. Open a fresh session row.
        self.storage.start_new_session(id).await?;

        // The new session number is deterministic: previous count + 1.
        let new_session_number = stats.session_count + 1;

        info!(agent_id = %id, new_session_number, "Agent context cleared");

        Ok(ClearContextResponse { agent_id: *id, session_usage, new_session_number })
    }

    /// Return the current and cumulative usage statistics for an agent.
    pub async fn get_usage_stats(&self, id: &Uuid) -> anyhow::Result<AgentUsageStats> {
        self.storage.get_usage_stats(id).await
    }

    /// Graceful shutdown: stop all managed agent sessions.
    ///
    /// Iterates over all running agents, marks them as `Stopped` in the
    /// database, then delegates to the backend's `shutdown_all_sessions`
    /// to clean up the actual processes/containers.
    ///
    /// The `leave_running` flag controls whether backend sessions are
    /// actually stopped or left running for reconnection on restart:
    /// - `false` (default): stop all sessions
    /// - `true`: only update DB status, leave sessions running
    pub async fn shutdown_all(&self, leave_running: bool) {
        info!(leave_running, "Shutting down all managed agents");

        // Update all running agents to Stopped in the database.
        let agents = match self.storage.list(Some(AgentStatus::Running)).await {
            Ok(a) => a,
            Err(e) => {
                error!(%e, "Failed to list running agents during shutdown");
                return;
            }
        };

        for mut agent in agents {
            agent.status = AgentStatus::Stopped;
            agent.updated_at = Utc::now();
            if let Err(e) = self.storage.update(&agent).await {
                error!(agent_id = %agent.id, %e, "Failed to update agent status during shutdown");
            }
        }

        if !leave_running {
            if let Err(e) = self.backend.shutdown_all_sessions().await {
                error!(%e, "Failed to shut down backend sessions");
            }
        } else {
            info!("Leaving backend sessions running for reconnection on restart");
        }
    }

    /// Restart a running agent: kill the current session and re-launch Claude.
    ///
    /// Preserves the agent's ID, name, and config. The prompt is NOT re-sent
    /// since the agent is being restarted mid-lifecycle.
    async fn restart_agent(&self, agent: &Agent) -> anyhow::Result<Agent> {
        let mut agent = agent.clone();

        // Kill the existing session.
        if let Some(ref session) = agent.session_id {
            if let Err(e) = self.backend.kill_session(session).await {
                warn!(agent_id = %agent.id, %e, "Failed to kill session during restart");
            }
        }

        // Create a new session.
        let session_name = format!("{}-{}", self.backend.prefix(), agent.id);
        let session_config = wrap::backend::SessionConfig {
            session_name: session_name.clone(),
            working_dir: agent.config.working_dir.clone(),
            agent_type: "claude-code".into(),
            model_provider: "anthropic".into(),
            model_name: agent.config.model.clone().unwrap_or_default(),
            layout: None,
            network_policy: agent.config.network_policy.clone(),
        };

        if let Err(e) = self.backend.create_session(&session_config).await {
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to create session on restart: {}", e));
        }

        // Build and send the claude command with the updated config.
        let ws_url = self
            .backend
            .agent_ws_url(&session_name, Some(&session_config))
            .unwrap_or_else(|| format!("{}/ws/{}", self.ws_base_url, agent.id));
        let claude_cmd = build_claude_command(&agent.config, &ws_url);

        if let Err(e) = self.backend.send_command(&session_name, &claude_cmd).await {
            let _ = self.backend.kill_session(&session_name).await;
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to launch claude on restart: {}", e));
        }

        // Update state.
        agent.status = AgentStatus::Running;
        agent.session_id = Some(session_name.clone());
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
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
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
