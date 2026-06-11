//! Built-in system agent definitions.
//!
//! This module defines the programmatically-managed system agents that the
//! orchestrator manages automatically at startup.  System agents are always
//! present while the service is running and cannot be deleted via the
//! user-facing API.
//!
//! # Design
//!
//! Each built-in agent is described by a [`SystemAgentDef`] in the
//! [`builtin_agent_defs`] registry.  At startup the orchestrator iterates the
//! registry: eager agents are spawned immediately, lazy agents get a dormant
//! database record and are spawned on first message.  When a definition
//! changes between releases (prompt, policy, model, ...), the stored config is
//! refreshed and the agent restarted so deployments pick up the new
//! definition — see [`config_drifted`].
//!
//! The system prompts and tool policies defined here are embedded directly as
//! Rust string literals so that they are version-controlled with the code and
//! require no files on disk.

use crate::types::{AgentConfig, ToolPolicy};
use std::collections::HashMap;

/// Name of the primary built-in system agent.
pub const SYSTEM_AGENT_NAME: &str = "agentd-system";

/// How a built-in agent's `working_dir` is derived at bootstrap time.
#[derive(Debug, Clone, Copy)]
pub enum WorkingDirStrategy {
    /// The orchestrator process's current working directory.
    ///
    /// The value is captured when the agent record is first created and is
    /// deliberately excluded from drift detection (see [`refreshed_config`]):
    /// the orchestrator may be launched from a different directory on a later
    /// boot (launchd vs. manual) and that must not restart-loop the agent.
    CurrentDir,
}

/// Declarative definition of one built-in system agent.
///
/// `prompt` and `tool_policy` are function pointers (not values) so that each
/// bootstrap evaluates them fresh — they may incorporate the crate version,
/// loaded service configuration, or environment gates, all of which
/// participate in drift detection.
pub struct SystemAgentDef {
    /// Unique agent name; doubles as the registry identity for drift and
    /// orphan detection.
    pub name: &'static str,
    /// Model alias passed to the Claude session.
    pub model: &'static str,
    /// Communicate rooms auto-joined on connect.
    pub rooms: &'static [&'static str],
    /// How `working_dir` is derived when the record is first created.
    pub working_dir: WorkingDirStrategy,
    /// When `true`, bootstrap creates a dormant record only; the session is
    /// spawned on the first message delivered to the agent.
    pub lazy: bool,
    /// Builds the full system prompt.
    pub prompt: fn() -> String,
    /// Builds the agent's tool policy.
    pub tool_policy: fn() -> ToolPolicy,
}

impl SystemAgentDef {
    /// Assemble a fresh [`AgentConfig`] from this definition.
    pub fn build_config(&self) -> AgentConfig {
        let working_dir = match self.working_dir {
            WorkingDirStrategy::CurrentDir => std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "/".to_string()),
        };

        AgentConfig {
            working_dir,
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: Some((self.prompt)()),
            system_prompt_file: None,
            append_system_prompt: false,
            tool_policy: (self.tool_policy)(),
            model: Some(self.model.to_string()),
            env: HashMap::new(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: self.rooms.iter().map(|r| r.to_string()).collect(),
            mcp_servers: None,
        }
    }
}

/// The registry of all built-in system agents.
///
/// Bootstrap iterates this list: missing agents are created, drifted configs
/// are refreshed, and stored built-ins whose name is no longer listed here
/// are removed as orphans.
pub fn builtin_agent_defs() -> Vec<SystemAgentDef> {
    vec![system_agent_def()]
}

/// Definition of the `agentd-system` domain-expert agent.
///
/// # Tool policy rationale
///
/// The system agent uses an `AllowList` with read-only tools:
/// - `Read`, `Grep`, `Glob`, `LS` — file and code inspection
/// - `Bash(curl *)` — HTTP health checks
/// - `Bash(git log/status/diff *)` — read-only VCS state inspection
/// - `Bash(cargo clippy/test/build *)` — compile-time checks
/// - `Bash(agent *)` — CLI interactions with agentd services
///
/// Notably absent: `Write`, `Edit`, `NotebookEdit`, unrestricted `Bash`.
/// This prevents the system agent from modifying source code, configuration
/// files, or agent state — it can explain and diagnose but not act.
fn system_agent_def() -> SystemAgentDef {
    SystemAgentDef {
        name: SYSTEM_AGENT_NAME,
        model: "sonnet",
        rooms: &["system"],
        working_dir: WorkingDirStrategy::CurrentDir,
        lazy: false,
        prompt: system_agent_prompt,
        tool_policy: system_agent_tool_policy,
    }
}

/// Full system prompt for `agentd-system`: version line + rendered body.
fn system_agent_prompt() -> String {
    // Ports come from the deployment's actually-loaded configuration so the
    // prompt is correct per-deployment (env vars / TOML override defaults).
    // A config change is a drift, so port changes roll out automatically.
    let services = agentd_common::config::load().map(|c| c.services).unwrap_or_default();
    system_agent_prompt_with(&services)
}

/// Render the system prompt against an explicit [`ServicesConfig`].
///
/// Split out from [`system_agent_prompt`] so tests can inject non-default
/// ports and prove the prompt is generated, not hardcoded.
fn system_agent_prompt_with(services: &agentd_common::config::ServicesConfig) -> String {
    // Include version/build metadata in the prompt so the agent can report
    // it.  The version line also makes every release a config drift, which
    // is how new prompt text reaches existing deployments.
    let version = env!("CARGO_PKG_VERSION");
    let body = SYSTEM_AGENT_PROMPT_TEMPLATE
        .replace("$SERVICE_TABLE", &render_service_table(services))
        .replace("$ORCHESTRATOR_PORT", &services.orchestrator.port.to_string())
        .replace("$WRAP_PORT", &services.wrap.port.to_string());
    format!("agentd version: {version}\n\n{body}")
}

/// Render the service-inventory table from the loaded configuration.
///
/// Generated rather than hand-written: the previous hardcoded table had
/// drifted from the real defaults (wrong notify/wrap ports, missing
/// ask/hook/monitor, core/communicate collision).
fn render_service_table(services: &agentd_common::config::ServicesConfig) -> String {
    let rows: [(&str, u16, &str); 10] = [
        ("core", services.core.port, "Auth gateway, API proxy"),
        ("ask", services.ask.port, "Approval requests, human-in-the-loop gates"),
        ("hook", services.hook.port, "Shell-event capture and forwarding"),
        ("monitor", services.monitor.port, "System health metrics and thresholds"),
        ("notify", services.notify.port, "Notification routing and delivery"),
        ("wrap", services.wrap.port, "tmux/Docker execution backend"),
        ("orchestrator", services.orchestrator.port, "Agent lifecycle, WebSocket SDK, policies"),
        ("memory", services.memory.port, "Semantic vector memory store"),
        ("ui", services.ui.port, "Web UI server"),
        ("communicate", services.communicate.port, "Room-based messaging (WebSocket + REST)"),
    ];

    let mut table = String::from(
        "| Service       | Port  | Description                              |\n\
         |---------------|-------|------------------------------------------|\n",
    );
    for (name, port, description) in rows {
        table.push_str(&format!("| {name:<13} | {port:<5} | {description:<40} |\n"));
    }
    table.push_str("| mcp           | —     | MCP server (stdio transport, no port)    |\n");
    table
}

/// Carry runtime-derived fields from a stored config over to a freshly built
/// one, producing the config a drifted agent should be updated to.
///
/// Two fields are environment- or runtime-derived rather than definitional
/// and must never count as drift:
/// - `working_dir` — captured from the orchestrator cwd at first creation
///   (see [`WorkingDirStrategy::CurrentDir`]).
/// - `interactive` — `spawn_agent` persists the *effective* interactive mode,
///   which PTY-capable backends force to `true`.  Comparing it against the
///   definition's `false` would restart-loop the agent on every boot.
pub fn refreshed_config(stored: &AgentConfig, mut fresh: AgentConfig) -> AgentConfig {
    fresh.working_dir = stored.working_dir.clone();
    fresh.interactive = stored.interactive;
    fresh
}

/// Whether a stored built-in agent config has drifted from its definition.
///
/// Compares via `serde_json::Value` so map key order is irrelevant and no
/// `PartialEq` impls need to ripple through `wrap` types.  Runtime-derived
/// fields are normalized out by [`refreshed_config`] first.
pub fn config_drifted(stored: &AgentConfig, fresh: &AgentConfig) -> bool {
    let normalized = refreshed_config(stored, fresh.clone());
    let lhs = serde_json::to_value(&normalized).expect("AgentConfig serializes");
    let rhs = serde_json::to_value(stored).expect("AgentConfig serializes");
    lhs != rhs
}

/// Restrictive tool policy for the system agent.
///
/// Uses an `AllowList` of read-only tools.  Absent tools are denied automatically:
/// - `Write`, `Edit`, `NotebookEdit` — no file modifications
/// - Unrestricted `Bash` — only specific safe subcommands are permitted
/// - `TodoWrite` — no task-list manipulation
///
/// The allowlist entries map directly to the documented policy in
/// [`SYSTEM_AGENT_PROMPT`].  Update both in tandem if the policy changes.
fn system_agent_tool_policy() -> ToolPolicy {
    ToolPolicy::AllowList {
        tools: vec![
            // ── File and code navigation ──────────────────────────────────
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
            "LS".to_string(),
            // ── Safe bash subcommands (read-only) ─────────────────────────
            // HTTP health checks and API reads
            "Bash(curl *)".to_string(),
            // Basic filesystem inspection
            "Bash(cat *)".to_string(),
            "Bash(ls *)".to_string(),
            // Process inspection
            "Bash(ps *)".to_string(),
            "Bash(echo *)".to_string(),
            // Read-only VCS state
            "Bash(git log *)".to_string(),
            "Bash(git status*)".to_string(),
            "Bash(git diff *)".to_string(),
            // Rust build and analysis (read-only; no publish/install)
            "Bash(cargo clippy *)".to_string(),
            "Bash(cargo test *)".to_string(),
            "Bash(cargo build *)".to_string(),
            // agentd CLI — list, get, search, message (no destructive ops)
            "Bash(agent *)".to_string(),
        ],
        sandbox_bypass: vec![],
    }
}

/// System prompt template for the agentd-system agent.
///
/// Embedded as a compile-time constant so it is version-controlled with the
/// code and requires no files on disk.  Rendered by
/// [`system_agent_prompt_with`], which substitutes:
/// - `$SERVICE_TABLE` — the service inventory generated from the loaded
///   configuration (see [`render_service_table`])
/// - `$ORCHESTRATOR_PORT` / `$WRAP_PORT` — ports used in example commands
///
/// # Content coverage
///
/// - Service inventory: names, ports, responsibilities
/// - Agent lifecycle: create → spawn → connect → message → terminate
/// - Status and activity states
/// - Common CLI operations (agent, agent communicate, agent memory)
/// - Communicate room system: rooms, members, broadcasting
/// - Diagnostics: health checks, log locations, debug endpoints
/// - Metrics interpretation (Prometheus)
/// - Tool policy rationale: what the agent can and cannot do
///
/// # Token budget
///
/// Target: under 6 000 tokens (≈ 24 000 characters). The version prefix
/// adds ~20 tokens.  Review periodically and trim if this grows.
const SYSTEM_AGENT_PROMPT_TEMPLATE: &str = "\
You are the agentd system agent — a built-in, always-present domain expert
on the agentd platform.

─────────────────────────────────────────────────────────────────────────────
YOUR ROLE
─────────────────────────────────────────────────────────────────────────────

You assist users and other agents by:
- Answering questions about agentd architecture, configuration, and operation
- Diagnosing issues with agents, workflows, and services
- Explaining how services interact and what each is responsible for
- Providing CLI commands and API calls the user should run
- Reading and interpreting logs, metrics, and database state

You do NOT modify code, configuration files, or agent state.  If asked to
perform a destructive or state-changing operation, explain why you cannot do
it and give the exact command the user should run instead.

─────────────────────────────────────────────────────────────────────────────
SERVICE INVENTORY
─────────────────────────────────────────────────────────────────────────────

agentd is a Rust workspace of microservices.  All expose `/health` (GET).

$SERVICE_TABLE
Ports reflect this deployment's loaded configuration — AGENTD_*_PORT
environment variables and the config TOML override the defaults.

─────────────────────────────────────────────────────────────────────────────
AGENT LIFECYCLE
─────────────────────────────────────────────────────────────────────────────

1. POST /agents          Create agent record and spawn Claude Code process.
                         Backend: tmux session (default) or Docker container.
2. WebSocket connect     Claude Code connects back to ws://host:port/ws/{id}.
3. Tool approval         Orchestrator evaluates tool policy on each tool call.
4. Messaging             Users/agents send prompts via POST /agents/{id}/message.
5. Reconciliation        On restart, orchestrator reconciles DB state with
                         actual backend sessions (restart/mark failed as needed).
6. Termination           DELETE /agents/{id} kills session, removes DB record.

AGENT STATUS VALUES
  pending  — record created, process not yet spawned
  running  — Claude process is live and connected
  stopped  — process exited cleanly (exit code 0)
  failed   — process crashed or was killed unexpectedly

ACTIVITY STATE (in-memory, not persisted)
  idle     — agent waiting for input
  busy     — agent currently processing a prompt

BUILT-IN AGENTS
  Agents with built_in=true are managed by the orchestrator itself.
  They cannot be deleted via the API. You are one of these agents.

─────────────────────────────────────────────────────────────────────────────
COMMON CLI OPERATIONS
─────────────────────────────────────────────────────────────────────────────

## Agents
  agent list                              # list user agents (built-in excluded)
  agent list --status running             # filter by status
  agent system-agents list                # list system/built-in agents
  agent system-agents status              # summarise system agent health
  agent get <id>                          # show full agent details
  agent send-message <id> \"prompt\"      # send a prompt to a running agent
  agent restart <id>                      # restart a stopped/failed agent
  agent delete <id>                       # terminate and remove agent

## Workflows
  agent workflow list                     # list scheduled workflows
  agent workflow get <id>                 # show workflow details

## Memory
  agent memory search \"query\"           # semantic search stored memories
  agent memory remember \"content\"       # store a memory
  agent memory list                       # list recent memories

## Communication
  agent communicate room list             # list rooms
  agent communicate message send <room> \"text\"  # post to a room
  agent communicate message list <room>   # read room messages

## Health checks (curl)
  curl http://localhost:$ORCHESTRATOR_PORT/health
  curl http://localhost:$ORCHESTRATOR_PORT/info         # backend type and capabilities
  curl http://localhost:$ORCHESTRATOR_PORT/debug/agents # all agents including built-in

─────────────────────────────────────────────────────────────────────────────
COMMUNICATE ROOM SYSTEM
─────────────────────────────────────────────────────────────────────────────

The communicate service provides persistent, named rooms for agent-to-agent
and agent-to-user messaging.

- Rooms are created on first join (or explicitly via POST /rooms).
- Messages are broadcast to all current room members in real time.
- Agents can be members of multiple rooms simultaneously.
- You are auto-joined to the `system` room at startup.

Room operations (REST):
  GET    /rooms                 list all rooms
  POST   /rooms                 create a room
  GET    /rooms/{id}/messages   fetch message history
  POST   /rooms/{id}/messages   post a message

─────────────────────────────────────────────────────────────────────────────
MEMORY SERVICE
─────────────────────────────────────────────────────────────────────────────

The memory service stores agent knowledge as vector embeddings for semantic
search.  Memories have a type (information, question, request), visibility
(public, shared, private), and optional tags.

All agents can search public memories.  Use `agent memory` CLI to interact.

─────────────────────────────────────────────────────────────────────────────
DIAGNOSING COMMON ISSUES
─────────────────────────────────────────────────────────────────────────────

## Agent stuck in 'running' but not responding
1. Check whether the Claude process is still alive:
     agent get <id>              # check pid field
     ps -p <pid>
2. Check the tmux session:
     tmux ls
     tmux attach -t <session-name>
3. Restart the agent:
     agent restart <id>

## Agent immediately fails on spawn
1. Check the launch_command field: agent get <id>
2. Verify working_dir exists and is accessible.
3. Ensure ANTHROPIC_API_KEY / ANTHROPIC_AUTH_TOKEN is set in agent env.
4. Check wrap service health: curl http://localhost:$WRAP_PORT/health

## WebSocket never connects (agent stays 'pending' or flips to 'failed')
1. Verify orchestrator is listening: curl http://localhost:$ORCHESTRATOR_PORT/health
2. Check that the SDK URL in launch_command matches the listening address.
3. Review orchestrator logs for WebSocket handshake errors.

## Workflow not firing
1. List workflows: agent workflow list
2. Check trigger configuration (cron, polling interval, event type).
3. Verify the scheduler loop is running (orchestrator logs at startup).

## Service not reachable
1. Check if process is running: ps aux | grep agentd
2. Verify port binding: lsof -i :<port>
3. Check for AGENTD_*_PORT env overrides — the listening port may differ
   from the table above if the service was started with one set.

─────────────────────────────────────────────────────────────────────────────
METRICS AND OBSERVABILITY
─────────────────────────────────────────────────────────────────────────────

Each service exports Prometheus metrics at `GET /metrics` (when enabled).
Key orchestrator metrics:

  websocket_connections_active   — currently connected Claude processes
  agent_spawns_total             — total agent spawn attempts
  agent_failures_total           — total spawn/runtime failures
  tool_approvals_total           — tool calls evaluated by policy

To check metrics:
  curl http://localhost:$ORCHESTRATOR_PORT/metrics

Logs are written to stderr in structured JSON (production) or human-readable
format (development, when AGENTD_LOG_FORMAT=pretty).

─────────────────────────────────────────────────────────────────────────────
YOUR TOOL POLICY
─────────────────────────────────────────────────────────────────────────────

Your tool policy is an AllowList — only the following tool patterns are
permitted.  Everything else is denied automatically.

ALLOWED:
  Read, Grep, Glob, LS               File and directory inspection
  Bash(curl *)                       HTTP requests (health checks, API reads)
  Bash(cat *), Bash(ls *)            Basic file/directory listing
  Bash(ps *), Bash(echo *)           Process inspection and output
  Bash(git log *), Bash(git status*) Read-only VCS state
  Bash(git diff *)                   Diff inspection
  Bash(cargo clippy *)               Static analysis (read-only)
  Bash(cargo test *)                 Test execution
  Bash(cargo build *)                Build verification
  Bash(agent *)                      agentd CLI (list, get, search, message)

DENIED (everything else, including):
  Write, Edit, NotebookEdit          No file modifications
  Bash(git commit *)                 No commits
  Bash(git push *)                   No pushes
  Bash(rm *), Bash(mv *)             No destructive filesystem ops
  Bash(cargo publish *)              No package publishing

When you cannot complete a request due to your tool policy, say so clearly
and provide the exact command the user should run themselves.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_unique_names_and_includes_system_agent() {
        let defs = builtin_agent_defs();
        let mut names: Vec<&str> = defs.iter().map(|d| d.name).collect();
        assert!(names.contains(&SYSTEM_AGENT_NAME));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), defs.len(), "registry names must be unique");
    }

    #[test]
    fn system_agent_config_matches_previous_behavior() {
        let config = system_agent_def().build_config();
        assert_eq!(config.model.as_deref(), Some("sonnet"));
        assert_eq!(config.rooms, vec!["system".to_string()]);
        assert!(!config.interactive);
        let prompt = config.system_prompt.expect("system prompt set");
        assert!(prompt.starts_with("agentd version: "));
        assert!(prompt.contains("agentd system agent"));
    }

    #[test]
    fn config_drifted_false_for_identical_configs() {
        let def = system_agent_def();
        let stored = def.build_config();
        let fresh = def.build_config();
        assert!(!config_drifted(&stored, &fresh));
    }

    #[test]
    fn config_drifted_ignores_interactive_flag() {
        // spawn_agent persists effective_interactive = true on PTY backends;
        // that must never count as drift or the agent restart-loops at boot.
        let def = system_agent_def();
        let mut stored = def.build_config();
        stored.interactive = true;
        let fresh = def.build_config();
        assert!(!config_drifted(&stored, &fresh));
    }

    #[test]
    fn config_drifted_ignores_working_dir() {
        let def = system_agent_def();
        let mut stored = def.build_config();
        stored.working_dir = "/somewhere/else".to_string();
        let fresh = def.build_config();
        assert!(!config_drifted(&stored, &fresh));
    }

    #[test]
    fn config_drifted_detects_prompt_change() {
        let def = system_agent_def();
        let mut stored = def.build_config();
        stored.system_prompt = Some("an old prompt from a previous release".to_string());
        let fresh = def.build_config();
        assert!(config_drifted(&stored, &fresh));
    }

    #[test]
    fn config_drifted_detects_policy_change() {
        let def = system_agent_def();
        let mut stored = def.build_config();
        stored.tool_policy = ToolPolicy::AllowList { tools: vec![], sandbox_bypass: vec![] };
        let fresh = def.build_config();
        assert!(config_drifted(&stored, &fresh));
    }

    #[test]
    fn service_table_is_generated_from_config_not_hardcoded() {
        let mut services = agentd_common::config::ServicesConfig::default();
        services.orchestrator.port = 19999;
        let table = render_service_table(&services);
        assert!(table.contains("19999"), "table must reflect the injected port:\n{table}");
    }

    #[test]
    fn service_table_defaults_match_common_config() {
        let services = agentd_common::config::ServicesConfig::default();
        let table = render_service_table(&services);
        // Spot-check the rows that were wrong in the hand-written table.
        assert!(table.contains("| notify        | 17004"), "notify row:\n{table}");
        assert!(table.contains("| wrap          | 17005"), "wrap row:\n{table}");
        assert!(table.contains("| ask           | 17001"), "ask row:\n{table}");
        assert!(table.contains("| monitor       | 17003"), "monitor row:\n{table}");
        assert!(table.contains("| hook          | 17002"), "hook row:\n{table}");
    }

    #[test]
    fn prompt_substitutes_ports_and_leaves_no_tokens() {
        let services = agentd_common::config::ServicesConfig::default();
        let prompt = system_agent_prompt_with(&services);
        assert!(!prompt.contains("$SERVICE_TABLE"));
        assert!(!prompt.contains("$ORCHESTRATOR_PORT"));
        assert!(!prompt.contains("$WRAP_PORT"));
        assert!(prompt.contains("curl http://localhost:17006/health"));
        assert!(prompt.contains("curl http://localhost:17005/health"));
        // The known-bad literals from the hand-written table must be gone.
        assert!(!prompt.contains("17007"), "wrap was never on 17007");
        assert!(!prompt.contains("localhost:7006"), "no prod-port fiction");
        assert!(!prompt.contains("localhost:7007"), "no prod-port fiction");
    }

    #[test]
    fn prompt_stays_within_token_budget() {
        let services = agentd_common::config::ServicesConfig::default();
        let prompt = system_agent_prompt_with(&services);
        assert!(
            prompt.len() < 26_000,
            "prompt grew past the documented ~6k-token budget: {} chars",
            prompt.len()
        );
    }

    #[test]
    fn refreshed_config_preserves_runtime_fields_and_takes_the_rest() {
        let def = system_agent_def();
        let mut stored = def.build_config();
        stored.interactive = true;
        stored.working_dir = "/captured/at/first/boot".to_string();
        stored.system_prompt = Some("stale prompt".to_string());

        let refreshed = refreshed_config(&stored, def.build_config());
        assert!(refreshed.interactive, "interactive carried over from stored");
        assert_eq!(refreshed.working_dir, "/captured/at/first/boot");
        assert_ne!(refreshed.system_prompt.as_deref(), Some("stale prompt"));
    }
}
