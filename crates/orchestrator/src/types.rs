use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A volume mount specification for Docker-backed agents.
///
/// Maps a host directory into the container at a specified path,
/// optionally as read-only. These are ignored for tmux backends.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VolumeMount {
    /// Path on the host machine.
    pub host_path: String,
    /// Mount point inside the container.
    pub container_path: String,
    /// If true, mount as read-only. Defaults to `false`.
    #[serde(default)]
    pub read_only: bool,
}

/// Resource limits for Docker-backed agent containers.
///
/// These are translated to Docker's `NanoCpus` and `Memory` host-config
/// fields. Ignored for tmux backends.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceLimits {
    /// Number of CPUs (e.g., `2.0` means two full cores).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_limit: Option<f64>,
    /// Memory cap in megabytes (e.g., `2048` for 2 GiB).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit_mb: Option<u64>,
}

/// Status of an agent managed by the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent record created, not yet running.
    Pending,
    /// Agent is running in a tmux session.
    Running,
    /// Agent was explicitly stopped.
    Stopped,
    /// Agent process failed or crashed.
    Failed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Pending => write!(f, "pending"),
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Stopped => write!(f, "stopped"),
            AgentStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Activity state of a connected agent — whether it is currently processing a
/// prompt or waiting for input.
///
/// This is tracked in memory by the [`ConnectionRegistry`] and is not
/// persisted to the database. A newly connected agent defaults to `Idle`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    /// Agent is waiting for input — no prompt is currently being processed.
    #[default]
    Idle,
    /// Agent is actively processing a prompt.
    Busy,
}

impl std::str::FromStr for AgentStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(AgentStatus::Pending),
            "running" => Ok(AgentStatus::Running),
            "stopped" => Ok(AgentStatus::Stopped),
            "failed" => Ok(AgentStatus::Failed),
            _ => Err(anyhow::anyhow!("Unknown agent status: {}", s)),
        }
    }
}

/// Policy controlling which tools an agent is allowed to use.
///
/// When a Claude Code agent requests permission to use a tool (via the
/// `can_use_tool` control request), this policy is evaluated to decide
/// whether to allow or deny the request.
///
/// All variants accept an optional `sandbox_bypass` list of tool+command globs.
/// When a Bash call matches a glob in this list, the orchestrator auto-approves
/// it with `dangerouslyDisableSandbox: true` injected into the tool input,
/// bypassing the Claude Code sandbox for that specific call.
///
/// Example glob syntax:
/// - `"Bash(git-spice *)"` — any git-spice command
/// - `"Bash(gh pr *)"` — any `gh pr` subcommand
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ToolPolicy {
    /// Allow all tools without restriction (default).
    AllowAll {
        /// Tool+command globs auto-approved with sandbox disabled.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sandbox_bypass: Vec<String>,
    },
    /// Deny all tool usage.
    DenyAll {
        /// Tool+command globs auto-approved with sandbox disabled.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sandbox_bypass: Vec<String>,
    },
    /// Only allow the listed tools; deny everything else.
    AllowList {
        tools: Vec<String>,
        /// Tool+command globs auto-approved with sandbox disabled.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sandbox_bypass: Vec<String>,
    },
    /// Allow everything except the listed tools.
    DenyList {
        tools: Vec<String>,
        /// Tool+command globs auto-approved with sandbox disabled.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sandbox_bypass: Vec<String>,
    },
    /// Hold every tool request for human approval before permitting it.
    RequireApproval {
        /// Tool+command globs auto-approved with sandbox disabled.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sandbox_bypass: Vec<String>,
    },
}

impl Default for ToolPolicy {
    fn default() -> Self {
        ToolPolicy::AllowAll { sandbox_bypass: vec![] }
    }
}

impl ToolPolicy {
    /// Evaluate whether a tool is allowed by this policy.
    ///
    /// `input` is the full tool-input JSON (e.g. `{"command": "cargo test"}`).
    /// When a pattern like `"Bash(cargo *)"` appears in the list, it is matched
    /// against the tool name and the `command` field of the input.
    ///
    /// Note: `RequireApproval` returns `false` here as a fallback — the actual
    /// approval logic is handled in `websocket.rs` before `evaluate` is called.
    /// Note: sandbox_bypass matching is handled before `evaluate` is called in
    /// `websocket.rs`; it does not affect the return value of this method.
    pub fn evaluate(&self, tool_name: &str, input: Option<&serde_json::Value>) -> bool {
        match self {
            ToolPolicy::AllowAll { .. } => true,
            ToolPolicy::DenyAll { .. } => false,
            ToolPolicy::AllowList { tools, .. } => {
                tools.iter().any(|t| match_tool(t, tool_name, input))
            }
            ToolPolicy::DenyList { tools, .. } => {
                !tools.iter().any(|t| match_tool(t, tool_name, input))
            }
            ToolPolicy::RequireApproval { .. } => false,
        }
    }

    /// Returns the policy mode as a string for logging.
    pub fn mode_str(&self) -> &'static str {
        match self {
            ToolPolicy::AllowAll { .. } => "allow_all",
            ToolPolicy::DenyAll { .. } => "deny_all",
            ToolPolicy::AllowList { .. } => "allow_list",
            ToolPolicy::DenyList { .. } => "deny_list",
            ToolPolicy::RequireApproval { .. } => "require_approval",
        }
    }

    /// Returns the sandbox_bypass glob list for this policy.
    ///
    /// A Bash call matching any of these globs is auto-approved with
    /// `dangerouslyDisableSandbox: true` injected into the tool input.
    pub fn sandbox_bypass(&self) -> &[String] {
        match self {
            ToolPolicy::AllowAll { sandbox_bypass }
            | ToolPolicy::DenyAll { sandbox_bypass }
            | ToolPolicy::RequireApproval { sandbox_bypass } => sandbox_bypass,
            ToolPolicy::AllowList { sandbox_bypass, .. }
            | ToolPolicy::DenyList { sandbox_bypass, .. } => sandbox_bypass,
        }
    }

    /// Check whether a tool call matches any sandbox_bypass glob.
    ///
    /// Returns `true` if the call should be auto-approved with the sandbox
    /// disabled. Uses the same glob matching as the tool policy list.
    pub fn matches_sandbox_bypass(
        &self,
        tool_name: &str,
        input: Option<&serde_json::Value>,
    ) -> bool {
        self.sandbox_bypass().iter().any(|pattern| match_tool(pattern, tool_name, input))
    }
}

/// Match a tool entry against a tool name and its input.
///
/// Supports three forms:
/// - `"Bash"` — plain name match, ignores input.
/// - `"Bash(cargo *)"` — matches only when the tool is `Bash` and its
///   `command` input field satisfies the glob-style pattern.
/// - `"Write(docs/**)"` — matches only when the tool is `Write` (or any
///   file tool with a `file_path` input field) and the path satisfies
///   the glob-style pattern. Supports `*` (single segment) and `**`
///   (any number of segments).
fn match_tool(pattern: &str, tool_name: &str, input: Option<&serde_json::Value>) -> bool {
    if pattern == tool_name {
        return true;
    }
    if let Some((tool, cmd_pattern)) = parse_tool_pattern(pattern) {
        if tool == tool_name {
            // For Bash, match against the 'command' field.
            if let Some(cmd) = input.and_then(|v| v.get("command")).and_then(|v| v.as_str()) {
                return match_command_pattern(cmd_pattern, cmd);
            }
            // For file tools (Write, Edit, Read, MultiEdit, NotebookEdit),
            // match against the 'file_path' field using glob path patterns.
            if let Some(file_path) = input.and_then(|v| v.get("file_path")).and_then(|v| v.as_str())
            {
                return match_path_pattern(cmd_pattern, file_path);
            }
        }
    }
    false
}

/// Parse `"Bash(cargo *)"` into `("Bash", "cargo *")`.
fn parse_tool_pattern(pattern: &str) -> Option<(&str, &str)> {
    let open = pattern.find('(')?;
    if !pattern.ends_with(')') {
        return None;
    }
    let tool = &pattern[..open];
    let cmd_pattern = &pattern[open + 1..pattern.len() - 1];
    Some((tool, cmd_pattern))
}

/// Match a command string against a simple glob-style pattern.
///
/// Supported forms:
/// - `"*"` — matches anything.
/// - `"prefix *"` — matches commands that start with `prefix ` (space-delimited token).
/// - `"prefix*"` — matches commands that start with `prefix` (no space required).
/// - `"* suffix"` — matches commands that end with `suffix`.
/// - `"* word *"` — matches commands that contain `word` as a token.
/// - `"exact"` — exact match (after leading whitespace is trimmed).
///
/// The `"prefix*"` form is useful for commands that may have zero or more
/// arguments without a guaranteed leading space, e.g. `"git-spice repo sync*"`
/// matches both `"git-spice repo sync"` and `"git-spice repo sync --remote"`.
fn match_command_pattern(pattern: &str, command: &str) -> bool {
    let cmd = command.trim_start();

    if pattern == "*" {
        return true;
    }

    if let Some(suffix) = pattern.strip_prefix("* ") {
        if let Some(rest) = suffix.strip_suffix(" *") {
            return cmd.contains(&format!(" {rest} "))
                || cmd.starts_with(&format!("{rest} "))
                || cmd.ends_with(&format!(" {rest}"))
                || cmd == rest;
        }
        return cmd.ends_with(&format!(" {suffix}")) || cmd == suffix;
    }

    // Space-delimited prefix: "prefix *" matches "prefix" or "prefix <args>".
    if let Some(prefix) = pattern.strip_suffix(" *") {
        return cmd == prefix || cmd.starts_with(&format!("{prefix} "));
    }

    // No-space trailing wildcard: "prefix*" matches any command beginning with
    // `prefix`. Useful for patterns like "git-spice repo sync*" that should
    // match both the bare command and any flags appended to it.
    if let Some(prefix) = pattern.strip_suffix('*') {
        if !prefix.is_empty() && !prefix.contains('*') {
            return cmd.starts_with(prefix);
        }
    }

    cmd == pattern
}

/// Match a file path against a glob-style pattern.
///
/// Supported forms:
/// - `"*"` — matches any single path segment.
/// - `"**"` — matches zero or more path segments (recursive).
/// - `"docs/**"` — matches all files recursively under `docs/`.
/// - `"crates/**/*.rs"` — matches any `.rs` file at any depth under `crates/`.
/// - `"*.rs"` — matches any `.rs` file in the current directory.
/// - `"exact/path.rs"` — exact match.
///
/// `*` within a segment matches any sequence of characters that does not
/// contain `/`.  `**` as a full segment matches zero or more `/`-separated
/// segments.
fn match_path_pattern(pattern: &str, path: &str) -> bool {
    let pat_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();
    match_path_segments(&pat_parts, &path_parts)
}

/// Recursive helper for [`match_path_pattern`].
fn match_path_segments(pat: &[&str], path: &[&str]) -> bool {
    match (pat, path) {
        // Both exhausted — full match.
        ([], []) => true,
        // Pattern exhausted but path still has segments — no match.
        ([], _) => false,
        // Only `**` left — matches any (including empty) remaining path.
        (["**"], _) => true,
        // `**` in the middle: try consuming 0..=path.len() segments.
        (["**", rest_pat @ ..], _) => {
            for skip in 0..=path.len() {
                if match_path_segments(rest_pat, &path[skip..]) {
                    return true;
                }
            }
            false
        }
        // Path exhausted but non-`**` pattern remains — no match.
        (_, []) => false,
        // Normal segment: match one segment and recurse.
        ([p, rest_pat @ ..], [s, rest_path @ ..]) => {
            match_path_segment(p, s) && match_path_segments(rest_pat, rest_path)
        }
    }
}

/// Match a single path segment against a pattern that may contain `*`.
///
/// `*` matches any sequence of characters (excluding `/`) within one segment.
fn match_path_segment(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == segment;
    }
    // Split on `*` and verify each literal piece appears in order.
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            // First piece must match the start of the segment.
            if !segment.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 {
            // Last piece must match the end of the remaining segment.
            if !segment[pos..].ends_with(part) {
                return false;
            }
            // Ensure the trailing piece does not overlap with pos.
            let needed = part.len();
            if segment[pos..].len() < needed {
                return false;
            }
        } else {
            // Middle pieces must appear in order.
            match segment[pos..].find(part) {
                Some(found) => pos += found + part.len(),
                None => return false,
            }
        }
    }
    true
}

/// Configuration for spawning an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Working directory for the agent process.
    pub working_dir: String,
    /// OS user to run the agent as (optional, defaults to current user).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Shell to run the agent in (e.g., "bash", "zsh").
    #[serde(default = "default_shell")]
    pub shell: String,
    /// If true, start claude in normal interactive mode without WebSocket.
    #[serde(default)]
    pub interactive: bool,
    /// Initial prompt to execute the claude session with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// If true, start the session with --worktree.
    #[serde(default)]
    pub worktree: bool,
    /// System prompt to use for the session (inline text).
    /// Mutually exclusive with `system_prompt_file`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Path to a file whose contents will be used as the system prompt.
    /// Mutually exclusive with `system_prompt`. Maps to `--system-prompt-file`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_file: Option<String>,
    /// If `true`, the system prompt is *appended* to the default prompt instead
    /// of replacing it. Changes the flag used:
    /// - `false` + `system_prompt` → `--system-prompt`
    /// - `false` + `system_prompt_file` → `--system-prompt-file`
    /// - `true`  + `system_prompt` → `--append-system-prompt`
    /// - `true`  + `system_prompt_file` → `--append-system-prompt-file`
    #[serde(default)]
    pub append_system_prompt: bool,
    /// Tool-use policy for this agent.
    #[serde(default)]
    pub tool_policy: ToolPolicy,
    /// Model to use for the claude session.
    /// Maps to --model flag. Accepts aliases (sonnet, opus, haiku)
    /// or full model names (claude-sonnet-4-6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Environment variables to set when launching the agent.
    /// Commonly used for ANTHROPIC_AUTH_TOKEN, ANTHROPIC_API_KEY, ANTHROPIC_BASE_URL.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    /// If set, automatically clear the agent's context when the cumulative
    /// input-token count for the current session exceeds this threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_clear_threshold: Option<u64>,
    /// Network policy for Docker-backed agents.
    ///
    /// Controls whether the container has internet access, is fully
    /// isolated, or shares the host network. Ignored for tmux backends.
    /// Defaults to `Internet` (bridge network with outbound access).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_policy: Option<wrap::docker::NetworkPolicy>,
    /// Custom Docker image override for this agent.
    ///
    /// When set, the Docker backend uses this image instead of its default.
    /// Ignored for tmux backends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_image: Option<String>,
    /// Additional volume mounts for Docker-backed agents.
    ///
    /// These are appended to the default `/workspace` bind mount.
    /// Ignored for tmux backends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_mounts: Option<Vec<VolumeMount>>,
    /// Resource limits (CPU, memory) for Docker-backed agents.
    ///
    /// Overrides the backend's default limits when set. Ignored for
    /// tmux backends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<ResourceLimits>,
    /// Additional directories the agent has access to.
    /// Maps to Claude Code's `--add-dir` flag.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_dirs: Vec<String>,
    /// Rooms the agent should automatically join when it connects.
    /// Each entry is a room name — rooms will be created if they don't exist.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rooms: Vec<String>,
}

fn default_shell() -> String {
    "zsh".to_string()
}

// ---------------------------------------------------------------------------
// Project types
// ---------------------------------------------------------------------------

/// A project groups agents, workflows, and rooms under a named boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    pub fn new(name: String, description: Option<String>) -> Self {
        let now = Utc::now();
        Self { id: Uuid::new_v4(), name, description, created_at: now, updated_at: now }
    }
}

/// Detailed project response including associated resource counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub agent_count: usize,
    pub workflow_count: usize,
}

/// Request body for POST /projects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Request body for PUT /projects/:id.
///
/// Fields that are `None` are left unchanged in the database.
/// Pass `description: Some(None)` is not supported via this struct —
/// to clear a description set it to `Some("")` or omit the field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProjectRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A managed AI agent instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub status: AgentStatus,
    pub config: AgentConfig,
    /// Backend-agnostic session identifier.
    ///
    /// For tmux backends this is the tmux session name; for Docker backends
    /// it would be the container ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Which execution backend owns this agent's session.
    ///
    /// Values: `"tmux"`, `"docker"`. Defaults to `"tmux"` for backward
    /// compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_type: Option<String>,
    /// Optional project this agent belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    /// The exact `claude` command that was generated and sent to the execution
    /// backend when the agent was spawned or restarted.  Useful for debugging
    /// flags, `--sdk-url`, model selection, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_command: Option<String>,
    /// OS process ID of the agent's subprocess. Used during startup
    /// reconciliation to check if a surviving process from a previous
    /// orchestrator run is still alive, avoiding duplicate spawns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Whether this is a built-in system agent.
    ///
    /// System agents are created programmatically by the orchestrator at startup
    /// and are always present while the service is running. User-created agents
    /// always have this set to `false`. The field is intentionally absent from
    /// [`CreateAgentRequest`] — only the orchestrator itself may set it.
    #[serde(default)]
    pub built_in: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Agent {
    pub fn new(name: String, config: AgentConfig) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            status: AgentStatus::Pending,
            config,
            session_id: None,
            backend_type: Some("tmux".to_string()),
            launch_command: None,
            pid: None,
            project_id: None,
            built_in: false,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Request body for POST /agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub working_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default = "default_shell")]
    pub shell: String,
    /// If true, start claude in normal interactive mode without WebSocket.
    #[serde(default)]
    pub interactive: bool,
    /// Initial prompt to execute the claude session with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// If true, start the session with --worktree.
    #[serde(default)]
    pub worktree: bool,
    /// System prompt to use for the session (inline text).
    /// Mutually exclusive with `system_prompt_file`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Path to a file whose contents will be used as the system prompt.
    /// Mutually exclusive with `system_prompt`. Maps to `--system-prompt-file`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_file: Option<String>,
    /// If `true`, use `--append-system-prompt` / `--append-system-prompt-file`
    /// instead of the replace variants.
    #[serde(default)]
    pub append_system_prompt: bool,
    /// Tool-use policy for this agent.
    #[serde(default)]
    pub tool_policy: ToolPolicy,
    /// Model to use for the claude session.
    /// Maps to --model flag. Accepts aliases (sonnet, opus, haiku)
    /// or full model names (claude-sonnet-4-6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Environment variables to set when launching the agent.
    /// Commonly used for ANTHROPIC_AUTH_TOKEN, ANTHROPIC_API_KEY, ANTHROPIC_BASE_URL.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    /// If set, automatically clear the agent's context when the cumulative
    /// input-token count for the current session exceeds this threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_clear_threshold: Option<u64>,
    /// Network policy for Docker-backed agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_policy: Option<wrap::docker::NetworkPolicy>,
    /// Custom Docker image override for this agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_image: Option<String>,
    /// Additional volume mounts for Docker-backed agents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_mounts: Option<Vec<VolumeMount>>,
    /// Resource limits (CPU, memory) for Docker-backed agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<ResourceLimits>,
    /// Additional directories the agent has access to.
    /// Maps to Claude Code's `--add-dir` flag.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_dirs: Vec<String>,
    /// Rooms the agent should automatically join when it connects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rooms: Vec<String>,
}

/// Response body for agent endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub id: Uuid,
    pub name: String,
    pub status: AgentStatus,
    /// Current activity state of the agent (idle or busy).
    ///
    /// This reflects whether the agent is currently processing a prompt.
    /// Defaults to `idle` for agents that are not connected via WebSocket.
    #[serde(default)]
    pub activity: ActivityState,
    pub config: AgentConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_type: Option<String>,
    /// The exact `claude` command that was generated and sent to the execution
    /// backend when the agent was spawned or restarted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Whether this is a built-in system agent.
    ///
    /// System agents are created programmatically at orchestrator startup.
    /// Clients should use this field to distinguish system agents from
    /// user-created agents and suppress destructive actions (e.g., delete).
    #[serde(default)]
    pub built_in: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Placeholder substituted for env values in API responses.
///
/// Also recognized on the way back in: `UpdateAgentRequest` env entries with
/// this exact value keep the stored value for that key, so clients can
/// round-trip a redacted config without knowing the secrets.
pub const ENV_REDACTED: &str = "***";

impl From<Agent> for AgentResponse {
    fn from(agent: Agent) -> Self {
        // Redact env values — keys are shown, but values are replaced with "***"
        // to avoid leaking secrets (API keys, tokens) via the REST API.
        let mut config = agent.config;
        config.env = config.env.into_keys().map(|k| (k, ENV_REDACTED.to_string())).collect();
        Self {
            id: agent.id,
            name: agent.name,
            status: agent.status,
            activity: ActivityState::default(),
            config,
            session_id: agent.session_id,
            backend_type: agent.backend_type,
            launch_command: agent.launch_command,
            pid: agent.pid,
            built_in: agent.built_in,
            created_at: agent.created_at,
            updated_at: agent.updated_at,
        }
    }
}

// Re-export pagination types from agentd-common.
#[allow(unused_imports)]
pub use agentd_common::types::{
    clamp_limit, PaginatedResponse, DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT,
};

// Re-export shared HealthResponse from agentd-common.
pub use agentd_common::types::HealthResponse;

/// Request body for PUT /agents/{id}/model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetModelRequest {
    /// Model to use (e.g. "sonnet", "opus", "haiku", "claude-sonnet-4-6").
    /// Use `null` to clear the model and inherit Claude Code's default.
    pub model: Option<String>,
    /// If true, restart the agent process immediately with the new model.
    /// If false (default), the model change takes effect on next restart.
    #[serde(default)]
    pub restart: bool,
}

/// Request body for PATCH /agents/{id}.
///
/// Every field is optional; absent fields are left unchanged (merge-patch
/// semantics). Plain `Option<T>` cannot express "clear this field" — for the
/// string prompts, an empty string clears the value, and `system_prompt` /
/// `system_prompt_file` are mutually exclusive (setting one non-empty clears
/// the other). Use `PUT /agents/{id}/model` to clear the model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAgentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append_system_prompt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_policy: Option<ToolPolicy>,
    /// Full replacement of the env map when present. Entries whose value is
    /// exactly [`ENV_REDACTED`] keep the currently stored value for that key;
    /// keys absent from the map are removed. Omitted = unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_clear_threshold: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_dirs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rooms: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<bool>,
    /// Restart the agent process immediately so launch-affecting changes
    /// (working_dir, shell, model, env, system prompt, additional_dirs,
    /// worktree) take effect. Defaults to `false`: the config is persisted
    /// and applies on the next restart.
    #[serde(default)]
    pub restart: bool,
}

/// Response body for PATCH /agents/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentResponse {
    #[serde(flatten)]
    pub agent: AgentResponse,
    /// True when launch-affecting fields changed on a running agent and no
    /// restart was performed — the live process is still using the old config.
    pub requires_restart: bool,
    /// True when the agent process was restarted as part of this update.
    pub restarted: bool,
}

/// Request body for POST /agents/{id}/message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
}

/// Request body for POST /agents/{id}/dirs and DELETE /agents/{id}/dirs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddDirRequest {
    pub path: String,
}

/// Response body for POST and DELETE /agents/{id}/dirs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddDirResponse {
    pub agent_id: Uuid,
    pub additional_dirs: Vec<String>,
    /// Always `true` — directory changes take effect on next agent restart.
    pub requires_restart: bool,
}

/// Response body for POST /agents/{id}/message.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub status: String,
    pub agent_id: Uuid,
}

// -- Tool approval types --

/// Status of a pending tool approval request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    TimedOut,
}

impl std::fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalStatus::Pending => write!(f, "pending"),
            ApprovalStatus::Approved => write!(f, "approved"),
            ApprovalStatus::Denied => write!(f, "denied"),
            ApprovalStatus::TimedOut => write!(f, "timed_out"),
        }
    }
}

impl std::str::FromStr for ApprovalStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ApprovalStatus::Pending),
            "approved" => Ok(ApprovalStatus::Approved),
            "denied" => Ok(ApprovalStatus::Denied),
            "timed_out" => Ok(ApprovalStatus::TimedOut),
            _ => Err(anyhow::anyhow!("Unknown approval status: {}", s)),
        }
    }
}

/// An in-flight tool approval request awaiting human decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    pub id: Uuid,
    pub agent_id: Uuid,
    /// The WebSocket request_id from the claude control_request message.
    pub request_id: String,
    pub tool_name: String,
    /// Full tool input as JSON (for display in the UI/CLI).
    pub tool_input: serde_json::Value,
    pub status: ApprovalStatus,
    pub created_at: DateTime<Utc>,
    /// When the approval will auto-deny if not acted on.
    pub expires_at: DateTime<Utc>,
}

/// Decision to resolve a pending approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Deny,
}

/// Request body for approval/deny endpoints (allows future extension).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalActionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// -- Usage tracking and context management types --

/// Token counts, cost, and timing from a single `result` message emitted by
/// the Claude Code SDK.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub total_cost_usd: f64,
    pub num_turns: u64,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
}

/// Session-level aggregated usage — shared shape for both the active session
/// and the cumulative lifetime totals in [`AgentUsageStats`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub total_cost_usd: f64,
    pub num_turns: u64,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    /// Number of `result` messages counted in this session.
    pub result_count: u32,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

/// Per-agent aggregated usage statistics, including the active session and
/// lifetime cumulative totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUsageStats {
    pub agent_id: Uuid,
    /// Stats for the currently-active session, if one is in progress.
    pub current_session: Option<SessionUsage>,
    /// Aggregate totals across all completed and current sessions.
    pub cumulative: SessionUsage,
    /// Total number of sessions (including the current one, if any).
    pub session_count: u32,
}

/// Structured information passed to a [`ResultCallback`] when an agent
/// completes a task.  Replaces the previous `(Uuid, bool)` tuple.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultInfo {
    pub agent_id: Uuid,
    pub is_error: bool,
    /// Token/cost/timing snapshot parsed from the `result` message, if present.
    pub usage: Option<UsageSnapshot>,
    /// The result text produced by the agent (the `result` field of the SDK
    /// `result` message). Empty string when the agent produced no text output.
    #[serde(default)]
    pub result_text: String,
}

/// Request body for POST /agents/{id}/clear-context.
///
/// Currently has no required fields; reserved for future options (e.g. forcing
/// a checkpoint even when under threshold).
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClearContextRequest {}

/// Response body for POST /agents/{id}/clear-context.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearContextResponse {
    pub agent_id: Uuid,
    /// Usage statistics at the moment the context was cleared.
    pub session_usage: Option<SessionUsage>,
    /// The session number that will be used going forward (1-based).
    pub new_session_number: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_agent_request_minimal_payload() {
        // All fields optional: an empty object is a valid no-op patch.
        let req: UpdateAgentRequest = serde_json::from_str("{}").unwrap();
        assert!(req.name.is_none());
        assert!(req.env.is_none());
        assert!(!req.restart);
    }

    #[test]
    fn test_update_agent_request_partial_payload() {
        let req: UpdateAgentRequest =
            serde_json::from_str(r#"{"model": "opus", "restart": true}"#).unwrap();
        assert_eq!(req.model.as_deref(), Some("opus"));
        assert!(req.restart);
        assert!(req.working_dir.is_none());
    }

    #[test]
    fn test_update_agent_request_serializes_only_present_fields() {
        let req = UpdateAgentRequest { model: Some("opus".to_string()), ..Default::default() };
        let json = serde_json::to_value(&req).unwrap();
        let obj = json.as_object().unwrap();
        // Absent options are skipped; only `model` and the non-optional
        // `restart` flag appear.
        assert_eq!(obj.len(), 2, "unexpected fields serialized: {obj:?}");
        assert_eq!(json["model"], "opus");
        assert_eq!(json["restart"], false);
    }

    #[test]
    fn test_agent_response_env_redaction_uses_shared_constant() {
        let mut config: AgentConfig =
            serde_json::from_value(serde_json::json!({ "working_dir": "/tmp" })).unwrap();
        config.env.insert("ANTHROPIC_API_KEY".to_string(), "sk-secret".to_string());
        let agent = Agent::new("redacted".to_string(), config);

        let response = AgentResponse::from(agent);
        assert_eq!(
            response.config.env.get("ANTHROPIC_API_KEY").map(String::as_str),
            Some(ENV_REDACTED)
        );
    }

    #[test]
    fn test_tool_policy_allow_all() {
        let policy = ToolPolicy::AllowAll { sandbox_bypass: vec![] };
        assert!(policy.evaluate("Bash", None));
        assert!(policy.evaluate("Read", None));
        assert!(policy.evaluate("Write", None));
        assert!(policy.evaluate("anything", None));
    }

    #[test]
    fn test_tool_policy_deny_all() {
        let policy = ToolPolicy::DenyAll { sandbox_bypass: vec![] };
        assert!(!policy.evaluate("Bash", None));
        assert!(!policy.evaluate("Read", None));
        assert!(!policy.evaluate("anything", None));
    }

    #[test]
    fn test_tool_policy_allow_list() {
        let policy = ToolPolicy::AllowList {
            tools: vec!["Read".to_string(), "Grep".to_string()],
            sandbox_bypass: vec![],
        };
        assert!(policy.evaluate("Read", None));
        assert!(policy.evaluate("Grep", None));
        assert!(!policy.evaluate("Bash", None));
        assert!(!policy.evaluate("Write", None));
    }

    #[test]
    fn test_tool_policy_deny_list() {
        let policy = ToolPolicy::DenyList {
            tools: vec!["Bash".to_string(), "Write".to_string()],
            sandbox_bypass: vec![],
        };
        assert!(!policy.evaluate("Bash", None));
        assert!(!policy.evaluate("Write", None));
        assert!(policy.evaluate("Read", None));
        assert!(policy.evaluate("Grep", None));
    }

    #[test]
    fn test_tool_policy_default_is_allow_all() {
        let policy = ToolPolicy::default();
        assert_eq!(policy, ToolPolicy::AllowAll { sandbox_bypass: vec![] });
        assert!(policy.evaluate("anything", None));
    }

    #[test]
    fn test_tool_policy_serialization_allow_all() {
        let policy = ToolPolicy::AllowAll { sandbox_bypass: vec![] };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("allow_all"));

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ToolPolicy::AllowAll { sandbox_bypass: vec![] });
    }

    #[test]
    fn test_tool_policy_serialization_deny_list() {
        let policy = ToolPolicy::DenyList {
            tools: vec!["Bash".to_string(), "Write".to_string()],
            sandbox_bypass: vec![],
        };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("deny_list"));
        assert!(json.contains("Bash"));

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, policy);
    }

    #[test]
    fn test_tool_policy_serialization_allow_list() {
        let policy =
            ToolPolicy::AllowList { tools: vec!["Read".to_string()], sandbox_bypass: vec![] };
        let json = serde_json::to_string(&policy).unwrap();

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, policy);
    }

    #[test]
    fn test_tool_policy_empty_allow_list_denies_all() {
        let policy = ToolPolicy::AllowList { tools: vec![], sandbox_bypass: vec![] };
        assert!(!policy.evaluate("Read", None));
        assert!(!policy.evaluate("Bash", None));
    }

    #[test]
    fn test_tool_policy_empty_deny_list_allows_all() {
        let policy = ToolPolicy::DenyList { tools: vec![], sandbox_bypass: vec![] };
        assert!(policy.evaluate("Read", None));
        assert!(policy.evaluate("Bash", None));
    }

    #[test]
    fn test_tool_policy_require_approval() {
        let policy = ToolPolicy::RequireApproval { sandbox_bypass: vec![] };
        // evaluate returns false as fallback — actual logic is in websocket.rs
        assert!(!policy.evaluate("Bash", None));
        assert!(!policy.evaluate("Read", None));
    }

    #[test]
    fn test_tool_policy_serialization_require_approval() {
        let policy = ToolPolicy::RequireApproval { sandbox_bypass: vec![] };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("require_approval"));

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ToolPolicy::RequireApproval { sandbox_bypass: vec![] });
    }

    #[test]
    fn test_approval_status_display_and_parse() {
        for (status, expected) in [
            (ApprovalStatus::Pending, "pending"),
            (ApprovalStatus::Approved, "approved"),
            (ApprovalStatus::Denied, "denied"),
            (ApprovalStatus::TimedOut, "timed_out"),
        ] {
            assert_eq!(status.to_string(), expected);
            assert_eq!(expected.parse::<ApprovalStatus>().unwrap(), status);
        }
    }

    #[test]
    fn test_agent_config_model_serialization() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            system_prompt_file: None,
            append_system_prompt: false,
            tool_policy: ToolPolicy::default(),
            model: Some("opus".to_string()),
            env: HashMap::new(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec![],
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"model\":\"opus\""));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, Some("opus".to_string()));
    }

    #[test]
    fn test_agent_config_model_none_omitted() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("model"));
    }

    #[test]
    fn test_create_agent_request_model_field() {
        let request = CreateAgentRequest {
            name: "test".to_string(),
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            system_prompt_file: None,
            append_system_prompt: false,
            tool_policy: ToolPolicy::default(),
            model: Some("sonnet".to_string()),
            env: HashMap::new(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec![],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"sonnet\""));

        let deserialized: CreateAgentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, Some("sonnet".to_string()));
    }

    #[test]
    fn test_agent_config_env_serialization() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-test-key".to_string());
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://example.com".to_string());

        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
            env: env.clone(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec![],
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("ANTHROPIC_API_KEY"));
        assert!(json.contains("sk-test-key"));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.env, env);
    }

    #[test]
    fn test_agent_config_env_empty_omitted() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("\"env\""));

        // Deserializing without env field gives empty map
        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.env.is_empty());
    }

    #[test]
    fn test_agent_config_env_default_from_missing_field() {
        // Backward compatibility: old JSON without env field should deserialize to empty map
        let json = r#"{"working_dir":"/tmp","shell":"zsh","tool_policy":{"mode":"allow_all"}}"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert!(config.env.is_empty());
    }

    #[test]
    fn test_create_agent_request_env_field() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-test".to_string());

        let request = CreateAgentRequest {
            name: "test".to_string(),
            working_dir: "/tmp".to_string(),
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
            env: env.clone(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec![],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("ANTHROPIC_API_KEY"));

        let deserialized: CreateAgentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.env, env);
    }

    #[test]
    fn test_agent_response_env_values_redacted() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-secret-key".to_string());
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://example.com".to_string());

        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
            env,
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec![],
        };
        let agent = Agent::new("test".to_string(), config);
        let response = AgentResponse::from(agent);

        // Keys should be present, but values should be redacted
        assert_eq!(response.config.env.get("ANTHROPIC_API_KEY"), Some(&"***".to_string()));
        assert_eq!(response.config.env.get("ANTHROPIC_BASE_URL"), Some(&"***".to_string()));
        // Secret value must not appear
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("sk-secret-key"));
    }

    #[test]
    fn test_set_model_request_serialization() {
        let request = SetModelRequest { model: Some("opus".to_string()), restart: true };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"opus\""));
        assert!(json.contains("\"restart\":true"));

        let deserialized: SetModelRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, Some("opus".to_string()));
        assert!(deserialized.restart);
    }

    #[test]
    fn test_set_model_request_restart_defaults_false() {
        let json = r#"{"model":"sonnet"}"#;
        let request: SetModelRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.model, Some("sonnet".to_string()));
        assert!(!request.restart);
    }

    #[test]
    fn test_set_model_request_clear_model() {
        let request = SetModelRequest { model: None, restart: false };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":null"));

        let deserialized: SetModelRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, None);
    }

    #[test]
    fn test_tool_policy_mode_str() {
        assert_eq!(ToolPolicy::AllowAll { sandbox_bypass: vec![] }.mode_str(), "allow_all");
        assert_eq!(ToolPolicy::DenyAll { sandbox_bypass: vec![] }.mode_str(), "deny_all");
        assert_eq!(
            ToolPolicy::RequireApproval { sandbox_bypass: vec![] }.mode_str(),
            "require_approval"
        );
    }

    // -- Docker config types --

    #[test]
    fn test_volume_mount_serialization() {
        let mount = VolumeMount {
            host_path: "/data/models".to_string(),
            container_path: "/models".to_string(),
            read_only: true,
        };
        let json = serde_json::to_string(&mount).unwrap();
        assert!(json.contains("\"read_only\":true"));

        let deserialized: VolumeMount = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, mount);
    }

    #[test]
    fn test_volume_mount_read_only_defaults_false() {
        let json = r#"{"host_path":"/data","container_path":"/mnt"}"#;
        let mount: VolumeMount = serde_json::from_str(json).unwrap();
        assert!(!mount.read_only);
    }

    #[test]
    fn test_resource_limits_serialization() {
        let limits = ResourceLimits { cpu_limit: Some(2.0), memory_limit_mb: Some(4096) };
        let json = serde_json::to_string(&limits).unwrap();
        assert!(json.contains("\"cpu_limit\":2.0"));
        assert!(json.contains("\"memory_limit_mb\":4096"));

        let deserialized: ResourceLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, limits);
    }

    #[test]
    fn test_resource_limits_partial() {
        let limits = ResourceLimits { cpu_limit: Some(1.5), memory_limit_mb: None };
        let json = serde_json::to_string(&limits).unwrap();
        assert!(json.contains("\"cpu_limit\":1.5"));
        assert!(!json.contains("memory_limit_mb"));

        let deserialized: ResourceLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, limits);
    }

    #[test]
    fn test_agent_config_docker_fields_omitted_when_none() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("docker_image"));
        assert!(!json.contains("extra_mounts"));
        assert!(!json.contains("resource_limits"));
    }

    #[test]
    fn test_agent_config_with_docker_fields() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
            docker_image: Some("custom-image:v1".to_string()),
            extra_mounts: Some(vec![VolumeMount {
                host_path: "/data".to_string(),
                container_path: "/mnt/data".to_string(),
                read_only: true,
            }]),
            resource_limits: Some(ResourceLimits {
                cpu_limit: Some(4.0),
                memory_limit_mb: Some(8192),
            }),
            additional_dirs: vec![],
            rooms: vec![],
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("custom-image:v1"));
        assert!(json.contains("/data"));
        assert!(json.contains("8192"));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.docker_image, Some("custom-image:v1".to_string()));
        assert_eq!(deserialized.extra_mounts.as_ref().unwrap().len(), 1);
        assert_eq!(deserialized.resource_limits.as_ref().unwrap().cpu_limit, Some(4.0));
    }

    #[test]
    fn test_agent_config_backward_compat_missing_docker_fields() {
        // Old JSON without docker fields should deserialize successfully
        let json = r#"{"working_dir":"/tmp","shell":"zsh","tool_policy":{"mode":"allow_all"}}"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.docker_image, None);
        assert_eq!(config.extra_mounts, None);
        assert_eq!(config.resource_limits, None);
    }

    // -- additional_dirs tests --

    #[test]
    fn test_agent_config_additional_dirs_serialization() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
            additional_dirs: vec!["/opt/configs".to_string(), "/shared/libs".to_string()],
            rooms: vec![],
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("additional_dirs"));
        assert!(json.contains("/opt/configs"));
        assert!(json.contains("/shared/libs"));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.additional_dirs, vec!["/opt/configs", "/shared/libs"]);
    }

    #[test]
    fn test_agent_config_additional_dirs_empty_omitted() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
        };
        let json = serde_json::to_string(&config).unwrap();
        // Empty vec should be omitted from JSON output
        assert!(!json.contains("additional_dirs"));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.additional_dirs.is_empty());
    }

    #[test]
    fn test_agent_config_backward_compat_missing_additional_dirs() {
        // Old JSON without additional_dirs should deserialize to empty vec
        let json = r#"{"working_dir":"/tmp","shell":"zsh","tool_policy":{"mode":"allow_all"}}"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert!(config.additional_dirs.is_empty());
    }

    // -- Bash(pattern) command-level filtering tests --

    fn make_input(command: &str) -> serde_json::Value {
        serde_json::json!({"command": command})
    }

    #[test]
    fn test_match_command_pattern_wildcard() {
        assert!(match_command_pattern("*", "cargo test"));
        assert!(match_command_pattern("*", "anything at all"));
        assert!(match_command_pattern("*", ""));
    }

    #[test]
    fn test_match_command_pattern_prefix() {
        assert!(match_command_pattern("cargo *", "cargo test"));
        assert!(match_command_pattern("cargo *", "cargo build --release"));
        assert!(match_command_pattern("cargo *", "cargo"));
        assert!(!match_command_pattern("cargo *", "git status"));
        assert!(!match_command_pattern("cargo *", "notcargo test"));
    }

    #[test]
    fn test_match_command_pattern_suffix() {
        assert!(match_command_pattern("* --release", "cargo build --release"));
        assert!(match_command_pattern("* --release", "--release"));
        assert!(!match_command_pattern("* --release", "cargo build"));
        assert!(!match_command_pattern("* --release", "cargo build --release --extra"));
    }

    #[test]
    fn test_match_command_pattern_contains() {
        assert!(match_command_pattern("* test *", "cargo test --verbose"));
        assert!(match_command_pattern("* test *", "test"));
        assert!(match_command_pattern("* test *", "test --verbose"));
        assert!(match_command_pattern("* test *", "cargo test"));
        assert!(!match_command_pattern("* test *", "cargo testing"));
    }

    #[test]
    fn test_match_command_pattern_exact() {
        assert!(match_command_pattern("cargo test", "cargo test"));
        assert!(match_command_pattern("cargo test", "  cargo test"));
        assert!(!match_command_pattern("cargo test", "cargo test --verbose"));
        assert!(!match_command_pattern("cargo test", "cargo"));
    }

    #[test]
    fn test_parse_tool_pattern_valid() {
        assert_eq!(parse_tool_pattern("Bash(cargo *)"), Some(("Bash", "cargo *")));
        assert_eq!(parse_tool_pattern("Bash(*)"), Some(("Bash", "*")));
        assert_eq!(parse_tool_pattern("Bash(git status)"), Some(("Bash", "git status")));
    }

    #[test]
    fn test_parse_tool_pattern_invalid() {
        assert_eq!(parse_tool_pattern("Bash"), None);
        assert_eq!(parse_tool_pattern("Bash(cargo *"), None);
        assert_eq!(parse_tool_pattern("(*)"), Some(("", "*")));
    }

    #[test]
    fn test_match_tool_plain_name() {
        assert!(match_tool("Bash", "Bash", None));
        assert!(match_tool("Read", "Read", None));
        assert!(!match_tool("Bash", "Read", None));
    }

    #[test]
    fn test_match_tool_with_pattern_and_input() {
        let input = make_input("cargo test");
        assert!(match_tool("Bash(cargo *)", "Bash", Some(&input)));
        assert!(!match_tool("Bash(cargo *)", "Bash", Some(&make_input("git status"))));
        assert!(!match_tool("Bash(cargo *)", "Read", Some(&input)));
    }

    #[test]
    fn test_match_tool_pattern_no_input() {
        // Pattern "Bash(cargo *)" requires input; without input it should not match
        assert!(!match_tool("Bash(cargo *)", "Bash", None));
    }

    #[test]
    fn test_tool_policy_allow_list_with_bash_pattern() {
        let policy = ToolPolicy::AllowList {
            tools: vec![
                "Read".to_string(),
                "Write".to_string(),
                "Bash(cargo *)".to_string(),
                "Bash(git *)".to_string(),
            ],
            sandbox_bypass: vec![],
        };

        // Plain tools work as before
        assert!(policy.evaluate("Read", None));
        assert!(policy.evaluate("Write", None));

        // Bash allowed for cargo and git commands
        let cargo_input = make_input("cargo test");
        let git_input = make_input("git status");
        let rm_input = make_input("rm -rf /");

        assert!(policy.evaluate("Bash", Some(&cargo_input)));
        assert!(policy.evaluate("Bash", Some(&git_input)));

        // Bash denied for other commands
        assert!(!policy.evaluate("Bash", Some(&rm_input)));
        // Bash denied when no input
        assert!(!policy.evaluate("Bash", None));
        // Other tools not in list are denied
        assert!(!policy.evaluate("Grep", None));
    }

    #[test]
    fn test_tool_policy_deny_list_with_bash_pattern() {
        let policy = ToolPolicy::DenyList {
            tools: vec!["Bash(rm *)".to_string(), "Bash(sudo *)".to_string()],
            sandbox_bypass: vec![],
        };

        // Dangerous commands denied
        assert!(!policy.evaluate("Bash", Some(&make_input("rm -rf /"))));
        assert!(!policy.evaluate("Bash", Some(&make_input("sudo apt install vim"))));

        // Safe commands allowed
        assert!(policy.evaluate("Bash", Some(&make_input("cargo test"))));
        assert!(policy.evaluate("Bash", Some(&make_input("git status"))));

        // Other tools not in deny list are allowed
        assert!(policy.evaluate("Read", None));
        assert!(policy.evaluate("Write", None));
    }

    #[test]
    fn test_tool_policy_bash_wildcard_pattern() {
        let policy =
            ToolPolicy::AllowList { tools: vec!["Bash(*)".to_string()], sandbox_bypass: vec![] };
        assert!(policy.evaluate("Bash", Some(&make_input("anything"))));
        assert!(policy.evaluate("Bash", Some(&make_input("rm -rf /"))));
        // Still requires input for pattern match
        assert!(!policy.evaluate("Bash", None));
    }

    #[test]
    fn test_tool_policy_backward_compat_plain_bash() {
        // Plain "Bash" in the list should still allow any Bash call regardless of input
        let policy = ToolPolicy::AllowList {
            tools: vec!["Bash".to_string(), "Read".to_string()],
            sandbox_bypass: vec![],
        };
        assert!(policy.evaluate("Bash", None));
        assert!(policy.evaluate("Bash", Some(&make_input("rm -rf /"))));
        assert!(policy.evaluate("Read", None));
        assert!(!policy.evaluate("Write", None));
    }

    // -- file-path pattern matching tests --

    fn make_file_input(path: &str) -> serde_json::Value {
        serde_json::json!({"file_path": path})
    }

    #[test]
    fn test_match_path_pattern_exact() {
        assert!(match_path_pattern("mkdocs.yml", "mkdocs.yml"));
        assert!(!match_path_pattern("mkdocs.yml", "docs/mkdocs.yml"));
        assert!(!match_path_pattern("mkdocs.yml", "other.yml"));
    }

    #[test]
    fn test_match_path_pattern_single_star_segment() {
        assert!(match_path_pattern("*.rs", "lib.rs"));
        assert!(match_path_pattern("*.rs", "main.rs"));
        assert!(!match_path_pattern("*.rs", "src/lib.rs"));
        assert!(match_path_pattern("*_test.rs", "foo_test.rs"));
        assert!(!match_path_pattern("*_test.rs", "foo.rs"));
    }

    #[test]
    fn test_match_path_pattern_double_star_suffix() {
        assert!(match_path_pattern("docs/**", "docs/foo.md"));
        assert!(match_path_pattern("docs/**", "docs/public/bar.md"));
        assert!(match_path_pattern("docs/**", "docs/a/b/c.md"));
        // ** matches zero additional segments too (i.e. docs/ itself)
        assert!(match_path_pattern("docs/**", "docs/"));
        assert!(!match_path_pattern("docs/**", "ui/foo.md"));
        assert!(!match_path_pattern("docs/**", "DOCS/foo.md"));
    }

    #[test]
    fn test_match_path_pattern_double_star_with_extension() {
        assert!(match_path_pattern("crates/**/*.rs", "crates/orchestrator/src/lib.rs"));
        assert!(match_path_pattern("crates/**/*.rs", "crates/common/src/types.rs"));
        assert!(match_path_pattern("crates/**/*.rs", "crates/cli/src/main.rs"));
        assert!(!match_path_pattern("crates/**/*.rs", "crates/orchestrator/Cargo.toml"));
        assert!(!match_path_pattern("crates/**/*.rs", "docs/foo.rs"));
    }

    #[test]
    fn test_match_path_pattern_ui_scope() {
        assert!(match_path_pattern("ui/**", "ui/src/App.tsx"));
        assert!(match_path_pattern("ui/**", "ui/public/index.html"));
        assert!(!match_path_pattern("ui/**", "crates/common/src/lib.rs"));
    }

    #[test]
    fn test_match_path_pattern_agentd_scope() {
        assert!(match_path_pattern(".agentd/**", ".agentd/agents/worker.yml"));
        assert!(match_path_pattern(".agentd/**", ".agentd/workflows/merge-worker.yml"));
        assert!(!match_path_pattern(".agentd/**", "docs/foo.md"));
    }

    #[test]
    fn test_match_path_pattern_double_star_zero_segments() {
        // "**" alone matches any path including single-segment paths
        assert!(match_path_pattern("**", "foo.rs"));
        assert!(match_path_pattern("**", "crates/lib.rs"));
        assert!(match_path_pattern("**", "a/b/c/d.txt"));
    }

    #[test]
    fn test_tool_policy_deny_list_with_file_path_pattern() {
        // Documenter: deny writes to Rust source files
        let policy = ToolPolicy::DenyList {
            tools: vec!["Write(crates/**/*.rs)".to_string(), "Edit(crates/**/*.rs)".to_string()],
            sandbox_bypass: vec![],
        };

        // Writing to docs is allowed
        assert!(policy.evaluate("Write", Some(&make_file_input("docs/foo.md"))));
        assert!(policy.evaluate("Edit", Some(&make_file_input("docs/public/bar.md"))));

        // Writing to Rust source is denied
        assert!(!policy.evaluate("Write", Some(&make_file_input("crates/orchestrator/src/lib.rs"))));
        assert!(!policy.evaluate("Edit", Some(&make_file_input("crates/common/src/types.rs"))));

        // Bash is unaffected
        assert!(policy.evaluate("Bash", Some(&make_input("cargo test"))));
    }

    #[test]
    fn test_tool_policy_deny_list_with_ui_scope() {
        // Designer: only allow writes under ui/
        let policy = ToolPolicy::DenyList {
            tools: vec![
                "Write(crates/**)".to_string(),
                "Edit(crates/**)".to_string(),
                "Write(.agentd/**)".to_string(),
                "Edit(.agentd/**)".to_string(),
            ],
            sandbox_bypass: vec![],
        };

        // ui/ writes are allowed
        assert!(policy.evaluate("Write", Some(&make_file_input("ui/src/App.tsx"))));

        // crates/ writes are denied
        assert!(!policy.evaluate("Write", Some(&make_file_input("crates/orchestrator/src/lib.rs"))));

        // .agentd/ writes are denied
        assert!(!policy.evaluate("Edit", Some(&make_file_input(".agentd/agents/worker.yml"))));
    }

    #[test]
    fn test_tool_policy_allow_list_with_file_path_pattern() {
        // Tester: only allow writes to test files
        let policy = ToolPolicy::AllowList {
            tools: vec![
                "Read".to_string(),
                "Bash".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "Write(crates/*/tests/**)".to_string(),
                "Edit(crates/*/tests/**)".to_string(),
                "Write(.github/workflows/**)".to_string(),
                "Edit(.github/workflows/**)".to_string(),
            ],
            sandbox_bypass: vec![],
        };

        // Writing to integration test directory is allowed
        assert!(policy
            .evaluate("Write", Some(&make_file_input("crates/orchestrator/tests/integration.rs"))));
        assert!(policy.evaluate("Edit", Some(&make_file_input("crates/cli/tests/e2e.rs"))));

        // Writing to CI config is allowed
        assert!(policy.evaluate("Write", Some(&make_file_input(".github/workflows/ci.yml"))));

        // Writing to production source is denied
        assert!(!policy.evaluate("Write", Some(&make_file_input("crates/orchestrator/src/lib.rs"))));
        assert!(!policy.evaluate("Edit", Some(&make_file_input("crates/common/src/types.rs"))));

        // Read/Bash/Grep allowed without file_path
        assert!(policy.evaluate("Read", None));
        assert!(policy.evaluate("Bash", None));
        assert!(policy.evaluate("Grep", None));
    }

    #[test]
    fn test_activity_state_default_is_idle() {
        let state: ActivityState = Default::default();
        assert_eq!(state, ActivityState::Idle);
    }

    #[test]
    fn test_activity_state_serializes_as_snake_case() {
        let idle = serde_json::to_string(&ActivityState::Idle).unwrap();
        let busy = serde_json::to_string(&ActivityState::Busy).unwrap();
        assert_eq!(idle, "\"idle\"");
        assert_eq!(busy, "\"busy\"");
    }

    #[test]
    fn test_activity_state_deserializes_from_snake_case() {
        let idle: ActivityState = serde_json::from_str("\"idle\"").unwrap();
        let busy: ActivityState = serde_json::from_str("\"busy\"").unwrap();
        assert_eq!(idle, ActivityState::Idle);
        assert_eq!(busy, ActivityState::Busy);
    }

    #[test]
    fn test_agent_response_includes_activity_field() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
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
            env: Default::default(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec![],
        };
        let agent = Agent::new("test".to_string(), config);
        let response = AgentResponse::from(agent);

        // Default from the From<Agent> impl is Idle.
        assert_eq!(response.activity, ActivityState::Idle);

        // The JSON representation should contain the activity field.
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"activity\":\"idle\""));
    }

    // -- sandbox_bypass tests --

    #[test]
    fn test_sandbox_bypass_empty_by_default() {
        let policy = ToolPolicy::default();
        assert!(policy.sandbox_bypass().is_empty());
        assert!(!policy.matches_sandbox_bypass("Bash", None));
    }

    #[test]
    fn test_sandbox_bypass_matches_bash_pattern() {
        let policy = ToolPolicy::AllowAll {
            sandbox_bypass: vec![
                "Bash(git-spice branch submit *)".to_string(),
                "Bash(git-spice repo sync*)".to_string(),
                "Bash(gh pr create *)".to_string(),
            ],
        };

        let submit_input = make_input("git-spice branch submit --no-prompt issue-1042");
        let sync_input = make_input("git-spice repo sync");
        let pr_input = make_input("gh pr create --title foo");
        let safe_input = make_input("cargo test");

        assert!(policy.matches_sandbox_bypass("Bash", Some(&submit_input)));
        assert!(policy.matches_sandbox_bypass("Bash", Some(&sync_input)));
        assert!(policy.matches_sandbox_bypass("Bash", Some(&pr_input)));

        // Non-matching commands are not bypassed
        assert!(!policy.matches_sandbox_bypass("Bash", Some(&safe_input)));
        assert!(!policy.matches_sandbox_bypass("Bash", None));

        // Non-Bash tools are not bypassed
        assert!(!policy.matches_sandbox_bypass("Read", Some(&submit_input)));
    }

    #[test]
    fn test_sandbox_bypass_on_deny_list_policy() {
        let policy = ToolPolicy::DenyList {
            tools: vec!["Bash(rm *)".to_string()],
            sandbox_bypass: vec!["Bash(git-spice *)".to_string()],
        };

        let spice_input = make_input("git-spice branch submit");
        let rm_input = make_input("rm -rf /");

        // Sandbox bypass matches git-spice commands
        assert!(policy.matches_sandbox_bypass("Bash", Some(&spice_input)));
        // rm is not in sandbox_bypass
        assert!(!policy.matches_sandbox_bypass("Bash", Some(&rm_input)));
    }

    #[test]
    fn test_sandbox_bypass_serialization_round_trip() {
        let policy = ToolPolicy::AllowAll {
            sandbox_bypass: vec![
                "Bash(git-spice branch submit *)".to_string(),
                "Bash(gh pr create *)".to_string(),
            ],
        };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("sandbox_bypass"));
        assert!(json.contains("git-spice branch submit *"));

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, policy);
    }

    #[test]
    fn test_sandbox_bypass_omitted_when_empty() {
        // When sandbox_bypass is empty, it should not appear in JSON
        let policy = ToolPolicy::AllowAll { sandbox_bypass: vec![] };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(!json.contains("sandbox_bypass"));
    }

    #[test]
    fn test_sandbox_bypass_backward_compat_no_field() {
        // Old JSON without sandbox_bypass should deserialize with an empty list
        let json = r#"{"mode":"allow_all"}"#;
        let policy: ToolPolicy = serde_json::from_str(json).unwrap();
        assert!(policy.sandbox_bypass().is_empty());

        let json = r#"{"mode":"deny_list","tools":["Bash(rm *)"]}"#;
        let policy: ToolPolicy = serde_json::from_str(json).unwrap();
        assert!(policy.sandbox_bypass().is_empty());
    }
}

// ─── Conversation events ──────────────────────────────────────────────────────

/// The type of a conversation event recorded for an agent session.
///
/// Each variant maps to a distinct phase of the Claude Code conversation
/// lifecycle.  Values are stored as snake_case strings in the database.
// Consumed by #1160 (WebSocket persistence), #1161 (REST API), #1163 (retention
// policy) — stacked on this PR; suppress until those land.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationEventType {
    /// A chunk of text output from the assistant.
    Output,
    /// The assistant invoked a tool (e.g. Bash, Read, Write).
    ToolUse,
    /// An extended-thinking block produced by the model.
    Thinking,
    /// The final result returned at the end of a turn.
    Result,
    /// A prompt was sent to the agent.
    PromptSent,
    /// The agent's activity state changed (idle ↔ busy).
    ActivityChanged,
    /// A token/cost usage snapshot was recorded.
    UsageUpdate,
    /// The conversation context was cleared (session_number incremented).
    ContextCleared,
}

impl std::fmt::Display for ConversationEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversationEventType::Output => write!(f, "output"),
            ConversationEventType::ToolUse => write!(f, "tool_use"),
            ConversationEventType::Thinking => write!(f, "thinking"),
            ConversationEventType::Result => write!(f, "result"),
            ConversationEventType::PromptSent => write!(f, "prompt_sent"),
            ConversationEventType::ActivityChanged => write!(f, "activity_changed"),
            ConversationEventType::UsageUpdate => write!(f, "usage_update"),
            ConversationEventType::ContextCleared => write!(f, "context_cleared"),
        }
    }
}

impl std::str::FromStr for ConversationEventType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "output" => Ok(ConversationEventType::Output),
            "tool_use" => Ok(ConversationEventType::ToolUse),
            "thinking" => Ok(ConversationEventType::Thinking),
            "result" => Ok(ConversationEventType::Result),
            "prompt_sent" => Ok(ConversationEventType::PromptSent),
            "activity_changed" => Ok(ConversationEventType::ActivityChanged),
            "usage_update" => Ok(ConversationEventType::UsageUpdate),
            "context_cleared" => Ok(ConversationEventType::ContextCleared),
            _ => Err(anyhow::anyhow!("Unknown conversation event type: {}", s)),
        }
    }
}

/// A single persisted conversation event for an agent session.
///
/// Events are written by the WebSocket handler as messages flow through and
/// are replayed on demand via the REST API.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEvent {
    /// Unique event identifier (UUID v4).
    pub id: Uuid,
    /// The agent that produced this event.
    pub agent_id: Uuid,
    /// Discriminator for the event's structure and meaning.
    pub event_type: ConversationEventType,
    /// Monotonically increasing counter, reset each time the context is cleared.
    pub session_number: i64,
    /// Free-text payload (output text, prompt text, thinking text, etc.).
    pub content: Option<String>,
    /// Structured JSON payload (tool input/output, usage stats, etc.).
    pub metadata: Option<serde_json::Value>,
    /// When the event was recorded (UTC).
    pub created_at: DateTime<Utc>,
    /// Strictly monotonic per-agent sequence number.
    ///
    /// Independent of [`session_number`] (which resets on context clear),
    /// `seq` only ever increases for a given `agent_id`. Used by the
    /// snapshot+live streaming protocol to dedupe events across the
    /// history → live boundary and resume mid-conversation.
    ///
    /// `0` on a newly constructed event means "not yet assigned"; the
    /// real value is set by the connection registry at insert time.
    #[serde(default)]
    pub seq: i64,
}

impl ConversationEvent {
    /// Create a new event with a generated UUID and the current timestamp.
    ///
    /// `seq` is `0` until the connection registry assigns the next
    /// per-agent sequence number at persistence time.
    // Used by integration tests (conversation_persistence.rs) on stacked branches.
    #[allow(dead_code)]
    pub fn new(
        agent_id: Uuid,
        event_type: ConversationEventType,
        session_number: i64,
        content: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_id,
            event_type,
            session_number,
            content,
            metadata,
            created_at: Utc::now(),
            seq: 0,
        }
    }
}

/// Options for querying conversation events.
///
/// All fields are optional — omitting a field removes that filter.
/// Results are always ordered by `created_at ASC`.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationQuery {
    /// Restrict to events of these types (empty = all types).
    pub event_types: Option<Vec<ConversationEventType>>,
    /// Only return events created on or after this timestamp.
    pub since: Option<DateTime<Utc>>,
    /// Only return events created before this timestamp.
    pub until: Option<DateTime<Utc>>,
    /// Restrict to events from a specific session number.
    ///
    /// When set, the DB filter is pushed down so that `limit`, `has_more`, and
    /// `total` are all computed against the session-filtered result set rather
    /// than the full history.
    pub session_number: Option<i64>,
    /// Maximum number of events to return.
    pub limit: Option<u64>,
    /// Skip this many events (for offset pagination).
    pub offset: Option<u64>,
}

// ─── Conversation history API types ──────────────────────────────────────────

/// A conversation event shaped for the REST API.
///
/// The `event_type` field uses the `"agent:<variant>"` prefix to match the
/// format of live WebSocket stream events (e.g. `"agent:output"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEventResponse {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub event_type: String,
    pub agent_id: Uuid,
    /// Free-text content (output text, prompt, thinking, etc.).
    pub line: Option<String>,
    /// Structured JSON payload (tool input/output, usage stats, etc.).
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub session_number: i64,
    /// Strictly monotonic per-agent sequence number; see [`ConversationEvent::seq`].
    #[serde(default)]
    pub seq: i64,
}

impl From<ConversationEvent> for ConversationEventResponse {
    fn from(ev: ConversationEvent) -> Self {
        Self {
            id: ev.id,
            event_type: format!("agent:{}", ev.event_type),
            agent_id: ev.agent_id,
            line: ev.content,
            metadata: ev.metadata,
            timestamp: ev.created_at,
            session_number: ev.session_number,
            seq: ev.seq,
        }
    }
}

// ─── Retention / cleanup configuration ──────────────────────────────────────

/// Configuration for conversation event retention and periodic pruning.
///
/// All values are read from environment variables at service startup via
/// [`RetentionConfig::from_env`].  The defaults are conservative — 30-day
/// history, 50 k events per agent, no on-terminate delete, 6-hour cleanup cycle.
#[derive(Debug, Clone)]
pub struct RetentionConfig {
    /// Delete events older than this many days
    /// (env: `AGENTD_CONVERSATION_RETENTION_DAYS`, default: 30).
    /// Must be ≥ 1; `0` would delete all events and is rejected (falls back to default).
    pub retention_days: u64,
    /// Hard cap per agent; oldest events are evicted first.
    /// Enforced both by the periodic cleanup task (for all known agents) and
    /// on agent termination when `cleanup_on_terminate = true`.
    /// (env: `AGENTD_CONVERSATION_MAX_EVENTS_PER_AGENT`, default: 50000).
    /// Must be ≥ 1; `0` would delete all events and is rejected (falls back to default).
    pub max_events_per_agent: u64,
    /// If `true`, all events for an agent are deleted when it is terminated
    /// (env: `AGENTD_CONVERSATION_CLEANUP_ON_TERMINATE`, default: false).
    pub cleanup_on_terminate: bool,
    /// How often the periodic cleanup task runs, in seconds
    /// (env: `AGENTD_CONVERSATION_CLEANUP_INTERVAL_SECS`, default: 21600 = 6 h).
    /// Must be ≥ 1; `0` panics `tokio::time::interval` and is rejected (falls back to default).
    pub cleanup_interval_secs: u64,
}

impl RetentionConfig {
    /// Safe, hardcoded fallback values used when env vars are absent or invalid.
    pub const DEFAULT_RETENTION_DAYS: u64 = 30;
    pub const DEFAULT_MAX_EVENTS_PER_AGENT: u64 = 50_000;
    pub const DEFAULT_CLEANUP_INTERVAL_SECS: u64 = 21_600; // 6 h

    /// Build a `RetentionConfig` from environment variables, falling back to
    /// safe defaults when a variable is absent, unparseable, or out of range.
    ///
    /// Zero values for `retention_days`, `max_events_per_agent`, and
    /// `cleanup_interval_secs` are rejected and replaced with defaults:
    /// `0` retention days would wipe all events, `0` max events would delete
    /// everything for every agent, and `0` interval panics `tokio::time::interval`.
    pub fn from_env() -> Self {
        Self {
            retention_days: std::env::var("AGENTD_CONVERSATION_RETENTION_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&v: &u64| v > 0)
                .unwrap_or(Self::DEFAULT_RETENTION_DAYS),
            max_events_per_agent: std::env::var("AGENTD_CONVERSATION_MAX_EVENTS_PER_AGENT")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&v: &u64| v > 0)
                .unwrap_or(Self::DEFAULT_MAX_EVENTS_PER_AGENT),
            cleanup_on_terminate: std::env::var("AGENTD_CONVERSATION_CLEANUP_ON_TERMINATE")
                .ok()
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            cleanup_interval_secs: std::env::var("AGENTD_CONVERSATION_CLEANUP_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&v: &u64| v > 0)
                .unwrap_or(Self::DEFAULT_CLEANUP_INTERVAL_SECS),
        }
    }
}

/// Query parameters for `GET /agents/{id}/conversation`.
#[derive(Debug, Default, Deserialize)]
pub struct ConversationHistoryQuery {
    /// Maximum number of events to return (default: 100).
    pub limit: Option<u64>,
    /// Return events created before this RFC 3339 timestamp (exclusive).
    pub before: Option<String>,
    /// Return events created after this RFC 3339 timestamp (exclusive).
    pub after: Option<String>,
    /// Comma-separated list of event types to include (e.g. `"output,tool_use"`).
    pub event_type: Option<String>,
    /// Restrict results to a specific session number.
    pub session: Option<i64>,
}

/// Paginated response body for `GET /agents/{id}/conversation`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationHistoryResponse {
    pub events: Vec<ConversationEventResponse>,
    pub total: u64,
    pub has_more: bool,
}

/// Aggregate summary for `GET /agents/{id}/conversation/summary`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub agent_id: Uuid,
    pub total_events: u64,
    /// Event counts keyed by the `"agent:<variant>"` event-type string.
    pub event_counts: HashMap<String, u64>,
    pub session_count: u64,
    pub first_event_at: Option<DateTime<Utc>>,
    pub last_event_at: Option<DateTime<Utc>>,
}

impl Default for RetentionConfig {
    /// Returns hardcoded safe defaults.  Does **not** read environment variables.
    /// Use [`RetentionConfig::from_env`] explicitly when env-var overrides are needed.
    fn default() -> Self {
        Self {
            retention_days: Self::DEFAULT_RETENTION_DAYS,
            max_events_per_agent: Self::DEFAULT_MAX_EVENTS_PER_AGENT,
            cleanup_on_terminate: false,
            cleanup_interval_secs: Self::DEFAULT_CLEANUP_INTERVAL_SECS,
        }
    }
}
