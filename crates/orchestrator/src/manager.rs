use crate::scheduler::events::SystemEvent;
use crate::storage::{AgentStorage, ProjectStorage};
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
#[derive(Clone)]
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

    /// Returns a [`ProjectStorage`] backed by the same database connection.
    pub fn project_storage(&self) -> ProjectStorage {
        ProjectStorage::from_db(self.storage.db().clone())
    }

    /// Returns the underlying [`AgentStorage`] for direct access.
    pub fn agent_storage(&self) -> &AgentStorage {
        &self.storage
    }

    /// Spawn a new agent: create DB record, backend session, and launch claude.
    ///
    /// If the agent config includes a prompt, it is NOT passed via `-p` (which
    /// would cause claude to exit after processing it). Instead, claude is
    /// started in long-running SDK mode and the initial prompt is sent via the
    /// WebSocket once the agent connects. This keeps the agent alive for
    /// follow-up messages.
    /// Spawn a new agent.
    ///
    /// `built_in` — when `true`, marks the agent as a programmatically-managed
    /// system agent that cannot be deleted via the user-facing API.
    pub async fn spawn_agent(
        &self,
        name: String,
        config: AgentConfig,
        built_in: bool,
    ) -> anyhow::Result<Agent> {
        let mut agent = Agent::new(name, config);
        agent.built_in = built_in;
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

        // Warn about additional_dirs that don't exist at spawn time.
        for dir in &agent.config.additional_dirs {
            if !std::path::Path::new(dir).is_dir() {
                warn!(
                    agent_id = %agent.id,
                    dir = %dir,
                    "additional_dirs entry does not exist or is not a directory at spawn time"
                );
            }
        }

        // Warn if system_prompt_file is set but does not exist at spawn time.
        if let Some(ref path) = agent.config.system_prompt_file {
            if !std::path::Path::new(path).is_file() {
                warn!(
                    agent_id = %agent.id,
                    path = %path,
                    "system_prompt_file does not exist or is not a regular file at spawn time"
                );
            }
        }

        // Determine whether to use interactive / PTY mode.
        // PTY backend always uses PTY stdin (not WebSocket) so that the session
        // remains interactable. Config-level `interactive` flag is also respected.
        let effective_interactive = agent.config.interactive || self.backend.supports_pty_input();

        // Persist the effective interactive state so downstream consumers
        // (API, UI) see the correct mode.  The DB record is the single source
        // of truth; without this, PTY-backend agents are misidentified as SDK
        // mode because `agent.config.interactive` stays `false` in storage.
        if effective_interactive && !agent.config.interactive {
            agent.config.interactive = true;
        }

        // Build the claude command (never uses -p; prompt sent via WebSocket or PTY stdin).
        let ws_url = self
            .backend
            .agent_ws_url(&session_name, Some(&session_config))
            .unwrap_or_else(|| format!("{}/ws/{}", self.ws_base_url, agent.id));
        let claude_cmd = build_claude_command(&agent.config, &ws_url, effective_interactive);

        // Persist the launch command so the UI can display it for debugging.
        agent.launch_command = Some(claude_cmd.clone());

        // Send the command into the session.
        if let Err(e) = self.backend.send_command(&session_name, &claude_cmd).await {
            let _ = self.backend.kill_session(&session_name).await;
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to launch claude in session: {}", e));
        }

        // Mark as running and persist the PID for reconciliation.
        agent.status = AgentStatus::Running;
        agent.session_id = Some(session_name.clone());
        agent.pid = self.backend.session_pid(&session_name).await.unwrap_or(None);
        agent.updated_at = Utc::now();
        self.storage.update(&agent).await?;

        // Register the agent's tool policy with the WebSocket registry.
        self.registry.set_policy(agent.id, agent.config.tool_policy.clone()).await;

        info!(
            agent_id = %agent.id,
            session = %session_name,
            pid = ?agent.pid,
            "Agent spawned"
        );

        // If there's an initial prompt, deliver it via the appropriate channel.
        //
        // Interactive / PTY mode: Claude reads from PTY stdin, not the WebSocket.
        //   Write the prompt directly to PTY stdin. A brief delay lets the
        //   shell and Claude process start before input arrives.
        //
        // SDK mode: Claude connects back to the orchestrator WebSocket.
        //   Poll until the agent connects, then send via WebSocket.
        if let Some(ref prompt) = agent.config.prompt {
            let prompt = prompt.clone();
            let agent_id = agent.id;

            if effective_interactive {
                // Interactive mode — inject via PTY stdin.
                let manager = self.clone();
                tokio::spawn(async move {
                    // Give the shell and Claude a moment to start.
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    match manager.inject_pty_prompt(&agent_id, &prompt).await {
                        Ok(_) => info!(%agent_id, "Initial prompt injected via PTY stdin"),
                        Err(e) => {
                            warn!(%agent_id, %e, "Failed to inject initial prompt via PTY stdin")
                        }
                    }
                });
            } else {
                // SDK mode — wait for the agent to connect, then send via WebSocket.
                let registry = self.registry.clone();
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

        // System agents are managed by the orchestrator and must not be
        // deleted via the user-facing API.
        if agent.built_in {
            anyhow::bail!(
                "Cannot terminate built-in system agent '{}'. \
                 System agents are managed by the orchestrator.",
                agent.name
            );
        }

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

    /// Bootstrap built-in system agents at orchestrator startup.
    ///
    /// Called once after [`reconcile`] so that reconciliation has already
    /// handled any surviving user agents before we insert/restart system agents.
    ///
    /// Behaviour:
    /// 1. Query storage for existing built-in agents.
    /// 2. If the system agent exists and is `Running` → skip (reconcile handled it).
    /// 3. If it exists but is `Stopped` or `Failed` → restart it.
    /// 4. If it doesn't exist → create it via `spawn_agent()` with `built_in: true`.
    pub async fn bootstrap_system_agents(&self) -> anyhow::Result<()> {
        use crate::system_agents::build_system_agent_config;
        use crate::system_agents::SYSTEM_AGENT_NAME;

        let existing = self.storage.list_system_agents().await?;
        let system_agent = existing.into_iter().find(|a| a.name == SYSTEM_AGENT_NAME);

        match system_agent {
            Some(agent) if agent.status == AgentStatus::Running => {
                // Reconciliation already handled it — nothing to do.
                info!(
                    agent_id = %agent.id,
                    "System agent '{}' is already running, skipping bootstrap",
                    SYSTEM_AGENT_NAME
                );
            }
            Some(agent) => {
                // Exists but stopped/failed — restart it.
                info!(
                    agent_id = %agent.id,
                    status = %agent.status,
                    "Restarting system agent '{}'",
                    SYSTEM_AGENT_NAME
                );
                if let Err(e) = self.restart_agent(&agent).await {
                    error!(
                        agent_id = %agent.id,
                        %e,
                        "Failed to restart system agent '{}'",
                        SYSTEM_AGENT_NAME
                    );
                }
            }
            None => {
                // First run — create the system agent.
                info!("Bootstrapping system agent '{}'", SYSTEM_AGENT_NAME);
                let config = build_system_agent_config();
                match self.spawn_agent(SYSTEM_AGENT_NAME.to_string(), config, true).await {
                    Ok(agent) => {
                        info!(agent_id = %agent.id, "System agent '{}' spawned", SYSTEM_AGENT_NAME)
                    }
                    Err(e) => {
                        error!(%e, "Failed to spawn system agent '{}'", SYSTEM_AGENT_NAME)
                    }
                }
            }
        }

        Ok(())
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
        let agents = self.storage.list(Some(AgentStatus::Running), None).await?;
        let mut known_sessions: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for agent in &agents {
            if let Some(ref s) = agent.session_id {
                known_sessions.insert(s.clone());
            }
        }

        for agent in agents {
            if let Err(e) = self.reconcile_agent(agent).await {
                error!(%e, "Failed to reconcile agent");
            }
        }

        // Clean up orphaned backend sessions (sessions with our prefix but
        // no matching DB record).
        self.cleanup_orphaned_sessions(&known_sessions).await;

        Ok(())
    }

    /// Reconcile a single agent by ID.
    ///
    /// Loads the agent from storage and checks whether its backend session
    /// and WebSocket connection are still alive. Updates status accordingly.
    /// This is called reactively when an `AgentDisconnected` event fires.
    pub async fn reconcile_single(&self, agent_id: &Uuid) -> anyhow::Result<()> {
        let agent = match self.storage.get(agent_id).await? {
            Some(a) => a,
            None => return Ok(()),
        };
        if agent.status != AgentStatus::Running {
            return Ok(());
        }
        self.reconcile_agent(agent).await
    }

    /// Reconcile a single agent record against its backend session state.
    async fn reconcile_agent(&self, agent: Agent) -> anyhow::Result<()> {
        let session_name = match agent.session_id.clone() {
            Some(s) => s,
            None => {
                let mut agent = agent;
                warn!(agent_id = %agent.id, "Agent marked running but has no session ID, marking failed");
                agent.status = AgentStatus::Failed;
                agent.updated_at = Utc::now();
                let _ = self.storage.update(&agent).await;
                return Ok(());
            }
        };

        let session_alive = self.backend.session_exists(&session_name).await.unwrap_or(false);

        if !session_alive {
            // Case 1: session is gone -- check exit info for diagnostics.
            let exit_info = self.backend.session_exit_info(&session_name).await.ok().flatten();

            match &exit_info {
                Some(info) if info.exit_code == 0 => {
                    info!(
                        agent_id = %agent.id,
                        session = %session_name,
                        "Agent session exited cleanly (exit code 0), marking stopped"
                    );
                    let mut agent = agent;
                    agent.status = AgentStatus::Stopped;
                    agent.updated_at = Utc::now();
                    if let Err(e) = self.storage.update(&agent).await {
                        error!(agent_id = %agent.id, %e, "Failed to update agent status");
                    }
                }
                Some(info) => {
                    warn!(
                        agent_id = %agent.id,
                        session = %session_name,
                        exit_code = info.exit_code,
                        error = ?info.error,
                        "Agent session exited with error, marking failed"
                    );
                    let mut agent = agent;
                    agent.status = AgentStatus::Failed;
                    agent.updated_at = Utc::now();
                    if let Err(e) = self.storage.update(&agent).await {
                        error!(agent_id = %agent.id, %e, "Failed to update agent status");
                    }
                }
                None => {
                    // No exit info means the backend has no record of this
                    // session. For in-memory backends (subprocess, PTY) this
                    // happens after a service restart. The process may still
                    // be alive if it was in its own process group (setpgid).
                    // Check the stored PID before spawning a duplicate.
                    if let Some(pid) = agent.pid {
                        if is_process_alive(pid) {
                            info!(
                                agent_id = %agent.id,
                                session = %session_name,
                                pid,
                                "Agent process still alive after service restart, skipping restart"
                            );
                            return Ok(());
                        }
                        info!(
                            agent_id = %agent.id,
                            session = %session_name,
                            pid,
                            "Agent process is dead, restarting"
                        );
                    } else {
                        info!(
                            agent_id = %agent.id,
                            session = %session_name,
                            "Agent session lost (no PID recorded), restarting"
                        );
                    }

                    if let Err(e) = self.restart_agent(&agent).await {
                        error!(agent_id = %agent.id, %e, "Failed to restart agent during reconcile");
                        let mut agent = agent;
                        agent.status = AgentStatus::Failed;
                        agent.updated_at = Utc::now();
                        let _ = self.storage.update(&agent).await;
                    }
                }
            };
        } else if !self.registry.is_connected(&agent.id).await {
            // Case 2: session alive but WebSocket connection is stale.
            //
            // This commonly happens after sleep/wake: the WebSocket drops but
            // the agent process is still alive and will reconnect. Check the
            // PID before killing a healthy process.
            if let Some(pid) = agent.pid {
                if is_process_alive(pid) {
                    info!(
                        agent_id = %agent.id,
                        session = %session_name,
                        pid,
                        "Agent process alive but WebSocket disconnected (sleep/wake?), \
                         waiting for reconnect"
                    );
                    return Ok(());
                }
            }

            let health = self
                .backend
                .session_health(&session_name)
                .await
                .unwrap_or(wrap::backend::SessionHealth::Unknown);

            warn!(
                agent_id = %agent.id,
                session = %session_name,
                health = %health,
                "Agent session alive but process dead and not connected, restarting"
            );

            if let Err(e) = self.restart_agent(&agent).await {
                error!(agent_id = %agent.id, %e, "Failed to restart stale agent during reconcile");
            }
        }
        // Case 3: alive and connected -- nothing to do.

        Ok(())
    }

    /// Remove backend sessions that are labeled with this backend's prefix
    /// but have no corresponding agent record in the database.
    async fn cleanup_orphaned_sessions(&self, known_sessions: &std::collections::HashSet<String>) {
        let backend_sessions = match self.backend.list_sessions().await {
            Ok(s) => s,
            Err(e) => {
                warn!(%e, "Failed to list backend sessions for orphan cleanup");
                return;
            }
        };

        let prefix = self.backend.prefix();
        for session in backend_sessions {
            if !session.starts_with(prefix) {
                continue;
            }
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
        self.storage.list(status, None).await
    }

    /// List agents with pagination.
    ///
    /// `built_in_filter`:
    /// - `Some(false)` -- exclude system agents (use for `GET /agents`)
    /// - `Some(true)` -- only system agents (use for `GET /system-agents`)
    /// - `None` -- all agents regardless of flag (use for debug/admin views)
    pub async fn list_agents_paginated(
        &self,
        status: Option<AgentStatus>,
        built_in_filter: Option<bool>,
        project_id: Option<Uuid>,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<(Vec<Agent>, usize)> {
        self.storage.list_paginated(status, built_in_filter, project_id, limit, offset).await
    }

    /// List all built-in system agents (newest first).
    ///
    /// Returns agents created programmatically by the orchestrator at startup.
    /// Used by `GET /system-agents`.
    pub async fn list_system_agents(&self) -> anyhow::Result<Vec<Agent>> {
        self.storage.list_system_agents().await
    }

    /// Update an agent record in storage.
    pub async fn update_agent(&self, agent: &Agent) -> anyhow::Result<()> {
        self.storage.update(agent).await
    }

    /// Update the `additional_dirs` list for an agent in storage.
    pub async fn update_additional_dirs(&self, id: &Uuid, dirs: &[String]) -> anyhow::Result<()> {
        self.storage.update_additional_dirs(id, dirs).await
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

        // Publish context-cleared event.
        if let Some(bus) = self.registry.event_bus() {
            bus.publish(SystemEvent::ContextCleared { agent_id: *id });
        }

        // Persist context_cleared conversation event and advance the in-memory
        // session counter so subsequent events use the new session number.
        self.registry.persist_context_cleared(*id, new_session_number as i64).await;

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
        let agents = match self.storage.list(Some(AgentStatus::Running), None).await {
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

    /// Return the PTY output stream for an agent's session, if the backend
    /// supports it.
    ///
    /// Returns `Ok(None)` when the agent does not have a session or when the
    /// backend does not support PTY streaming (e.g., tmux or Docker backends).
    /// Returns `Ok(Some(stream))` for PTY-backed sessions.
    pub async fn get_agent_pty_stream(
        &self,
        agent_id: &Uuid,
    ) -> anyhow::Result<Option<wrap::pty_stream::PtyOutputStream>> {
        let agent =
            self.storage.get(agent_id).await?.ok_or_else(|| anyhow::anyhow!("Agent not found"))?;

        let session_name = match agent.session_id {
            Some(s) => s,
            None => return Ok(None),
        };

        self.backend.session_output_stream(&session_name).await
    }

    /// Inject a prompt into an interactive-mode agent by writing text to PTY
    /// stdin, exactly as if the user had typed it in the terminal.
    ///
    /// This is the programmatic prompt path for agents launched with
    /// `config.interactive = true`. In interactive mode Claude reads from PTY
    /// stdin, not from a WebSocket connection, so `send_user_message` cannot
    /// be used.
    ///
    /// A newline is appended automatically so Claude receives the prompt as a
    /// complete input line.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The agent is not found or has no active session.
    /// - The backend does not expose a PTY stream (tmux / Docker backends).
    /// - The PTY writer has been closed (session already exited).
    pub async fn inject_pty_prompt(&self, agent_id: &Uuid, prompt: &str) -> anyhow::Result<()> {
        let stream = self.get_agent_pty_stream(agent_id).await?.ok_or_else(|| {
            anyhow::anyhow!(
                "Agent {} has no PTY stream; inject_pty_prompt requires a PTY backend",
                agent_id
            )
        })?;

        // Append a newline so Claude receives a complete input line.
        let mut input = prompt.as_bytes().to_vec();
        input.push(b'\n');
        stream.write_input(&input)?;
        Ok(())
    }

    /// Resize the PTY terminal for an agent's session.
    ///
    /// No-ops silently for backends that do not support resize (tmux/Docker).
    /// Returns `Ok(())` when the agent has no active session.
    pub async fn resize_agent_pty(
        &self,
        agent_id: &Uuid,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<()> {
        let agent = match self.storage.get(agent_id).await? {
            Some(a) => a,
            None => return Ok(()),
        };

        let session_name = match agent.session_id {
            Some(s) => s,
            None => return Ok(()),
        };

        self.backend.resize_session(&session_name, cols, rows).await
    }

    /// Restart an agent by ID: kill any existing session and re-launch Claude.
    ///
    /// Accepts agents in any status (Running, Failed, Stopped). Preserves the
    /// agent's ID, name, and config. The initial prompt is NOT re-sent since
    /// the agent is being restarted, not created fresh.
    pub async fn restart_agent_by_id(&self, id: &Uuid) -> anyhow::Result<Agent> {
        let agent =
            self.storage.get(id).await?.ok_or_else(|| anyhow::anyhow!("Agent not found"))?;
        self.restart_agent(&agent).await
    }

    /// Internal restart: kill the current session and re-launch Claude.
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
        let effective_interactive = agent.config.interactive || self.backend.supports_pty_input();

        // Persist the effective interactive state (mirrors the fix in spawn_agent).
        if effective_interactive && !agent.config.interactive {
            agent.config.interactive = true;
        }

        let ws_url = self
            .backend
            .agent_ws_url(&session_name, Some(&session_config))
            .unwrap_or_else(|| format!("{}/ws/{}", self.ws_base_url, agent.id));
        let claude_cmd = build_claude_command(&agent.config, &ws_url, effective_interactive);

        // Persist the launch command so the UI can display it for debugging.
        agent.launch_command = Some(claude_cmd.clone());

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
        agent.pid = self.backend.session_pid(&session_name).await.unwrap_or(None);
        agent.updated_at = Utc::now();
        self.storage.update(&agent).await?;

        // Re-register tool policy.
        self.registry.set_policy(agent.id, agent.config.tool_policy.clone()).await;

        info!(
            agent_id = %agent.id,
            session = %session_name,
            pid = ?agent.pid,
            model = ?agent.config.model,
            "Agent restarted"
        );

        Ok(agent)
    }
}

/// Validate that an environment variable name is safe.
///
/// Only allows names matching `[A-Za-z_][A-Za-z0-9_]*`.  Names that fail
/// this check are silently dropped from the command to prevent shell
/// injection via malformed key names.
/// Check if a process with the given PID is still alive.
///
/// Uses `kill(pid, 0)` which checks for process existence without sending a
/// signal. Returns `false` if the process does not exist or is not owned by
/// the current user.
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    // SAFETY: kill with signal 0 checks process existence without side effects.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    // Cannot check on non-Unix; assume dead to trigger restart.
    false
}

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

fn build_claude_command(config: &AgentConfig, ws_url: &str, interactive: bool) -> String {
    let mut args = vec!["claude".to_string()];

    if interactive {
        // Interactive / PTY mode: no --sdk-url, no stream-json flags.
        // Prompts are delivered via PTY stdin injection; a human can also
        // type directly in the terminal.
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

    for dir in &config.additional_dirs {
        args.push(format!("--add-dir {}", shell_escape_value(dir)));
    }

    // System prompt flags — four combinations based on (file vs inline) × (replace vs append).
    match (config.append_system_prompt, &config.system_prompt, &config.system_prompt_file) {
        (false, Some(prompt), _) => {
            args.push(format!("--system-prompt {}", shell_escape_value(prompt)));
        }
        (false, None, Some(path)) => {
            args.push(format!("--system-prompt-file {}", shell_escape_value(path)));
        }
        (true, Some(prompt), _) => {
            args.push(format!("--append-system-prompt {}", shell_escape_value(prompt)));
        }
        (true, None, Some(path)) => {
            args.push(format!("--append-system-prompt-file {}", shell_escape_value(path)));
        }
        // Neither prompt nor file set — no flag emitted.
        (_, None, None) => {}
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
            system_prompt_file: None,
            append_system_prompt: false,
            tool_policy: ToolPolicy::default(),
            model: None,
            env: HashMap::new(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec![],
        }
    }

    #[test]
    fn test_build_claude_command_no_model() {
        let config = base_config();
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(!cmd.contains("--model"));
        assert!(cmd.contains("claude"));
        assert!(cmd.contains("--sdk-url"));
    }

    #[test]
    fn test_build_claude_command_with_model_alias() {
        let config = AgentConfig { model: Some("opus".to_string()), ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--model opus"));
    }

    #[test]
    fn test_build_claude_command_with_full_model_name() {
        let config = AgentConfig { model: Some("claude-sonnet-4-6".to_string()), ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--model claude-sonnet-4-6"));
    }

    #[test]
    fn test_build_claude_command_model_with_interactive() {
        let config =
            AgentConfig { model: Some("haiku".to_string()), interactive: true, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", true);
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
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.starts_with("sudo -u deploy"));
        assert!(cmd.contains("--model sonnet"));
    }

    // -- env var injection tests --

    #[test]
    fn test_build_claude_command_with_env_vars() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-ant-test123".to_string());
        let config = AgentConfig { env, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);

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
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);

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
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);

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
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);

        assert!(cmd.contains("GOOD_KEY='ok'"));
        assert!(!cmd.contains("BAD KEY"));
        assert!(!cmd.contains("rm -rf"));
        assert!(!cmd.contains("123STARTS_WITH_DIGIT"));
    }

    #[test]
    fn test_build_claude_command_empty_env_no_prefix() {
        let config = base_config(); // env is empty
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);

        // Must start directly with claude (no env prefix)
        assert!(cmd.starts_with("claude"));
    }

    #[test]
    fn test_build_claude_command_env_with_interactive_mode() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://custom.api.example.com".to_string());
        let config = AgentConfig { interactive: true, env, ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", true);

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
        let cmd1 = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        let cmd2 = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);

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

    // -- system prompt flag tests --

    #[test]
    fn test_build_claude_command_no_system_prompt_flag_when_absent() {
        let config = base_config();
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(!cmd.contains("--system-prompt"), "no system-prompt flag when none configured");
        assert!(!cmd.contains("--append-system-prompt"), "no append flag when none configured");
    }

    #[test]
    fn test_build_claude_command_replace_inline_prompt() {
        let config = AgentConfig {
            system_prompt: Some("You are a Rust expert".to_string()),
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--system-prompt 'You are a Rust expert'"));
        assert!(!cmd.contains("--append-system-prompt"));
        assert!(!cmd.contains("--system-prompt-file"));
    }

    #[test]
    fn test_build_claude_command_replace_prompt_file() {
        let config = AgentConfig {
            system_prompt_file: Some("./.agentd/agents/expert.md".to_string()),
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--system-prompt-file './.agentd/agents/expert.md'"));
        assert!(!cmd.contains("--system-prompt "));
        assert!(!cmd.contains("--append-system-prompt"));
    }

    #[test]
    fn test_build_claude_command_append_inline_prompt() {
        let config = AgentConfig {
            system_prompt: Some("Always use TypeScript".to_string()),
            append_system_prompt: true,
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--append-system-prompt 'Always use TypeScript'"));
        assert!(!cmd.contains("--system-prompt "));
        assert!(!cmd.contains("--system-prompt-file"));
    }

    #[test]
    fn test_build_claude_command_append_prompt_file() {
        let config = AgentConfig {
            system_prompt_file: Some("./style-rules.txt".to_string()),
            append_system_prompt: true,
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--append-system-prompt-file './style-rules.txt'"));
        assert!(!cmd.contains("--system-prompt "));
        assert!(!cmd.contains("--system-prompt-file "));
    }

    #[test]
    fn test_build_claude_command_system_prompt_inline_escaped() {
        // Single quotes in the prompt must be shell-escaped.
        let config =
            AgentConfig { system_prompt: Some("You're an expert".to_string()), ..base_config() };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--system-prompt 'You'\\''re an expert'"));
    }

    #[test]
    fn test_build_claude_command_append_inline_prompt_escaped() {
        let config = AgentConfig {
            system_prompt: Some("Don't skip tests".to_string()),
            append_system_prompt: true,
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--append-system-prompt 'Don'\\''t skip tests'"));
    }

    #[test]
    fn test_build_claude_command_system_prompt_inline_takes_precedence_over_file() {
        // When both are set, system_prompt (inline) takes precedence (append=false).
        let config = AgentConfig {
            system_prompt: Some("inline prompt".to_string()),
            system_prompt_file: Some("./file.md".to_string()),
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--system-prompt 'inline prompt'"));
        assert!(!cmd.contains("--system-prompt-file"));
    }

    #[test]
    fn test_build_claude_command_append_inline_takes_precedence_over_file() {
        // When both are set with append=true, system_prompt (inline) takes precedence.
        let config = AgentConfig {
            system_prompt: Some("inline append".to_string()),
            system_prompt_file: Some("./file.md".to_string()),
            append_system_prompt: true,
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--append-system-prompt 'inline append'"));
        assert!(!cmd.contains("--append-system-prompt-file"));
        assert!(!cmd.contains("--system-prompt-file"));
        assert!(!cmd.contains("--system-prompt "));
    }

    // -- additional_dirs / --add-dir tests --

    #[test]
    fn test_build_claude_command_no_add_dir_when_empty() {
        let config = base_config(); // additional_dirs is empty
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(!cmd.contains("--add-dir"), "no --add-dir flag when additional_dirs is empty");
    }

    #[test]
    fn test_build_claude_command_with_additional_dirs() {
        let config = AgentConfig {
            additional_dirs: vec!["/tmp/project".to_string(), "/home/user/data".to_string()],
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--add-dir '/tmp/project'"), "first dir must appear");
        assert!(cmd.contains("--add-dir '/home/user/data'"), "second dir must appear");
    }

    #[test]
    fn test_build_claude_command_add_dir_shell_escaped() {
        // Path contains spaces and a single quote — must be properly shell-escaped.
        let config = AgentConfig {
            additional_dirs: vec!["/tmp/my project/it's here".to_string()],
            ..base_config()
        };
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        // Single-quote escaping: ' → '\''
        assert!(
            cmd.contains("--add-dir '/tmp/my project/it'\\''s here'"),
            "path with spaces and single quote must be shell-escaped: {cmd}"
        );
    }

    #[test]
    fn test_build_claude_command_pty_backend_no_sdk_url() {
        // Simulates PTY backend: effective_interactive = true
        let config = base_config();
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", true);
        assert!(!cmd.contains("--sdk-url"), "PTY mode must not include --sdk-url");
        assert!(!cmd.contains("--input-format"), "PTY mode must not include --input-format");
        assert!(!cmd.contains("--output-format"), "PTY mode must not include --output-format");
        assert!(cmd.contains("claude"));
    }

    #[test]
    fn test_build_claude_command_sdk_mode_has_sdk_url() {
        // Non-PTY backend: effective_interactive = false
        let config = base_config(); // interactive = false
        let cmd = build_claude_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("--sdk-url"));
        assert!(cmd.contains("--input-format stream-json"));
        assert!(cmd.contains("--output-format stream-json"));
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

    // -----------------------------------------------------------------------
    // inject_pty_prompt tests
    // -----------------------------------------------------------------------

    use crate::storage::AgentStorage;
    use crate::types::{Agent, AgentStatus};
    use crate::websocket::ConnectionRegistry;
    use async_trait::async_trait;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use wrap::backend::{ExecutionBackend, SessionConfig, SessionHealth};
    use wrap::pty_stream::{PtyOutputStream, DEFAULT_CHANNEL_CAPACITY, DEFAULT_HISTORY_BYTES};

    // A writer that captures bytes into a shared Vec for inspection.
    struct CapturingWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // Mock backend that returns a PtyOutputStream for any session.
    struct MockPtyBackend {
        stream: PtyOutputStream,
    }

    impl MockPtyBackend {
        fn new(captured: Arc<Mutex<Vec<u8>>>) -> Self {
            let writer = Box::new(CapturingWriter(captured));
            Self {
                stream: PtyOutputStream::new(
                    DEFAULT_CHANNEL_CAPACITY,
                    DEFAULT_HISTORY_BYTES,
                    writer,
                ),
            }
        }
    }

    #[async_trait]
    impl ExecutionBackend for MockPtyBackend {
        async fn create_session(&self, _config: &SessionConfig) -> anyhow::Result<()> {
            Ok(())
        }
        async fn launch_agent(&self, _config: &SessionConfig) -> anyhow::Result<()> {
            Ok(())
        }
        async fn session_exists(&self, _session_name: &str) -> anyhow::Result<bool> {
            Ok(true)
        }
        async fn kill_session(&self, _session_name: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn send_command(&self, _session_name: &str, _command: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
            Ok(vec![])
        }
        fn prefix(&self) -> &str {
            "mock"
        }
        async fn session_health(&self, _session_name: &str) -> anyhow::Result<SessionHealth> {
            Ok(SessionHealth::Healthy)
        }
        async fn session_output_stream(
            &self,
            _session_name: &str,
        ) -> anyhow::Result<Option<PtyOutputStream>> {
            Ok(Some(self.stream.clone()))
        }
    }

    // Mock backend with no PTY support (default session_output_stream → None).
    struct MockNoPtyBackend;

    #[async_trait]
    impl ExecutionBackend for MockNoPtyBackend {
        async fn create_session(&self, _config: &SessionConfig) -> anyhow::Result<()> {
            Ok(())
        }
        async fn launch_agent(&self, _config: &SessionConfig) -> anyhow::Result<()> {
            Ok(())
        }
        async fn session_exists(&self, _session_name: &str) -> anyhow::Result<bool> {
            Ok(true)
        }
        async fn kill_session(&self, _session_name: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn send_command(&self, _session_name: &str, _command: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
            Ok(vec![])
        }
        fn prefix(&self) -> &str {
            "no-pty"
        }
    }

    async fn make_manager_with_agent(
        backend: Arc<dyn ExecutionBackend>,
        interactive: bool,
    ) -> (AgentManager, Agent, TempDir) {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let storage = Arc::new(AgentStorage::with_path(&db_path).await.unwrap());
        let registry = ConnectionRegistry::new();
        let manager =
            AgentManager::new(storage.clone(), backend, registry, "ws://localhost".into());

        let mut agent =
            Agent::new("test-agent".into(), AgentConfig { interactive, ..base_config() });
        agent.status = AgentStatus::Running;
        agent.session_id = Some("mock-session-1".into());
        storage.add(&agent).await.unwrap();
        (manager, agent, temp)
    }

    #[tokio::test]
    async fn inject_pty_prompt_writes_text_and_newline() {
        let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
        let backend = Arc::new(MockPtyBackend::new(captured.clone()));
        let (manager, agent, _temp) = make_manager_with_agent(backend, /*interactive=*/ true).await;

        manager.inject_pty_prompt(&agent.id, "hello world").await.unwrap();

        let written = captured.lock().unwrap().clone();
        assert_eq!(written, b"hello world\n");
    }

    #[tokio::test]
    async fn inject_pty_prompt_appends_newline_to_multi_line_input() {
        let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
        let backend = Arc::new(MockPtyBackend::new(captured.clone()));
        let (manager, agent, _temp) = make_manager_with_agent(backend, /*interactive=*/ true).await;

        manager.inject_pty_prompt(&agent.id, "line one\nline two").await.unwrap();

        let written = captured.lock().unwrap().clone();
        // A trailing newline is appended; the embedded newline is preserved.
        assert_eq!(written, b"line one\nline two\n");
    }

    #[tokio::test]
    async fn inject_pty_prompt_errors_when_no_pty_stream() {
        let backend = Arc::new(MockNoPtyBackend);
        let (manager, agent, _temp) = make_manager_with_agent(backend, /*interactive=*/ true).await;

        let result = manager.inject_pty_prompt(&agent.id, "hello").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("no PTY stream"), "unexpected error: {msg}");
    }

    #[tokio::test]
    async fn inject_pty_prompt_errors_when_agent_not_found() {
        let backend = Arc::new(MockPtyBackend::new(Arc::new(Mutex::new(vec![]))));
        let (manager, _agent, _temp) =
            make_manager_with_agent(backend, /*interactive=*/ true).await;

        let unknown_id = uuid::Uuid::new_v4();
        let result = manager.inject_pty_prompt(&unknown_id, "hello").await;
        assert!(result.is_err());
    }
}
