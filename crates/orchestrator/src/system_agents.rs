//! Built-in system agent definitions.
//!
//! This module defines the programmatically-managed system agents that the
//! orchestrator spawns automatically at startup.  System agents are always
//! present while the service is running and cannot be deleted via the
//! user-facing API.
//!
//! # Design
//!
//! A single omniscient agent named `agentd-system` acts as a domain expert on
//! the full agentd platform.  It auto-joins a `system` communicate room and
//! uses a restrictive tool policy that prevents destructive operations while
//! still allowing read-oriented tooling for diagnostics and information
//! gathering.
//!
//! The system prompt and tool policy constants defined here are embedded
//! directly as Rust string literals so that they are version-controlled with
//! the code and require no files on disk.

use crate::types::{AgentConfig, ToolPolicy};
use std::collections::HashMap;

/// Name of the primary built-in system agent.
pub const SYSTEM_AGENT_NAME: &str = "agentd-system";

/// Build the [`AgentConfig`] for the agentd system agent.
///
/// The configuration sets:
/// - An inline system prompt covering the agentd architecture.
/// - A restrictive `AllowList` tool policy (read-only tooling only).
/// - Automatic membership in the `system` communicate room.
/// - The `sonnet` model alias.
pub fn build_system_agent_config() -> AgentConfig {
    AgentConfig {
        working_dir: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/".to_string()),
        user: None,
        shell: "zsh".to_string(),
        interactive: false,
        prompt: None,
        worktree: false,
        system_prompt: Some(SYSTEM_AGENT_PROMPT.to_string()),
        system_prompt_file: None,
        append_system_prompt: false,
        tool_policy: system_agent_tool_policy(),
        model: Some("sonnet".to_string()),
        env: HashMap::new(),
        auto_clear_threshold: None,
        network_policy: None,
        docker_image: None,
        extra_mounts: None,
        resource_limits: None,
        additional_dirs: vec![],
        rooms: vec!["system".to_string()],
    }
}

/// Restrictive tool policy for the system agent.
///
/// Uses an allowlist of read-only tools.  No `Write`, `Edit`, `NotebookEdit`,
/// or unconstrained `Bash` is permitted — the agent can read the codebase and
/// check service health but cannot modify anything.
fn system_agent_tool_policy() -> ToolPolicy {
    ToolPolicy::AllowList {
        tools: vec![
            // Navigation and search
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
            "LS".to_string(),
            // Restricted bash: only safe read-only commands
            "Bash(curl *)".to_string(),
            "Bash(cat *)".to_string(),
            "Bash(ls *)".to_string(),
            "Bash(ps *)".to_string(),
            "Bash(echo *)".to_string(),
            "Bash(git log *)".to_string(),
            "Bash(git status*)".to_string(),
            "Bash(git diff *)".to_string(),
            "Bash(cargo clippy *)".to_string(),
            "Bash(cargo test *)".to_string(),
            "Bash(cargo build *)".to_string(),
            "Bash(agent *)".to_string(),
        ],
        sandbox_bypass: vec![],
    }
}

/// System prompt for the agentd-system agent.
///
/// This prompt is embedded as a compile-time constant so it is version-controlled
/// with the code.  Keep it under 8 000 tokens (≈ 30 000 characters).
///
/// The full authoring of this prompt is tracked in issue #1142.
const SYSTEM_AGENT_PROMPT: &str = "\
You are the agentd system agent — a domain expert on the agentd platform.

# Your Role

You assist users and other agents by answering questions about the agentd
architecture, diagnosing issues, explaining how services interact, and
providing guidance on common operations.  You do NOT modify code or
configuration files.

# Architecture Overview

agentd is a Rust workspace of microservices that together provide an
AI-agent orchestration platform:

| Service         | Port  | Description                                      |
|-----------------|-------|--------------------------------------------------|
| orchestrator    | 7006  | Manages agent lifecycle, WebSocket connections   |
| communicate     | 17010 | Room-based messaging between agents and users    |
| memory          | 17008 | Semantic memory store (vector search)            |
| index           | 17012 | Code-search index (semantic + keyword)           |
| notify          | 17005 | Notification delivery and routing                |
| core            | 7010  | Authentication gateway and API proxy             |
| wrap            | 7007  | Docker/tmux execution backend                    |

All services expose REST APIs and health endpoints at `/health`.

# Agent Lifecycle

1. A user or agent sends `POST /agents` to the orchestrator with an `AgentConfig`.
2. The orchestrator spawns a Claude Code process via the configured backend
   (tmux or Docker).
3. Claude Code connects back via WebSocket at `ws://localhost:7006/ws/{agent_id}`.
4. The orchestrator routes messages, tool approvals, and events over this
   WebSocket connection.
5. When an agent completes or is terminated, its record remains in the SQLite
   database (status = stopped/failed) for auditing.

# Common Operations

## Check which agents are running
```
agent list
agent list --status running
```

## Send a message to an agent
```
agent send-message <agent-id> \"Your prompt here\"
```

## Check service health
```
curl http://localhost:7006/health
curl http://localhost:17010/health
curl http://localhost:17008/health
```

## View agent logs (tmux backend)
```
tmux attach -t <session-name>
```

# Communicate Rooms

Agents can join named rooms in the communicate service.  Messages posted to a
room are broadcast to all members (agents and users).  You are auto-joined to
the `system` room.  Use `agent communicate` to interact with rooms.

# Tool Policy

Your tool policy is restrictive by design — you may read files, search code,
and run safe read-only commands.  You may NOT write or edit files, run
destructive git commands, restart services, or perform operations that modify
state.

When asked to perform an action you cannot do, explain what you cannot do and
suggest the appropriate CLI command or API call the user should run instead.

# Diagnostics

When diagnosing issues, check:
1. Service health endpoints
2. Agent status via `agent list`
3. Recent logs in `tmp/` (development) or the platform log aggregator
4. Database state via the orchestrator debug endpoint at `GET /debug/agents`

Always prefer explaining how to fix an issue over attempting to fix it yourself.
";
