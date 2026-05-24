# agentd-mcp

MCP (Model Context Protocol) server that exposes agentd's agent management,
workflow, notification, approval, and diagnostic services as tools for Claude
and other MCP clients.

## What is agentd-mcp?

agentd-mcp acts as a bridge between your MCP client (Claude Code, Claude
Desktop, etc.) and the agentd service fleet. Once registered, Claude can
directly inspect agents, diagnose failures, manage approvals, and trigger
self-healing remediation — all without leaving the conversation.

## Quick Start

```bash
# Run via the agent CLI (works from any directory)
agent mcp

# Or run the standalone binary directly
cargo run -p agentd-mcp
```

The server communicates over **stdio** using the MCP JSON-RPC transport.
All log output goes to **stderr** so it does not interfere with the protocol.

## MCP Client Configuration

### Claude Code (`.claude/mcp.json`)

```json
{
  "mcpServers": {
    "agentd": {
      "command": "agent",
      "args": ["mcp"]
    }
  }
}
```

Environment variable overrides can be passed if your services run on non-default ports:

```json
{
  "mcpServers": {
    "agentd": {
      "command": "agent",
      "args": ["mcp"],
      "env": {
        "AGENTD_ORCHESTRATOR_URL": "http://127.0.0.1:17006",
        "AGENTD_NOTIFY_URL": "http://127.0.0.1:17004",
        "AGENTD_MONITOR_URL": "http://127.0.0.1:17003"
      }
    }
  }
}
```

### Claude Desktop (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "agentd": {
      "command": "agent",
      "args": ["mcp"]
    }
  }
}
```

### Development Mode (with MCP Inspector)

```bash
npx @modelcontextprotocol/inspector agent mcp
```

## Configuration

All configuration is via environment variables. Defaults target the standard
localhost ports used by `agentd` services.

| Variable                   | Default                    | Service        |
|----------------------------|----------------------------|----------------|
| `AGENTD_ORCHESTRATOR_URL`  | `http://127.0.0.1:17006`  | Orchestrator   |
| `AGENTD_COMMUNICATE_URL`   | `http://127.0.0.1:17010`  | Communicate    |
| `AGENTD_MEMORY_URL`        | `http://127.0.0.1:17008`  | Memory         |
| `AGENTD_NOTIFY_URL`        | `http://127.0.0.1:17004`  | Notify         |
| `AGENTD_ASK_URL`           | `http://127.0.0.1:17001`  | Ask            |
| `AGENTD_WRAP_URL`          | `http://127.0.0.1:17005`  | Wrap           |
| `AGENTD_MONITOR_URL`       | `http://127.0.0.1:17003`  | Monitor        |
| `AGENTD_HOOK_URL`          | `http://127.0.0.1:17002`  | Hook           |
| `RUST_LOG`                 | `info`                     | Log level      |

## Available Tools

### System Diagnostics
| Tool | Description |
|------|-------------|
| `diagnose_system` | Full system overview: services, failed agents, alerts, backlogs |
| `diagnose_agent` | Deep dive on a single agent: status, activity, approvals, usage, backend-specific health (docker/subprocess/pty/tmux) |
| `diagnose_workflow` | Workflow health: agent state, dispatch success rate, trigger-type-specific notes (cron, webhook, agent_lifecycle, dispatch_result, agent_idle, etc.) |
| `diagnose_state_mismatches` | Detect orphan WebSocket connections, agents running-but-disconnected, and connected-but-not-running. Highest-value tool for catching subtle stuckness. |
| `inspect_queue` | Stats and peek for a named orchestrator queue (pending, processing, failed, dead) |
| `get_conversation_summary` | Per-event-type counts and time range for an agent's history — cheap productivity check |

### Agent Inspection
| Tool | Description |
|------|-------------|
| `list_agents` | List all agents, optionally filtered by status |
| `get_agent` | Full agent details: config, tool policy, model, env keys |
| `get_agent_status_summary` | Fleet-wide status counts; lists all failed agents |

### Workflow & Dispatch Inspection
| Tool | Description |
|------|-------------|
| `list_workflows` | List all workflows with trigger type and enabled state |
| `get_workflow` | Full workflow config including prompt template and source config |
| `list_dispatches` | Dispatch history for a workflow, with optional status filter |
| `get_failed_dispatches` | Failed dispatches across all workflows |

### Communicate: Rooms & Messages
| Tool | Description |
|------|-------------|
| `list_rooms` | List rooms with optional filters for type (direct/group/broadcast) and project_id |
| `get_room` | Room metadata plus full participant list |
| `list_messages` | Recent messages in a room with sender, status, and content preview |
| `send_room_message` | Post a message to a room (for diagnostic and remediation flows) |

### Memory
| Tool | Description |
|------|-------------|
| `search_memories` | Semantic search across stored memories with optional tag and type filters |
| `list_memories` | List memories with filters for type, tag, creator, and visibility |
| `get_memory` | Fetch a single memory's full content and metadata |

### Projects
| Tool | Description |
|------|-------------|
| `list_projects` | List projects grouping agents and workflows |
| `get_project` | Project detail including agent and workflow counts |

### Notification Management
| Tool | Description |
|------|-------------|
| `list_notifications` | List with optional status/priority filters |
| `get_notification` | Full notification details and response |
| `get_actionable_notifications` | Pending notifications requiring attention |
| `create_notification` | Create a system notification (for diagnostic findings) |
| `dismiss_notification` | Dismiss a notification by ID |

### Approval Management
| Tool | Description |
|------|-------------|
| `list_pending_approvals` | All pending tool approval requests across agents |
| `get_agent_approvals` | Pending approvals for a specific agent |
| `approve_tool_request` | Approve a pending tool use request |
| `deny_tool_request` | Deny a pending tool use request with optional reason |

### Agent Lifecycle Management
| Tool | Description |
|------|-------------|
| `restart_agent` | ⚠️ Restart a specific agent (terminates current session) |
| `send_agent_message` | Send a prompt/message to a running agent |
| `update_agent_tool_policy` | Change an agent's tool policy |
| `terminate_agent` | ⚠️ Permanently terminate an agent |
| `update_agent_model` | Change the AI model an agent is using |

### Self-Healing Remediation
| Tool | Description |
|------|-------------|
| `restart_failed_agents` | ⚠️ Find and restart all failed agents |
| `retry_failed_dispatches` | Re-send prompts for failed dispatches within a time window |
| `cleanup_stale_dispatches` | Identify dispatches stuck in "dispatched" state |
| `auto_approve_safe_tools` | Batch-approve read-only tool requests |
| `resolve_notification_backlog` | Bulk-dismiss expired/old low-priority notifications |

### Service Health & Metrics
| Tool | Description |
|------|-------------|
| `check_service_health` | Concurrent health check of all 8 agentd services |
| `check_single_service` | Health check for one named service |
| `get_system_metrics` | CPU, memory, disk, load from the monitor service |
| `get_prometheus_metrics` | Parse key Prometheus counters from any service (orchestrator, notify, memory, communicate, monitor, ask, wrap, hook) |

## Troubleshooting Workflows

### Agent Not Responding

```
1. check_service_health          → verify orchestrator is reachable
2. list_agents status=running    → confirm agent is registered
3. diagnose_agent {id}           → identify root cause (approvals? backend session dead?)
4. diagnose_state_mismatches     → check for running-but-disconnected agents
5. get_agent_approvals {id}      → check for pending approval blocks
6. approve_tool_request {id}     → unblock if approval-gated
   — or —
7. restart_agent {id}            → restart if crashed/failed
```

### Workflow Not Dispatching

```
1. list_workflows                → check enabled=true, find agent_id
2. get_agent {agent_id}          → verify agent status=running
3. diagnose_workflow {id}        → analyze dispatch success rate
4. list_dispatches {id} status=failed  → inspect failure details
5. retry_failed_dispatches {id}  → re-send failed prompts
   — or —
6. cleanup_stale_dispatches      → identify and unblock stuck dispatches
```

### Full System Health Check

```
1. diagnose_system               → get prioritized issue list (🔴/🟡/🟢)
2. restart_failed_agents         → fix failed agents
3. auto_approve_safe_tools       → unblock approval-gated agents
4. cleanup_stale_dispatches      → identify stuck workflow dispatches
5. resolve_notification_backlog  → clear expired notifications
6. diagnose_system               → verify issues resolved
```

### Notification Backlog Growing

```
1. get_actionable_notifications  → see what needs attention
2. list_notifications priority=urgent  → prioritize urgent items
3. get_notification {id}         → read full details
4. dismiss_notification {id}     → clear resolved items
   — or —
5. resolve_notification_backlog hours=24  → bulk-dismiss old low-priority
```

## Self-Healing Automation Guide

agentd-mcp is designed for the **diagnose → remediate → verify** loop:

```
Claude:
  1. diagnose_system
     → "2 failed agents, 5 stale dispatches, 30-notification backlog"
  2. restart_failed_agents
     → "Restarted: worker-agent-1, worker-agent-2"
  3. cleanup_stale_dispatches stale_hours=1
     → "5 dispatches stuck for >1h identified"
  4. resolve_notification_backlog hours=48
     → "Dismissed 25 expired/low-priority notifications"
  5. diagnose_system
     → "✅ All services healthy, 0 failed agents"
```

### Safety Principles

- **Destructive tools** (`restart_agent`, `terminate_agent`,
  `restart_failed_agents`) are marked ⚠️ in their descriptions and only
  operate on agents already in a terminal/failed state.
- **`auto_approve_safe_tools`** uses a conservative default list of read-only
  tools (Read, Glob, Grep, WebFetch, etc.). Additional tools require explicit
  opt-in via the `additional_safe_tools` parameter.
- **`cleanup_stale_dispatches`** is a reporting-only tool — it identifies
  stuck dispatches but cannot update their status directly (the orchestrator
  API does not expose a dispatch-update endpoint). Use `restart_agent` on
  the associated agent to unblock.
- All remediation tools produce detailed audit reports of every action taken.

### Escalation Pattern

When automated remediation is insufficient:

```
1. create_notification title="Agent fleet degraded" message="..." priority=urgent
2. → Notification appears in agentd dashboard for human review
3. Human reviews, takes manual action
4. dismiss_notification {id}  → mark as resolved
```

## Architecture

```
MCP Client (Claude)
     │ JSON-RPC over stdio
     ▼
agentd-mcp (this crate)
     │ HTTP/REST
     ├── orchestrator  :17006  (agents, workflows, approvals)
     ├── communicate   :17010  (rooms, messages)
     ├── memory        :17008  (vector memory store)
     ├── notify        :17004  (notifications)
     ├── ask           :17001  (approval requests)
     ├── wrap          :17005  (Docker wrap configs)
     ├── monitor       :17003  (system metrics, alerts)
     └── hook          :17002  (pre/post tool hooks)
```

The server is stateless — each tool call makes direct HTTP requests to the
relevant agentd services. No local state is maintained between calls.

## Development

```bash
# Run all tests (unit + integration)
cargo test -p agentd-mcp

# Run with verbose logging
RUST_LOG=debug cargo run -p agentd-mcp

# Format and lint
cargo fmt -p agentd-mcp
cargo clippy -p agentd-mcp
```

Integration tests in `tests/` use lightweight axum mock servers — no running
agentd services are required.
