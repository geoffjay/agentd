# agentd-orchestrator Service

The orchestrator service is a daemon that manages AI agent processes. It spawns Claude Code instances in tmux sessions, provides a WebSocket server implementing the Claude Code SDK protocol, and exposes a REST API for agent lifecycle management and autonomous workflows.

## Base URL

```
http://127.0.0.1:17006
```

Port defaults to `17006` (dev) or `7006` (production), configurable via the `AGENTD_PORT` environment variable.

## Architecture

```
                          ┌──────────────────────────┐
                          │   agentd-orchestrator    │
                          │                          │
  curl/client ──REST──▶   │  ┌──────────┐ ┌────────┐ │
                          │  │ REST API │ │ WS API │ │
                          │  └────┬─────┘ └───┬────┘ │
                          │       │           │      │
                          │  ┌────▼───────────▼───┐  │
                          │  │   Agent Manager    │  │
                          │  │   Scheduler        │  │
                          │  └────┬───────────┬───┘  │
                          │       │           │      │
                          │  ┌────▼────┐ ┌────▼───┐  │
                          │  │ SQLite  │ │  Tmux  │  │
                          │  └─────────┘ └────┬───┘  │
                          └───────────────────┼──────┘
                                              │
                            ┌─────────────────┼─────────────────┐
                            │                 │                 │
                       ┌────▼────┐       ┌────▼────┐       ┌────▼────┐
                       │  tmux   │       │  tmux   │       │  tmux   │
                       │ session │       │ session │       │ session │
                       │ (claude)│       │ (claude)│       │ (claude)│
                       └─────────┘       └─────────┘       └─────────┘
```

Each agent runs as a Claude Code process inside a dedicated tmux session. By default, agents connect back to the orchestrator via WebSocket (`--sdk-url`) for programmatic control. Agents can also be started in interactive mode for manual use.

## Endpoints

### Health Check

```
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "agents_active": 2
}
```

`agents_active` reflects the number of agents with live WebSocket connections.

---

### List Agents

```
GET /agents?status={status}&limit={limit}&offset={offset}
```

**Query Parameters:**
- `status` (optional): Filter by status — `pending`, `running`, `stopped`, `failed`
- `limit` (optional): Page size (default: 50, max: 200)
- `offset` (optional): Number of records to skip (default: 0)

**Response:** Paginated list of agent objects.
```json
{
  "items": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "my-agent",
      "status": "running",
      "config": {
        "working_dir": "/home/user/project",
        "shell": "zsh",
        "interactive": false,
        "worktree": false,
        "tool_policy": {"mode": "allow_all"},
        "model": "sonnet",
        "env": {"ANTHROPIC_API_KEY": "***"}
      },
      "session_id": "agentd-orch-550e8400-e29b-41d4-a716-446655440000",
      "backend_type": "tmux",
      "created_at": "2026-02-28T12:00:00Z",
      "updated_at": "2026-02-28T12:00:00Z"
    }
  ],
  "total": 1,
  "limit": 50,
  "offset": 0
}
```

!!! note "Environment variable redaction"
    The `env` field in the response shows key names but values are always replaced with `"***"` to prevent secrets from leaking via the API.

---

### Create Agent

Spawn a new Claude Code agent.

```
POST /agents
Content-Type: application/json
```

**Request Body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | | Human-readable name for the agent |
| `working_dir` | string | yes | | Working directory for the agent process |
| `user` | string | no | current user | OS user to run the agent as (uses `sudo -u`) |
| `shell` | string | no | `"zsh"` | Shell to run in (`bash`, `zsh`) |
| `interactive` | bool | no | `false` | Start in interactive mode without WebSocket |
| `prompt` | string | no | | Initial prompt sent via WebSocket after agent connects |
| `worktree` | bool | no | `false` | Start with `--worktree` for isolated git worktree |
| `system_prompt` | string | no | | System prompt passed via `--system-prompt` |
| `tool_policy` | object | no | `{"mode":"allow_all"}` | Tool use restrictions (see [Tool Policy](#tool-policy)) |
| `model` | string | no | | Model to use — accepts aliases (`sonnet`, `opus`, `haiku`) or full names (`claude-sonnet-4-6`). Maps to the `--model` flag. |
| `env` | object | no | `{}` | Environment variables set when launching the agent. Commonly used for `ANTHROPIC_API_KEY`, `ANTHROPIC_BASE_URL`. Values are write-only — the API returns `"***"` in responses. |
| `additional_dirs` | array | no | `[]` | Extra directories the agent can read and write, in addition to `working_dir`. Each entry maps to a `--add-dir` flag. See [Additional Directories](../additional-dirs.md). |
| `auto_clear_threshold` | integer | no | | Automatically clear context when cumulative input tokens for the session exceeds this value. |
| `network_policy` | string | no | | Network policy for Docker-backed agents (`internet`, `isolated`, `host`). Ignored for tmux backends. |

**Response:** `201 Created` with agent object.

**Note:** The initial `prompt` is sent via the WebSocket after the agent connects, not via the `-p` CLI flag. This keeps the agent alive for follow-up messages. If no prompt is provided, the agent starts idle and waits for messages.

---

### Get Agent

```
GET /agents/{id}
```

**Response:** Single agent object (same shape as items in the List Agents response).

---

### Set Agent Model

Update the model used by an agent.

```
PUT /agents/{id}/model
Content-Type: application/json
```

**Request Body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `model` | string\|null | yes | | Model to use (e.g. `"sonnet"`, `"opus"`, `"claude-sonnet-4-6"`). Pass `null` to clear and inherit Claude Code's default. |
| `restart` | bool | no | `false` | If `true`, kill and re-launch the agent process immediately with the new model. If `false`, the change takes effect on next restart. |

**Response:** Updated agent object.

**Example:**
```bash
# Switch to opus and restart immediately
curl -X PUT http://127.0.0.1:17006/agents/<ID>/model \
  -H "Content-Type: application/json" \
  -d '{"model": "opus", "restart": true}'
```

---

### Get Agent Usage

Get token usage and cost statistics for an agent.

```
GET /agents/{id}/usage
```

**Response:**
```json
{
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "current_session": {
    "input_tokens": 12500,
    "output_tokens": 3200,
    "cache_read_input_tokens": 8000,
    "cache_creation_input_tokens": 4500,
    "total_cost_usd": 0.0184,
    "num_turns": 3,
    "duration_ms": 45230,
    "duration_api_ms": 38100,
    "result_count": 3,
    "started_at": "2026-03-10T09:00:00Z",
    "ended_at": null
  },
  "cumulative": {
    "input_tokens": 58000,
    "output_tokens": 14200,
    "cache_read_input_tokens": 32000,
    "cache_creation_input_tokens": 26000,
    "total_cost_usd": 0.0821,
    "num_turns": 12,
    "duration_ms": 198000,
    "duration_api_ms": 167000,
    "result_count": 12,
    "started_at": "2026-03-10T08:00:00Z",
    "ended_at": null
  },
  "session_count": 4
}
```

`current_session` is `null` when the agent has no active WebSocket connection.

---

### Clear Agent Context

Reset the agent's conversation context, starting a fresh session. Useful when the agent is approaching context limits or when you want to start a new task cleanly.

```
POST /agents/{id}/clear-context
Content-Type: application/json
```

**Request Body:** Empty object `{}` (reserved for future options).

**Response:**
```json
{
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "session_usage": {
    "input_tokens": 45000,
    "output_tokens": 11200,
    "total_cost_usd": 0.0637,
    "num_turns": 9,
    "result_count": 9,
    "started_at": "2026-03-10T08:00:00Z",
    "ended_at": "2026-03-10T09:30:00Z"
  },
  "new_session_number": 5
}
```

`session_usage` contains the stats for the session that was just ended. `new_session_number` is the 1-based index of the new session going forward.

!!! tip "Auto-clear threshold"
    Set `auto_clear_threshold` when creating an agent to automatically clear context when input tokens exceed the threshold, without manual intervention.

---

### Manage Additional Directories

Add or remove filesystem directories the agent can access. Each directory maps to Claude Code's `--add-dir` flag. Changes are persisted immediately but **take effect on the next agent restart**.

See [Additional Directories](../additional-dirs.md) for full details including YAML template configuration and Docker behavior.

#### Add a directory

```
POST /agents/{id}/dirs
Content-Type: application/json
```

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | yes | Absolute path to the directory. Must exist and be a directory at call time. |

**Response:** `200 OK`

```json
{
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "additional_dirs": ["/path/to/shared-libs"],
  "requires_restart": true
}
```

**Errors:**
- `404` — agent not found
- `422` — path does not exist or is not a directory

Adding a path that is already present is a no-op (idempotent).

**Example:**
```bash
curl -X POST http://127.0.0.1:17006/agents/<ID>/dirs \
  -H "Content-Type: application/json" \
  -d '{"path": "/path/to/shared-libs"}'
```

#### Remove a directory

```
DELETE /agents/{id}/dirs
Content-Type: application/json
```

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | yes | Path to remove. |

**Response:** `200 OK`

```json
{
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "additional_dirs": [],
  "requires_restart": true
}
```

**Errors:**
- `404` — agent not found

Removing a path that is not in the list is a no-op (idempotent).

**Example:**
```bash
curl -X DELETE http://127.0.0.1:17006/agents/<ID>/dirs \
  -H "Content-Type: application/json" \
  -d '{"path": "/path/to/shared-libs"}'
```

---

### Send Message to Agent

Send a prompt or follow-up message to a running SDK-mode agent. The agent must be in `running` status with an active WebSocket connection.

```
POST /agents/{id}/message
Content-Type: application/json
```

**Request Body:**
```json
{
  "content": "Your message or task description here"
}
```

**Response:**
```json
{
  "status": "sent",
  "agent_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Errors:**
- `404` if the agent doesn't exist
- `400` if the agent is not running or not connected via WebSocket

**Example — send a follow-up task to a running agent:**
```bash
curl -s -X POST http://127.0.0.1:17006/agents/{id}/message \
  -H 'Content-Type: application/json' \
  -d '{"content": "Now create issues for the documentation gaps you identified"}'
```

This is the primary way to interact with SDK-mode agents. You can send multiple messages over the agent's lifetime — each one starts a new conversation turn.

---

### Terminate Agent

Kill the agent's tmux session and mark it as stopped.

```
DELETE /agents/{id}
```

**Response:** Agent object with `"status": "stopped"`.

---

### Monitoring Streams

WebSocket endpoints for observing agent output in real time.

#### All Agents

```
ws://127.0.0.1:17006/stream
```

Receives NDJSON messages from all connected agents. Each message includes an `agent_id` field identifying the source agent.

#### Single Agent

```
ws://127.0.0.1:17006/stream/{agent_id}
```

Receives NDJSON messages from only the specified agent. Messages are filtered server-side.

**Message format:**
```json
{
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "type": "assistant",
  "content": "I'll start by reading the Cargo.toml..."
}
```

Message types include `system`, `assistant`, `result`, `control_request`, and `keep_alive`.

**Monitoring with websocat:**
```bash
# Watch all agents
websocat ws://127.0.0.1:17006/stream

# Watch a specific agent
websocat ws://127.0.0.1:17006/stream/{agent_id}
```

**Note:** Streams only deliver messages that arrive after you connect. There is no replay buffer for missed messages.

---

### Workflow Endpoints

Workflows pair a long-running agent with a trigger source. The scheduler runs the trigger strategy and dispatches tasks to the agent one at a time. See [Trigger Strategies](../trigger-strategies.md) for architecture details.

#### Create Workflow

```
POST /workflows
Content-Type: application/json
```

**Request Body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | | Unique workflow name |
| `agent_id` | UUID | yes | | Agent to dispatch tasks to (must be Running) |
| `trigger_config` | object | yes | | Trigger configuration (see below) |
| `prompt_template` | string | yes | | Template with `{{placeholders}}` for task data |
| `poll_interval_secs` | integer | no | `60` | Seconds between poll cycles (poll-based triggers only) |
| `enabled` | bool | no | `true` | Whether the workflow is active |
| `tool_policy` | object | no | `{"mode":"auto"}` | Tool policy applied when dispatching tasks |

!!! note "Backwards compatibility"
    The field name `source_config` is accepted as an alias for `trigger_config`. New integrations should use `trigger_config`.

**`trigger_config` — GitHub Issues:**
```json
{
  "type": "github_issues",
  "owner": "org-or-user",
  "repo": "repo-name",
  "labels": ["agent"],
  "state": "open"
}
```

**`trigger_config` — GitHub Pull Requests:**
```json
{
  "type": "github_pull_requests",
  "owner": "org-or-user",
  "repo": "repo-name",
  "labels": [],
  "state": "open"
}
```

**Template placeholders:** `{{title}}`, `{{body}}`, `{{url}}`, `{{labels}}`, `{{assignee}}`, `{{source_id}}`, `{{metadata}}`

**Response:** `201 Created` with workflow object.

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "name": "issue-worker",
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "trigger_config": {
    "type": "github_issues",
    "owner": "myorg",
    "repo": "myrepo",
    "labels": ["agent"],
    "state": "open"
  },
  "prompt_template": "Work on issue #{{source_id}}: {{title}}\n\n{{body}}",
  "poll_interval_secs": 60,
  "enabled": true,
  "tool_policy": { "mode": "auto" },
  "created_at": "2026-03-14T10:00:00Z",
  "updated_at": "2026-03-14T10:00:00Z"
}
```

#### List Workflows

```
GET /workflows?limit={limit}&offset={offset}
```

**Response:** Paginated list of workflow objects.

#### Get Workflow

```
GET /workflows/{id}
```

**Response:** Single workflow object.

#### Update Workflow

```
PUT /workflows/{id}
Content-Type: application/json
```

Supports partial updates: `name`, `prompt_template`, `poll_interval_secs`, `enabled`, `tool_policy`. Enabling or disabling a workflow (`"enabled": true/false`) immediately starts or stops its polling loop.

#### Delete Workflow

```
DELETE /workflows/{id}
```

**Response:** `204 No Content`

#### Dispatch History

```
GET /workflows/{id}/history?limit={limit}&offset={offset}
```

**Response:** Paginated log of dispatched tasks.

```json
{
  "items": [
    {
      "id": "...",
      "workflow_id": "...",
      "source_id": "42",
      "status": "completed",
      "prompt": "Work on issue #42: Fix login bug\n\n...",
      "dispatched_at": "2026-03-10T09:00:00Z",
      "completed_at": "2026-03-10T09:12:00Z"
    }
  ],
  "total": 7,
  "limit": 50,
  "offset": 0
}
```

Dispatch statuses: `dispatched` (in progress), `completed` (agent finished), `failed` (error or agent not available).

---

### Tool Policy

Control which tools an agent can use. Policies are set at agent creation or updated via the policy endpoint.

```
GET /agents/{id}/policy
PUT /agents/{id}/policy
```

**Policy modes:**

| Mode | JSON | Effect |
|------|------|--------|
| Allow all | `{"mode":"allow_all"}` | No restrictions (default) |
| Deny all | `{"mode":"deny_all"}` | Block all tools |
| Allow list | `{"mode":"allow_list","tools":["Read","Grep"]}` | Only listed tools |
| Deny list | `{"mode":"deny_list","tools":["Bash","Write"]}` | All except listed |
| Require approval | `{"mode":"require_approval"}` | Human must approve each tool use |

**CLI:**
```bash
agent orchestrator get-policy <ID>
agent orchestrator set-policy <ID> '{"mode":"allow_list","tools":["Read","Grep"]}'
agent orchestrator create-agent --name safe --tool-policy '{"mode":"deny_list","tools":["Bash"]}'
```

---

### Tool Approval Endpoints

When an agent runs with `require_approval` policy, tool requests are held pending until a human approves or denies them.

#### List All Approvals

```
GET /approvals?status={status}&limit={limit}&offset={offset}
```

**Query Parameters:**
- `status` (optional): Filter by status — `pending`, `approved`, `denied`, `timed_out`
- `limit` (optional): Page size (default: 50, max: 200)
- `offset` (optional): Records to skip (default: 0)

**Response:** Paginated list of approval objects.
```json
{
  "items": [
    {
      "id": "abc12345-...",
      "agent_id": "550e8400-...",
      "request_id": "req-xyz",
      "tool_name": "Bash",
      "tool_input": {"command": "cargo test"},
      "status": "pending",
      "created_at": "2026-03-10T10:00:00Z",
      "expires_at": "2026-03-10T10:05:00Z"
    }
  ],
  "total": 1,
  "limit": 50,
  "offset": 0
}
```

#### Get Approval

```
GET /approvals/{id}
```

**Response:** Single approval object.

#### Approve Tool Request

```
POST /approvals/{id}/approve
Content-Type: application/json
```

**Request Body:** (optional)
```json
{"reason": "Reviewed and approved"}
```

**Response:** Updated approval object with `"status": "approved"`.

#### Deny Tool Request

```
POST /approvals/{id}/deny
Content-Type: application/json
```

**Request Body:** (optional)
```json
{"reason": "Too risky in this context"}
```

**Response:** Updated approval object with `"status": "denied"`.

#### List Approvals for an Agent

```
GET /agents/{id}/approvals?status={status}&limit={limit}&offset={offset}
```

Same query parameters and response shape as List All Approvals, filtered to a single agent.

---

**CLI:**
```bash
agent orchestrator list-approvals
agent orchestrator list-approvals --agent-id <AGENT_ID>
agent orchestrator approve <APPROVAL_ID>
agent orchestrator deny <APPROVAL_ID>
```

Pending approvals auto-deny after 5 minutes if not acted on. Approval events are broadcast on the `/stream` WebSocket.

---

### Debug Endpoint

Provides a detailed diagnostic view of agent state, WebSocket connectivity, and active workflows in a single response. Intended for troubleshooting, not production monitoring.

```
GET /debug/agents
```

**Response:**
```json
{
  "agents": [
    {
      "id": "550e8400-...",
      "name": "worker",
      "status": "running",
      "session_id": "agentd-orch-550e8400-...",
      "ws_connected": true,
      "model": "sonnet",
      "workflows": ["wf-uuid-1"]
    }
  ],
  "orphan_connections": [],
  "summary": {
    "total_agents": 1,
    "running": 1,
    "ws_connected": 1,
    "running_but_disconnected": [],
    "connected_but_not_running": [],
    "active_workflows": 1
  }
}
```

| Field | Description |
|-------|-------------|
| `agents` | All agents in the database with their current WebSocket connection state |
| `orphan_connections` | Agent IDs that have a live WebSocket connection but no database record |
| `summary.running_but_disconnected` | Agents marked `running` in DB whose WebSocket disconnected — likely crashed |
| `summary.connected_but_not_running` | WebSocket-connected agents not marked `running` in DB — transient state |

---

### Prometheus Metrics

```
GET /metrics
```

Returns Prometheus text format metrics including `service_info`, `agents_created_total`, and `websocket_connections_active`.

---

## CLI Commands

The `agent orchestrator` subcommand provides full access to all orchestrator features:

### Streaming

Watch agent output in real-time with formatted, colored messages:

```bash
agent orchestrator stream <AGENT_ID>        # single agent
agent orchestrator stream --all             # all agents
agent orchestrator stream --all --json      # raw JSON for piping
agent orchestrator stream --all --verbose   # include keepalive/system msgs
```

Press Ctrl+C to disconnect.

### Attach

Connect to an agent's tmux session for interactive debugging:

```bash
agent orchestrator attach <AGENT_ID>
agent orchestrator attach --name my-agent
```

Verifies the agent is running and the tmux session exists before attaching.

### Send Message

Send a prompt to a running non-interactive agent:

```bash
agent orchestrator send-message <ID> "Fix the failing tests"
echo "Review the code" | agent orchestrator send-message <ID> --stdin
```

### Health

Check the orchestrator service status:

```bash
agent orchestrator health
agent orchestrator health --json
```

### Manage Additional Directories

Add or remove directories from an agent's accessible paths:

```bash
# Add a directory (must exist; takes effect on next restart)
agent orchestrator add-dir <AGENT_ID> /path/to/dir

# Remove a directory (takes effect on next restart)
agent orchestrator remove-dir <AGENT_ID> /path/to/dir
```

Both commands print the updated directory list and a restart reminder. See [Additional Directories](../additional-dirs.md) for full details.

---

### Validate Template

Check a workflow prompt template for errors:

```bash
agent orchestrator validate-template "Fix: {{title}} {{body}}"
agent orchestrator validate-template --file ./my-template.txt
```

Reports unknown variables, unclosed placeholders, and empty templates.

### Shell Completions

Generate shell completion scripts:

```bash
agent completions bash > ~/.local/share/bash-completion/completions/agent
agent completions zsh > ~/.zfunc/_agent
agent completions fish > ~/.config/fish/completions/agent.fish
```

### Service Status

Check health of all agentd services at once:

```bash
agent status
agent status --json
```

---

## Agent Modes

### SDK Mode (default)

When `interactive` is `false` (the default), the agent is launched with WebSocket connectivity:

```
claude --sdk-url ws://127.0.0.1:17006/ws/{agent_id} --print --output-format stream-json --input-format stream-json
```

The orchestrator acts as the server side of the Claude Code SDK protocol:
- Accepts WebSocket connections at `/ws/{agent_id}`
- Receives `system/init` from the claude process
- Handles `control_request` messages (tool permission requests are auto-allowed)
- Receives `assistant` responses and `result` completion messages
- Broadcasts all messages to monitoring streams at `/stream` and `/stream/{agent_id}`

SDK-mode agents stay alive after completing a task, waiting for the next message. Send follow-up work via `POST /agents/{id}/message`.

### Interactive Mode

When `interactive` is `true`, the agent is launched as a plain `claude` process without SDK flags:

```
claude
```

The user can attach to the tmux session and interact with Claude Code directly. Interactive agents cannot receive messages via the REST API.

Both modes support `--worktree` and `--system-prompt` when the corresponding options are provided.

## Tmux Sessions

Each agent runs in a tmux session named `agentd-orch-{agent_id}`. You can:

**List agent sessions:**
```bash
tmux list-sessions | grep agentd-orch
```

**Attach to an agent session:**
```bash
tmux attach -t agentd-orch-{agent_id}
```

**Detach from a session:**
Press `Ctrl-b d`.

## Usage Examples

### Using YAML templates (recommended)

The simplest way to launch agents and workflows:

```bash
# Apply a project directory (agents first, then workflows)
agent apply .agentd/

# Or apply individual templates
agent apply .agentd/agents/worker.yml
agent apply .agentd/workflows/issue-worker.yml

# Validate without creating
agent apply --dry-run .agentd/

# Tear down everything
agent teardown .agentd/
```

### Create an agent and send it work

```bash
# Create an agent using the CLI
agent orchestrator create-agent \
  --name planner \
  --prompt "Analyze the codebase and propose improvements" \
  --add-dir /path/to/shared-libs \
  --add-dir /opt/configs

# Monitor the output in real-time
agent orchestrator stream --all

# Send follow-up work
agent orchestrator send-message <ID> "Now create issues for the gaps you found"

# Attach to the tmux session for interactive debugging
agent orchestrator attach --name planner
```

### Create a workflow with tool restrictions

```bash
# Create a read-only code review workflow
agent orchestrator create-workflow \
  --name code-review \
  --agent-name planner \
  --owner myorg --repo myrepo \
  --labels "review" \
  --prompt-template "Review: {{title}}\n{{body}}" \
  --tool-policy '{"mode":"allow_list","tools":["Read","Grep","Glob"]}'

# Validate template before creating
agent orchestrator validate-template "Fix: {{title}} {{body}}"
```

### Using the REST API directly

```bash
# Create an agent
curl -X POST http://127.0.0.1:17006/agents \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent", "working_dir": "/path/to/project"}'

# Send a message
curl -X POST http://127.0.0.1:17006/agents/<ID>/message \
  -H "Content-Type: application/json" \
  -d '{"content": "Analyze the codebase"}'

# Set a tool policy
curl -X PUT http://127.0.0.1:17006/agents/<ID>/policy \
  -H "Content-Type: application/json" \
  -d '{"mode":"deny_list","tools":["Bash","Write"]}'
```

### Common operations

```bash
# Check all services
agent status

# List running agents
agent orchestrator list-agents --status running

# Check orchestrator health
agent orchestrator health

# Terminate an agent
agent orchestrator delete-agent <ID>

# View workflow dispatch history
agent orchestrator workflow-history <ID>

# List pending tool approvals
agent orchestrator list-approvals
```

## Startup Reconciliation

When the orchestrator starts, it reconciles database state with actual tmux sessions. Any agent marked as `running` in the database whose tmux session no longer exists is automatically marked as `failed`. In-flight workflow dispatches from a previous run are marked as `failed` and polling resumes for enabled workflows with connected agents.

## Storage

Agent and workflow records are stored in SQLite at:
```
~/Library/Application Support/agentd-orchestrator/orchestrator.db
```

## Running the Service

```bash
# Development (with live reload)
watchexec -r -e rs -w crates/orchestrator cargo run -p agentd-orchestrator

# Direct
cargo run -p agentd-orchestrator

# With debug logging
RUST_LOG=debug cargo run -p agentd-orchestrator

# Custom port
PORT=8080 cargo run -p agentd-orchestrator
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENTD_PORT` | `17006` | HTTP/WebSocket listen port |
| `RUST_LOG` | `info` | Log level (`debug`, `info`, `warn`, `error`) |
