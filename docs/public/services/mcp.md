# agentd-mcp Service

The MCP (Model Context Protocol) server exposes agentd's agent management, workflow, notification, approval, and diagnostic services as tools for Claude and other MCP clients. It enables a self-healing loop: Claude can query system state, diagnose problems, and take corrective action through structured MCP tool calls.

Unlike other agentd services, agentd-mcp is not an HTTP daemon - it communicates over **stdio** using the MCP JSON-RPC transport and acts as a stateless bridge to the rest of the agentd fleet.

## Architecture

```
MCP Client (Claude Code, Claude Desktop)
     │ JSON-RPC over stdio
     ▼
agentd-mcp
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

Each tool call makes direct HTTP requests to the relevant agentd services. No local state is maintained between calls.

## Quick Start

```bash
# Run via the agent CLI (works from any directory)
agent mcp

# Or run the standalone binary
cargo run -p agentd-mcp

# Run with MCP Inspector for development
npx @modelcontextprotocol/inspector agent mcp
```

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

## Configuration

All configuration is via environment variables. Defaults target the standard localhost ports used by agentd services.

| Variable | Default | Service |
|---|---|---|
| `AGENTD_ORCHESTRATOR_URL` | `http://127.0.0.1:17006` | Orchestrator |
| `AGENTD_COMMUNICATE_URL` | `http://127.0.0.1:17010` | Communicate |
| `AGENTD_MEMORY_URL` | `http://127.0.0.1:17008` | Memory |
| `AGENTD_NOTIFY_URL` | `http://127.0.0.1:17004` | Notify |
| `AGENTD_ASK_URL` | `http://127.0.0.1:17001` | Ask |
| `AGENTD_WRAP_URL` | `http://127.0.0.1:17005` | Wrap |
| `AGENTD_MONITOR_URL` | `http://127.0.0.1:17003` | Monitor |
| `AGENTD_HOOK_URL` | `http://127.0.0.1:17002` | Hook |
| `RUST_LOG` | `info` | Log level |

## Tool Reference

All tools return markdown-formatted strings with severity tagging where applicable: 🔴 Critical, 🟡 Warning, 🟢 Info.

---

### System Diagnostics

#### `diagnose_system`

Run a full system diagnostic: check all agentd services, identify failed agents, surface monitor alerts, count pending approvals and notifications. Returns a prioritized report. Tolerates partial service unavailability.

**Parameters:** None

**Example output:**
```
# System Diagnostic Report

## Service Health
| Service | Status |
|---|---|
| orchestrator | 🟢 Healthy |
| notify | 🟢 Healthy |
| monitor | 🔴 Unreachable |

## Issues (prioritized)
🔴 2 agents in failed state: worker-1, worker-2
🟡 5 pending approval requests blocking agents
🟢 30 notifications in backlog (12 low-priority, older than 48h)
```

---

#### `diagnose_agent`

Deep dive on a single agent: status, activity state, WebSocket connection, pending approval backlog, and session usage. Returns a structured report with severity-tagged issues and actionable remediation steps.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `agent_id` | String | Yes | The agent ID (UUID) to diagnose |

---

#### `diagnose_workflow`

Workflow health analysis: verify the associated agent is running, analyze dispatch success rate over the last 20 dispatches, and identify consecutive failure patterns.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `workflow_id` | String | Yes | The workflow ID (UUID) to diagnose |

---

#### `check_connectivity`

Test connectivity between the MCP server and all agentd services. Returns a table showing which services are reachable and which are not.

**Parameters:** None

---

### Agent Inspection

#### `list_agents`

List all agents managed by agentd, optionally filtered by status. Returns a table with agent ID, name, status, and activity state.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `status` | String | No | Filter by status: `pending`, `running`, `stopped`, `failed`. Omit for all. |

---

#### `get_agent`

Get detailed information about a specific agent including configuration, tool policy, model, working directory, and environment variable keys (values are redacted).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `agent_id` | String | Yes | The agent ID (UUID) to inspect |

---

#### `get_agent_status_summary`

Fleet-wide status counts of pending, running, stopped, and failed agents. Lists any failed agents with their IDs and names for quick identification.

**Parameters:** None

---

### Workflow & Dispatch Inspection

#### `list_workflows`

List all configured workflows with trigger type, poll interval, enabled state, and associated agent.

**Parameters:** None

---

#### `get_workflow`

Full configuration of a workflow including trigger source config, prompt template, and tool policy.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `workflow_id` | String | Yes | The workflow ID (UUID) to inspect |

---

#### `list_dispatches`

Dispatch records for a specific workflow showing task execution history with status and timing.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `workflow_id` | String | Yes | The workflow ID (UUID) |
| `status` | String | No | Filter: `pending`, `dispatched`, `completed`, `failed`, `skipped`. Omit for all. |
| `limit` | Integer | No | Maximum records to return (default: 20, max: 200) |

---

#### `get_failed_dispatches`

All failed dispatch records across all workflows, sorted by most recent first.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `limit` | Integer | No | Maximum records to return (default: 50, max: 200) |

---

### Notification Management

#### `list_notifications`

List notifications with optional filters for status and priority. Sorted by priority (highest first).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `status` | String | No | Filter: `pending`, `viewed`, `responded`, `dismissed`, `expired` |
| `priority` | String | No | Filter: `low`, `normal`, `high`, `urgent` |
| `limit` | Integer | No | Maximum to return (default: 20, max: 200) |

---

#### `get_notification`

Full details of a specific notification including source data, message body, and response.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `notification_id` | String | Yes | The notification ID (UUID) |

---

#### `get_actionable_notifications`

All notifications that are pending or viewed and have not expired, sorted by priority (urgent first).

**Parameters:** None

---

#### `create_notification`

Create a system notification for flagging issues found during diagnostics or remediation.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `title` | String | Yes | Short title for the notification |
| `message` | String | Yes | Detailed message body with diagnostic context |
| `priority` | String | No | Priority level: `low`, `normal`, `high`, `urgent` (default: `normal`) |

---

#### `dismiss_notification`

Dismiss a notification, marking it as reviewed and removing it from the active backlog.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `notification_id` | String | Yes | The notification ID (UUID) to dismiss |

---

### Approval Management

#### `list_pending_approvals`

All pending tool approval requests across all agents. Shows tool name, input summary, requesting agent, and expiry time.

**Parameters:** None

---

#### `get_agent_approvals`

Pending tool approval requests for a specific agent. Useful when diagnosing why an agent appears blocked.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `agent_id` | String | Yes | The agent ID (UUID) |

---

#### `approve_tool_request`

Approve a pending tool use request, allowing the agent to proceed.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `approval_id` | String | Yes | The approval request ID (UUID) |

---

#### `deny_tool_request`

Deny a pending tool use request. An optional reason is sent back to the agent.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `approval_id` | String | Yes | The approval request ID (UUID) |
| `reason` | String | No | Reason for denial, sent back to the agent |

---

### Agent Lifecycle Management

#### `restart_agent`

Restart an agent by terminating the current session and recreating it with the same configuration. Loses all in-flight work. Use on failed or stopped agents. Returns the new agent ID.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `agent_id` | String | Yes | The agent ID (UUID) to restart |

---

#### `send_agent_message`

Send a message or prompt to a running agent via the orchestrator. The agent processes the message in its current session context.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `agent_id` | String | Yes | The agent ID (UUID) |
| `message` | String | Yes | The message content or prompt to send |

---

#### `update_agent_tool_policy`

Change an agent's tool policy to restrict or allow tool usage.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `agent_id` | String | Yes | The agent ID (UUID) |
| `mode` | String | Yes | Policy mode: `allow_all`, `deny_all`, `require_approval`, `allow_list`, `deny_list` |
| `tools` | String[] | No | Tool name patterns for `allow_list` or `deny_list` modes (e.g., `["Bash", "Write"]`) |

---

#### `terminate_agent`

Permanently terminate an agent. Kills the tmux session and removes the agent from the registry. All in-flight work is permanently lost.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `agent_id` | String | Yes | The agent ID (UUID) to terminate |

---

#### `update_agent_model`

Change the AI model an agent is using. Takes effect for subsequent turns in the current session.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `agent_id` | String | Yes | The agent ID (UUID) |
| `model` | String | Yes | The new model identifier (e.g., `claude-opus-4-5`, `claude-sonnet-4-5`) |

---

### Self-Healing Remediation

#### `restart_failed_agents`

Find all agents in a failed state and restart them. For each: captures config, terminates the old session, and recreates the agent. Returns an audit report. Only targets agents already in failed state.

**Parameters:** None

---

#### `retry_failed_dispatches`

Retry failed dispatch records for a workflow by re-sending their prompts to the associated agent.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `workflow_id` | String | Yes | The workflow ID (UUID) |
| `hours` | Integer | No | Only retry dispatches that failed within this many hours (default: 24) |

---

#### `cleanup_stale_dispatches`

Identify dispatch records stuck in "dispatched" state longer than the staleness threshold. This is a reporting-only tool - use `restart_agent` on the associated agent to unblock.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `stale_hours` | Integer | No | Consider dispatches stale after this many hours (default: 2) |

---

#### `auto_approve_safe_tools`

Automatically approve pending tool requests that match a conservative safe list of read-only tools (Read, Glob, Grep, ListFiles, WebFetch, etc.). Non-matching requests are skipped and reported.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `additional_safe_tools` | String[] | No | Additional tool names to consider safe (e.g., `["LSP", "TaskOutput"]`) |

---

#### `resolve_notification_backlog`

Bulk-dismiss pending notifications that are no longer actionable: expired ephemeral notifications and low-priority notifications older than the threshold.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `hours` | Integer | No | Dismiss low-priority notifications older than this many hours (default: 48) |

---

### Service Health & Metrics

#### `check_service_health`

Concurrent health check of all 8 agentd services. Returns a table with status, response time, and URL for each. Uses a 3-second timeout per service.

**Parameters:** None

---

#### `check_single_service`

Health check for a specific agentd service by name.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `service` | String | Yes | Service name: `orchestrator`, `communicate`, `memory`, `notify`, `ask`, `wrap`, `monitor`, `hook` |

---

#### `get_system_metrics`

Current system metrics from the monitor service: CPU usage, memory, disk usage, and load average. Includes active alerts if any thresholds are exceeded.

**Parameters:** None

---

#### `get_prometheus_metrics`

Fetch and parse key Prometheus counters and gauges from a service. Supports orchestrator (agents, WebSocket, approvals) and notify (notification counts).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `service` | String | No | Service to fetch from: `orchestrator`, `notify` (default: `orchestrator`) |

---

## Troubleshooting Workflows

### Agent Not Responding

```
1. check_service_health          → verify orchestrator is reachable
2. list_agents status=running    → confirm agent is registered
3. diagnose_agent {id}           → identify root cause (approvals? usage?)
4. get_agent_approvals {id}      → check for pending approval blocks
5. approve_tool_request {id}     → unblock if approval-gated
   - or -
6. restart_agent {id}            → restart if crashed/failed
```

### Workflow Not Dispatching

```
1. list_workflows                → check enabled=true, find agent_id
2. get_agent {agent_id}          → verify agent status=running
3. diagnose_workflow {id}        → analyze dispatch success rate
4. list_dispatches {id} status=failed  → inspect failure details
5. retry_failed_dispatches {id}  → re-send failed prompts
   - or -
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
   - or -
5. resolve_notification_backlog hours=24  → bulk-dismiss old low-priority
```

## Self-Healing Automation

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
     → "All systems healthy"
```

### Safety Principles

- **Destructive tools** (`restart_agent`, `terminate_agent`, `restart_failed_agents`) only operate on agents already in a terminal/failed state.
- **`auto_approve_safe_tools`** uses a conservative default list of read-only tools (Read, Glob, Grep, WebFetch, etc.). Additional tools require explicit opt-in.
- **`cleanup_stale_dispatches`** is reporting-only - it identifies stuck dispatches but does not update their status. Use `restart_agent` on the associated agent to unblock.
- All remediation tools produce detailed audit reports of every action taken.

### Escalation Pattern

When automated remediation is insufficient:

```
1. create_notification title="Agent fleet degraded" message="..." priority=urgent
2. → Notification appears in agentd dashboard for human review
3. Human reviews, takes manual action
4. dismiss_notification {id}  → mark as resolved
```

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

Integration tests in `tests/` use lightweight axum mock servers - no running agentd services are required.
