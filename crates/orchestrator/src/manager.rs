use crate::scheduler::events::SystemEvent;
use crate::storage::{AgentStorage, ProjectStorage};
use crate::types::{
    Agent, AgentConfig, AgentStatus, AgentUsageStats, ClearContextResponse, RetentionConfig,
};
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
/// session manager (tmux, PTY, subprocess, etc.), making the orchestrator
/// backend-agnostic.
#[derive(Clone)]
pub struct AgentManager {
    storage: Arc<AgentStorage>,
    backend: Arc<dyn ExecutionBackend>,
    registry: ConnectionRegistry,
    /// The base URL agents will use to connect back via WebSocket.
    ws_base_url: String,
    /// Conversation event retention policy applied at terminate and on schedule.
    retention_config: Arc<RetentionConfig>,
    /// Directory holding per-agent MCP config files (`<agent_id>.json`),
    /// written at spawn/restart for agents with `mcp_servers` configured.
    mcp_config_dir: std::path::PathBuf,
}

/// Default directory for per-agent MCP config files: a sibling of the
/// orchestrator database (survives restarts, respects `AGENTD_ENV`).
fn default_mcp_config_dir() -> std::path::PathBuf {
    AgentStorage::get_db_path()
        .ok()
        .and_then(|db| db.parent().map(|p| p.join("mcp")))
        .unwrap_or_else(|| std::env::temp_dir().join("agentd-mcp"))
}

impl AgentManager {
    #[allow(dead_code)]
    pub fn new(
        storage: Arc<AgentStorage>,
        backend: Arc<dyn ExecutionBackend>,
        registry: ConnectionRegistry,
        ws_base_url: String,
    ) -> Self {
        Self::with_retention(
            storage,
            backend,
            registry,
            ws_base_url,
            Arc::new(RetentionConfig::from_env()),
        )
    }

    /// Construct with an explicit [`RetentionConfig`] (useful for testing and
    /// for wiring up the config read from env vars in `main`).
    pub fn with_retention(
        storage: Arc<AgentStorage>,
        backend: Arc<dyn ExecutionBackend>,
        registry: ConnectionRegistry,
        ws_base_url: String,
        retention_config: Arc<RetentionConfig>,
    ) -> Self {
        Self {
            storage,
            backend,
            registry,
            ws_base_url,
            retention_config,
            mcp_config_dir: default_mcp_config_dir(),
        }
    }

    /// Override the MCP config directory (tests pass a `TempDir` path).
    #[allow(dead_code)]
    pub fn with_mcp_config_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.mcp_config_dir = dir;
        self
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
    ///
    /// `organization_id` — when `Some`, the org is written into the initial DB
    /// INSERT so the record is never briefly visible as unscoped.
    pub async fn spawn_agent(
        &self,
        name: String,
        config: AgentConfig,
        built_in: bool,
        organization_id: Option<String>,
    ) -> anyhow::Result<Agent> {
        let mut agent = Agent::new(name, config);
        agent.built_in = built_in;
        agent.organization_id = organization_id;
        let session_name = format!("{}-{}", self.backend.prefix(), agent.id);

        // Persist agent record — organization_id is included in this INSERT.
        self.storage.add(&agent).await?;

        // Create a session in the agent's working directory.
        let session_config = wrap::backend::SessionConfig {
            session_name: session_name.clone(),
            working_dir: agent.config.working_dir.clone(),
            agent_type: agent.config.agent_type.clone(),
            model_provider: "anthropic".into(),
            model_name: agent.config.model.clone().unwrap_or_default(),
            layout: None,
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

        // Subprocess stdio mode takes precedence: agents communicate via
        // stdin/stdout NDJSON rather than WebSocket.
        let use_stdio = self.backend.supports_subprocess_stdio();

        // Determine whether to use interactive / PTY mode.
        // PTY backend always uses PTY stdin (not WebSocket) so that the session
        // remains interactable. Config-level `interactive` flag is also respected.
        // Subprocess stdio mode is mutually exclusive with PTY/interactive mode.
        let effective_interactive =
            !use_stdio && (agent.config.interactive || self.backend.supports_pty_input());

        // Persist the effective interactive state so downstream consumers
        // (API, UI) see the correct mode.  The DB record is the single source
        // of truth; without this, PTY-backend agents are misidentified as SDK
        // mode because `agent.config.interactive` stays `false` in storage.
        if effective_interactive && !agent.config.interactive {
            agent.config.interactive = true;
        }

        // Build the launch command. Interactive PTY mode launches the native
        // agent directly (a human types in the terminal — out of AAP scope).
        // Every other mode launches the AAP adapter for the configured
        // agent_type; config flows to the adapter via the AAP `initialize`
        // message, and the transport env selects stdio vs websocket.
        let launch_cmd = if effective_interactive {
            let mcp_config_path = self.write_mcp_config(&agent)?;
            build_interactive_command(&agent.config, mcp_config_path.as_deref())
        } else {
            let ws_url = self
                .backend
                .agent_ws_url(&session_name, Some(&session_config))
                .unwrap_or_else(|| format!("{}/ws/{}", self.ws_base_url, agent.id));
            build_adapter_command(&agent.config, &ws_url, use_stdio)
        };

        // Persist the launch command so the UI can display it for debugging.
        agent.launch_command = Some(launch_cmd.clone());

        // Send the command into the session.
        if let Err(e) = self.backend.send_command(&session_name, &launch_cmd).await {
            let _ = self.backend.kill_session(&session_name).await;
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to launch agent adapter in session: {}", e));
        }

        // Mark as running and persist the PID for reconciliation.
        agent.status = AgentStatus::Running;
        agent.session_id = Some(session_name.clone());
        agent.pid = self.backend.session_pid(&session_name).await.unwrap_or(None);
        agent.updated_at = Utc::now();
        self.storage.update(&agent).await?;

        // Register the agent's tool policy with the WebSocket registry.
        self.registry.set_policy(agent.id, agent.config.tool_policy.clone()).await;

        // For subprocess stdio mode, wire stdin/stdout IO immediately after spawn.
        // The ConnectionRegistry is populated right away (no WebSocket handshake needed).
        if use_stdio {
            self.wire_subprocess_io(agent.id, &agent.config, &session_name).await?;
        }

        info!(
            agent_id = %agent.id,
            session = %session_name,
            pid = ?agent.pid,
            "Agent spawned"
        );

        // If there's an initial prompt, deliver it via the appropriate channel.
        //
        // Subprocess stdio mode: registered immediately, send without waiting.
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

            if use_stdio {
                // Stdio mode — the adapter is registered immediately, but the
                // prompt must wait for its AAP `ready` handshake. Spawn so the
                // spawn path returns promptly.
                let registry = self.registry.clone();
                tokio::spawn(async move {
                    if !registry.wait_for_ready(&agent_id, Duration::from_secs(30)).await {
                        warn!(%agent_id, "Adapter never sent AAP ready; sending initial prompt anyway");
                    }
                    if let Err(e) = registry.send_user_message(&agent_id, &prompt).await {
                        warn!(%agent_id, %e, "Failed to send initial prompt via subprocess stdin");
                    } else {
                        info!(%agent_id, "Initial prompt sent via subprocess stdin");
                    }
                });
            } else if effective_interactive {
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
                // Websocket (SDK) mode — the adapter dials back and sends AAP
                // `ready` once it has received `initialize`. Wait for readiness
                // (which implies the connection), then send the prompt.
                let registry = self.registry.clone();
                tokio::spawn(async move {
                    if registry.wait_for_ready(&agent_id, Duration::from_secs(60)).await {
                        match registry.send_user_message(&agent_id, &prompt).await {
                            Ok(_) => info!(%agent_id, "Initial prompt sent via WebSocket"),
                            Err(e) => warn!(%agent_id, %e, "Failed to send initial prompt"),
                        }
                    } else {
                        warn!(%agent_id, "Adapter never became ready, initial prompt not sent");
                    }
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

        // Optionally delete all conversation events before removing the record.
        if self.retention_config.cleanup_on_terminate {
            match self.storage.delete_conversation_events_for_agent(*id).await {
                Ok(n) => {
                    info!(agent_id = %id, deleted = n, "Conversation events deleted on terminate")
                }
                Err(e) => {
                    warn!(agent_id = %id, %e, "Failed to clean up conversation events on terminate")
                }
            }
        }

        // Remove the record from storage entirely so the name can be reused.
        self.storage.delete(id).await?;
        self.remove_mcp_config(id);

        // Set status on the returned value for callers that inspect it.
        agent.status = AgentStatus::Stopped;
        agent.updated_at = Utc::now();

        info!(agent_id = %id, "Agent terminated and record deleted");

        Ok(agent)
    }

    /// Write the agent's MCP server map to its per-agent config file.
    ///
    /// Returns the file path to pass via `--mcp-config`, or `None` when the
    /// agent has no `mcp_servers` configured. The file is created with mode
    /// 0600 — MCP server env vars can carry secrets.
    fn write_mcp_config(&self, agent: &Agent) -> anyhow::Result<Option<String>> {
        let Some(servers) = agent.config.mcp_servers.as_ref().filter(|s| !s.is_empty()) else {
            // Config removed since last launch — drop any stale file.
            self.remove_mcp_config(&agent.id);
            return Ok(None);
        };

        std::fs::create_dir_all(&self.mcp_config_dir)?;
        let path = self.mcp_config_dir.join(format!("{}.json", agent.id));
        let body = serde_json::to_vec_pretty(&serde_json::json!({ "mcpServers": servers }))?;
        std::fs::write(&path, body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(Some(path.to_string_lossy().into_owned()))
    }

    /// Best-effort removal of an agent's MCP config file.
    fn remove_mcp_config(&self, agent_id: &Uuid) {
        let path = self.mcp_config_dir.join(format!("{agent_id}.json"));
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                warn!(%agent_id, %e, "Failed to remove MCP config file");
            }
        }
    }

    /// Bootstrap built-in system agents at orchestrator startup.
    ///
    /// Called once after [`reconcile`](Self::reconcile) so that reconciliation
    /// has already handled any surviving user agents before we insert/restart
    /// system agents.
    ///
    /// For each definition in [`builtin_agent_defs`](crate::system_agents::builtin_agent_defs):
    /// 1. **Missing** — eager defs are spawned via `spawn_agent()`; lazy defs
    ///    get a dormant `Pending` record and are spawned on first message
    ///    (see [`ensure_builtin_running`](Self::ensure_builtin_running)).
    /// 2. **Config drift** — when the stored config no longer matches the
    ///    definition (new prompt, policy, model, ... in this release), the
    ///    stored config is refreshed *before* any restart so a failed restart
    ///    still retries with the new config on the next boot.  Running agents
    ///    are restarted to pick the refresh up; dormant agents pick it up on
    ///    their next spawn.
    /// 3. **Stopped/failed eager agents** — restarted (lazy agents are left
    ///    dormant).
    ///
    /// Stored built-ins whose name is no longer in the registry are removed
    /// as orphans — built-ins cannot be deleted via the API, so leaving them
    /// would create undeletable zombies.
    ///
    /// Per-definition errors are logged and never abort the loop.
    pub async fn bootstrap_system_agents(&self) -> anyhow::Result<()> {
        use crate::system_agents::{builtin_agent_defs, config_drifted, refreshed_config};

        let defs = builtin_agent_defs();
        let existing = self.storage.list_system_agents().await?;

        for def in &defs {
            let stored = existing.iter().find(|a| a.name == def.name);
            let fresh = def.build_config();

            match stored {
                None if def.lazy => {
                    info!("Bootstrapping dormant system agent '{}'", def.name);
                    if let Err(e) = self.create_dormant_agent(def.name.to_string(), fresh).await {
                        error!(%e, "Failed to create dormant system agent '{}'", def.name);
                    }
                }
                None => {
                    info!("Bootstrapping system agent '{}'", def.name);
                    match self.spawn_agent(def.name.to_string(), fresh, true, None).await {
                        Ok(agent) => {
                            info!(agent_id = %agent.id, "System agent '{}' spawned", def.name)
                        }
                        Err(e) => error!(%e, "Failed to spawn system agent '{}'", def.name),
                    }
                }
                Some(agent) => {
                    let mut agent = agent.clone();
                    let drifted = config_drifted(&agent.config, &fresh);

                    if drifted {
                        // Persist the refreshed config BEFORE restarting so a
                        // failed restart still retries with the new config.
                        info!(
                            agent_id = %agent.id,
                            "System agent '{}' definition drifted, refreshing config",
                            def.name
                        );
                        agent.config = refreshed_config(&agent.config, fresh);
                        agent.updated_at = Utc::now();
                        if let Err(e) = self.storage.update(&agent).await {
                            error!(
                                agent_id = %agent.id,
                                %e,
                                "Failed to persist refreshed config for '{}'",
                                def.name
                            );
                            continue;
                        }
                    }

                    let should_restart = if drifted {
                        // Running agents must restart to pick up the refresh;
                        // dormant/stopped lazy agents pick it up on next spawn.
                        agent.status == AgentStatus::Running || !def.lazy
                    } else {
                        // No drift: keep eager agents alive (current
                        // behavior); leave lazy agents dormant.
                        agent.status != AgentStatus::Running && !def.lazy
                    };

                    if should_restart {
                        info!(
                            agent_id = %agent.id,
                            status = %agent.status,
                            drifted,
                            "Restarting system agent '{}'",
                            def.name
                        );
                        if let Err(e) = self.restart_agent(&agent).await {
                            error!(
                                agent_id = %agent.id,
                                %e,
                                "Failed to restart system agent '{}'",
                                def.name
                            );
                        }
                    } else if agent.status == AgentStatus::Running {
                        info!(
                            agent_id = %agent.id,
                            "System agent '{}' is already running, skipping bootstrap",
                            def.name
                        );
                    }
                }
            }
        }

        // Remove orphaned built-ins: rows whose name left the registry.
        for agent in existing.iter().filter(|a| !defs.iter().any(|d| d.name == a.name)) {
            info!(
                agent_id = %agent.id,
                name = %agent.name,
                "Removing orphaned built-in agent (no longer in the registry)"
            );
            if let Err(e) = self.remove_built_in_agent(agent).await {
                error!(agent_id = %agent.id, %e, "Failed to remove orphaned built-in agent");
            }
        }

        Ok(())
    }

    /// Create a dormant built-in agent: DB record only, no backend session.
    ///
    /// The record is created with status `Pending` and `built_in = true`.
    /// Reconciliation ignores `Pending` agents, so the record stays dormant
    /// until [`ensure_builtin_running`](Self::ensure_builtin_running) spawns
    /// it on first message.
    async fn create_dormant_agent(
        &self,
        name: String,
        config: AgentConfig,
    ) -> anyhow::Result<Agent> {
        let mut agent = Agent::new(name, config);
        agent.built_in = true;
        self.storage.add(&agent).await?;
        info!(agent_id = %agent.id, name = %agent.name, "Dormant system agent record created");
        Ok(agent)
    }

    /// Remove a built-in agent: kill any session and delete the DB record.
    ///
    /// `terminate_agent` deliberately bails on `built_in` agents, so orphan
    /// cleanup needs this private path.  Only ever called with rows returned
    /// by `list_system_agents()` (`built_in = true`), never for user agents.
    async fn remove_built_in_agent(&self, agent: &Agent) -> anyhow::Result<()> {
        if let Some(ref session) = agent.session_id {
            if let Err(e) = self.backend.kill_session(session).await {
                warn!(agent_id = %agent.id, %e, "Failed to kill session for orphaned built-in");
            }
        }
        self.registry.unregister(&agent.id).await;
        self.storage.delete(&agent.id).await?;
        self.remove_mcp_config(&agent.id);
        Ok(())
    }

    /// Ensure a built-in agent has a live session, spawning it if dormant.
    ///
    /// Used by the message-delivery path to wake lazy system agents on first
    /// contact.  Returns the (possibly freshly spawned) agent record.  For
    /// non-built-in agents this returns the record unchanged — waking user
    /// agents is not this method's business.
    pub async fn ensure_builtin_running(&self, id: &Uuid) -> anyhow::Result<Agent> {
        let agent =
            self.storage.get(id).await?.ok_or_else(|| anyhow::anyhow!("Agent not found"))?;

        if !agent.built_in || agent.status == AgentStatus::Running {
            return Ok(agent);
        }

        info!(
            agent_id = %agent.id,
            name = %agent.name,
            status = %agent.status,
            "Waking dormant system agent"
        );
        self.restart_agent(&agent).await
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
                    // happens after a service restart.
                    //
                    // Subprocess stdio agents cannot reconnect after a service
                    // restart — the pipe pair is gone. Always restart them.
                    //
                    // For other backends the process may still be alive in its
                    // own process group (setpgid). Check the stored PID before
                    // spawning a duplicate.
                    if !self.backend.supports_subprocess_stdio() {
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
    #[allow(dead_code)]
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

    /// Like [`list_agents_paginated`] but also scopes to `org_id` when present.
    pub async fn list_agents_paginated_org(
        &self,
        status: Option<AgentStatus>,
        built_in_filter: Option<bool>,
        project_id: Option<Uuid>,
        org_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<(Vec<Agent>, usize)> {
        self.storage
            .list_paginated_org(status, built_in_filter, project_id, org_id, limit, offset)
            .await
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

    /// Persist an already-merged agent config and optionally restart the
    /// process so launch-affecting changes take effect.
    ///
    /// Pushes the tool policy to the live WebSocket registry (policies apply
    /// without a restart). When `restart` is true the agent is relaunched in
    /// any status — a stopped or failed agent comes back up with the new
    /// config, matching `POST /agents/{id}/restart` semantics. If the restart
    /// fails the config is still persisted and the agent is marked Failed.
    pub async fn update_agent_and_maybe_restart(
        &self,
        mut agent: Agent,
        restart: bool,
    ) -> anyhow::Result<Agent> {
        self.storage.update(&agent).await?;

        // Apply the tool policy to the live connection immediately.
        self.registry.set_policy(agent.id, agent.config.tool_policy.clone()).await;

        if restart {
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
    /// backend does not support PTY streaming (e.g., tmux backends).
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
    /// - The backend does not expose a PTY stream (tmux backend).
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
    /// No-ops silently for backends that do not support resize (tmux).
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
            agent_type: agent.config.agent_type.clone(),
            model_provider: "anthropic".into(),
            model_name: agent.config.model.clone().unwrap_or_default(),
            layout: None,
        };

        if let Err(e) = self.backend.create_session(&session_config).await {
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to create session on restart: {}", e));
        }

        // Build and send the claude command with the updated config.
        let use_stdio = self.backend.supports_subprocess_stdio();
        let effective_interactive =
            !use_stdio && (agent.config.interactive || self.backend.supports_pty_input());

        // Persist the effective interactive state (mirrors the fix in spawn_agent).
        if effective_interactive && !agent.config.interactive {
            agent.config.interactive = true;
        }

        let launch_cmd = if effective_interactive {
            // Always rewrite the MCP config — a PATCH may have changed servers
            // since the last launch.
            let mcp_config_path = self.write_mcp_config(&agent)?;
            build_interactive_command(&agent.config, mcp_config_path.as_deref())
        } else {
            let ws_url = self
                .backend
                .agent_ws_url(&session_name, Some(&session_config))
                .unwrap_or_else(|| format!("{}/ws/{}", self.ws_base_url, agent.id));
            build_adapter_command(&agent.config, &ws_url, use_stdio)
        };

        // Persist the launch command so the UI can display it for debugging.
        agent.launch_command = Some(launch_cmd.clone());

        if let Err(e) = self.backend.send_command(&session_name, &launch_cmd).await {
            let _ = self.backend.kill_session(&session_name).await;
            agent.status = AgentStatus::Failed;
            agent.updated_at = Utc::now();
            let _ = self.storage.update(&agent).await;
            return Err(anyhow::anyhow!("Failed to launch agent adapter on restart: {}", e));
        }

        // Update state.
        agent.status = AgentStatus::Running;
        agent.session_id = Some(session_name.clone());
        agent.pid = self.backend.session_pid(&session_name).await.unwrap_or(None);
        agent.updated_at = Utc::now();
        self.storage.update(&agent).await?;

        // Re-register tool policy.
        self.registry.set_policy(agent.id, agent.config.tool_policy.clone()).await;

        // Re-wire subprocess IO if needed.
        if use_stdio {
            self.wire_subprocess_io(agent.id, &agent.config, &session_name).await?;
        }

        // Publish AgentRestarted so the scheduler can re-launch dead workflow runners.
        if let Some(bus) = self.registry.event_bus() {
            bus.publish(SystemEvent::AgentRestarted { agent_id: agent.id });
        }

        info!(
            agent_id = %agent.id,
            session = %session_name,
            pid = ?agent.pid,
            model = ?agent.config.model,
            "Agent restarted"
        );

        Ok(agent)
    }

    /// Wire subprocess stdin/stdout IO for an agent running in stdio mode.
    ///
    /// - Takes the stdout reader from the backend and spawns a task that reads
    ///   NDJSON lines and calls `handle_incoming_message`.
    /// - Creates an mpsc channel, registers it in the ConnectionRegistry, and
    ///   spawns a relay task that forwards channel messages to subprocess stdin.
    ///
    /// Must be called immediately after `send_command` succeeds for subprocess
    /// backends. The agent is registered in the registry upon return, so
    /// `send_user_message` works without a wait loop.
    async fn wire_subprocess_io(
        &self,
        agent_id: Uuid,
        config: &AgentConfig,
        session_name: &str,
    ) -> anyhow::Result<()> {
        use crate::websocket::{aap_initialize_line, handle_incoming_message, AgentConnection};
        use tokio::io::{AsyncBufReadExt, BufReader};
        use tokio::sync::mpsc;

        let stdout_reader = self.backend.take_subprocess_stdout(session_name).await?;

        // mpsc channel bridges ConnectionRegistry → subprocess stdin relay.
        let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<String>();

        // AAP handshake: the adapter needs the `initialize` message (carrying
        // all agent configuration) before it will accept prompts. Queue it
        // first so it precedes any prompt; the relay task delivers it on stdin.
        let _ = ws_tx.send(aap_initialize_line(config));

        self.registry.register(agent_id, AgentConnection { tx: ws_tx }).await;

        // Stdin relay task: forwards registry messages → subprocess stdin.
        let backend = self.backend.clone();
        let session_for_relay = session_name.to_string();
        tokio::spawn(async move {
            while let Some(msg) = ws_rx.recv().await {
                if let Err(e) = backend.write_subprocess_stdin(&session_for_relay, &msg).await {
                    warn!(
                        agent_id = %agent_id,
                        %e,
                        "Subprocess stdin relay error"
                    );
                    break;
                }
            }
        });

        // Stdout reader task: NDJSON lines → handle_incoming_message.
        let registry = self.registry.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout_reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                handle_incoming_message(&agent_id, &line, &registry).await;
            }
            registry.unregister(&agent_id).await;
            info!(agent_id = %agent_id, "Subprocess stdout closed, agent unregistered");
        });

        Ok(())
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

/// Wrap a base command string with agent env-var assignments and optional
/// `sudo -u` privilege drop.
///
/// The `env -u SUDO_*` prefix under sudo unsets the SUDO_* variables (Claude
/// Code — spawned by the adapter — silently ignores ANTHROPIC_AUTH_TOKEN /
/// ANTHROPIC_BASE_URL when any SUDO_* variable is present) and carries the
/// agent env across the privilege boundary regardless of sudoers env_keep.
fn wrap_with_env_and_sudo(base: &str, env_assignments: &[String], user: Option<&str>) -> String {
    match user {
        Some(user) => {
            let unset = "-u SUDO_USER -u SUDO_UID -u SUDO_GID -u SUDO_COMMAND";
            if env_assignments.is_empty() {
                format!("sudo -u {} env {} {}", user, unset, base)
            } else {
                format!("sudo -u {} env {} {} {}", user, unset, env_assignments.join(" "), base)
            }
        }
        None => {
            if env_assignments.is_empty() {
                base.to_string()
            } else {
                // Prefix the command with shell variable assignments scoped to
                // the invocation.
                format!("{} {}", env_assignments.join(" "), base)
            }
        }
    }
}

/// Build the launch command for an AAP adapter process.
///
/// The adapter binary is resolved from the agent's `agent_type`. Agent
/// configuration is NOT passed as flags — it travels in the AAP `initialize`
/// message ([`crate::websocket::aap_initialize_line`]). Only the AAP transport
/// is selected here, via environment: stdio backends use the stdin/stdout
/// binding; the tmux backend uses the websocket binding and the adapter
/// dials back to `ws_url`.
fn build_adapter_command(config: &AgentConfig, ws_url: &str, use_stdio: bool) -> String {
    let program = crate::adapter::resolve_adapter_program(&config.agent_type);
    let base = shell_escape_value(&program);

    // Merge the agent env with the AAP transport selection so both cross the
    // sudo boundary together.
    let mut env = config.env.clone();
    if use_stdio {
        env.insert(
            agentd_agent_protocol::ENV_TRANSPORT.to_string(),
            agentd_agent_protocol::TRANSPORT_STDIO.to_string(),
        );
    } else {
        env.insert(
            agentd_agent_protocol::ENV_TRANSPORT.to_string(),
            agentd_agent_protocol::TRANSPORT_WEBSOCKET.to_string(),
        );
        env.insert(agentd_agent_protocol::ENV_WS_URL.to_string(), ws_url.to_string());
    }

    let env_assignments = build_env_assignments(&env);
    wrap_with_env_and_sudo(&base, &env_assignments, config.user.as_deref())
}

/// Build the launch command for interactive PTY mode.
///
/// Interactive mode runs the native `claude` CLI directly so a human can type
/// in the terminal. It is outside the AAP path (no adapter, no protocol) and
/// is currently claude-specific; a non-claude `agent_type` in interactive mode
/// still launches `claude`.
fn build_interactive_command(config: &AgentConfig, mcp_config_path: Option<&str>) -> String {
    let mut args = vec!["claude".to_string()];

    if let Some(ref model) = config.model {
        args.push(format!("--model {}", model));
    }
    if config.worktree {
        args.push("--worktree".to_string());
    }
    for dir in &config.additional_dirs {
        args.push(format!("--add-dir {}", shell_escape_value(dir)));
    }
    if let Some(path) = mcp_config_path {
        args.push(format!("--mcp-config {}", shell_escape_value(path)));
        args.push("--strict-mcp-config".to_string());
    }
    // System prompt flags — four combinations based on (file vs inline) × (replace vs append).
    match (config.append_system_prompt, &config.system_prompt, &config.system_prompt_file) {
        (false, Some(prompt), _) => {
            args.push(format!("--system-prompt {}", shell_escape_value(prompt)))
        }
        (false, None, Some(path)) => {
            args.push(format!("--system-prompt-file {}", shell_escape_value(path)))
        }
        (true, Some(prompt), _) => {
            args.push(format!("--append-system-prompt {}", shell_escape_value(prompt)))
        }
        (true, None, Some(path)) => {
            args.push(format!("--append-system-prompt-file {}", shell_escape_value(path)))
        }
        (_, None, None) => {}
    }

    let base = args.join(" ");
    let env_assignments = build_env_assignments(&config.env);
    wrap_with_env_and_sudo(&base, &env_assignments, config.user.as_deref())
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
            additional_dirs: vec![],
            rooms: vec![],
            mcp_servers: None,
            agent_type: "claude".to_string(),
        }
    }

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

    // -- build_interactive_command tests (interactive PTY mode, claude-specific) --

    #[test]
    fn interactive_command_launches_claude_without_stream_flags() {
        let config = base_config();
        let cmd = build_interactive_command(&config, None);
        assert!(cmd.contains("claude"));
        assert!(!cmd.contains("--sdk-url"));
        assert!(!cmd.contains("stream-json"));
        assert!(!cmd.contains("--print"));
    }

    #[test]
    fn interactive_command_includes_model_worktree_dirs_and_mcp() {
        let config = AgentConfig {
            model: Some("opus".to_string()),
            worktree: true,
            additional_dirs: vec!["/data".to_string()],
            ..base_config()
        };
        let cmd = build_interactive_command(&config, Some("/tmp/mcp.json"));
        assert!(cmd.contains("--model opus"));
        assert!(cmd.contains("--worktree"));
        assert!(cmd.contains("--add-dir '/data'"));
        assert!(cmd.contains("--mcp-config '/tmp/mcp.json'"));
        assert!(cmd.contains("--strict-mcp-config"));
    }

    #[test]
    fn interactive_command_system_prompt_variants() {
        let replace = AgentConfig { system_prompt: Some("hi".to_string()), ..base_config() };
        assert!(build_interactive_command(&replace, None).contains("--system-prompt 'hi'"));
        let append = AgentConfig {
            system_prompt: Some("hi".to_string()),
            append_system_prompt: true,
            ..base_config()
        };
        assert!(build_interactive_command(&append, None).contains("--append-system-prompt 'hi'"));
    }

    // -- build_adapter_command tests (AAP path) --

    #[test]
    fn adapter_command_stdio_sets_transport_env_and_no_flags() {
        let config = base_config(); // agent_type = "claude"
        let cmd = build_adapter_command(&config, "ws://localhost:7006/ws/abc", true);
        assert!(cmd.contains("agentd-adapter-claude"));
        assert!(cmd.contains("AGENTD_AAP_TRANSPORT='stdio'"));
        assert!(!cmd.contains("AGENTD_AAP_WS_URL"));
        // Agent config is delivered via the AAP initialize message, not flags.
        assert!(!cmd.contains("--model"));
        assert!(!cmd.contains("stream-json"));
    }

    #[test]
    fn adapter_command_websocket_sets_transport_and_url() {
        let config = base_config();
        let cmd = build_adapter_command(&config, "ws://localhost:7006/ws/abc", false);
        assert!(cmd.contains("AGENTD_AAP_TRANSPORT='websocket'"));
        assert!(cmd.contains("AGENTD_AAP_WS_URL='ws://localhost:7006/ws/abc'"));
    }

    #[test]
    fn adapter_command_resolves_agent_type() {
        let config = AgentConfig { agent_type: "gemini".to_string(), ..base_config() };
        let cmd = build_adapter_command(&config, "ws://x", true);
        assert!(cmd.contains("agentd-adapter-gemini"));
    }

    #[test]
    fn adapter_command_sudo_wraps_and_strips_sudo_env() {
        let config = AgentConfig { user: Some("deploy".to_string()), ..base_config() };
        let cmd = build_adapter_command(&config, "ws://x", true);
        assert!(cmd.starts_with("sudo -u deploy env -u SUDO_USER"));
        assert!(cmd.contains("AGENTD_AAP_TRANSPORT='stdio'"));
    }

    #[test]
    fn adapter_command_includes_agent_env() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "secret".to_string());
        let config = AgentConfig { env, ..base_config() };
        let cmd = build_adapter_command(&config, "ws://x", true);
        assert!(cmd.contains("ANTHROPIC_AUTH_TOKEN='secret'"));
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
