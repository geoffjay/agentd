# agentd-orchestrator Service

The orchestrator service is a daemon that manages AI agent processes. It spawns Claude Code instances in tmux sessions, provides a WebSocket server implementing the Claude Code SDK protocol, and exposes a REST API for agent lifecycle management and autonomous workflows.

## Base URL

```
http://127.0.0.1:17006
```

Port defaults to `17006` (dev) or `7006` (production), configurable via the `PORT` environment variable.

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
GET /agents?status={status}
```

**Query Parameters:**
- `status` (optional): Filter by status - `pending`, `running`, `stopped`, `failed`

**Response:**
```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "my-agent",
    "status": "running",
    "config": {
      "working_dir": "/home/user/project",
      "shell": "zsh",
      "interactive": false,
      "worktree": false
    },
    "tmux_session": "agentd-orch-550e8400-e29b-41d4-a716-446655440000",
    "created_at": "2026-02-28T12:00:00Z",
    "updated_at": "2026-02-28T12:00:00Z"
  }
]
```

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
| `system_prompt` | string | no | | System prompt (`--system-prompt`) |

**Response:** `201 Created` with agent object.

**Note:** The initial `prompt` is sent via the WebSocket after the agent connects, not via the `-p` CLI flag. This keeps the agent alive for follow-up messages. If no prompt is provided, the agent starts idle and waits for messages.

---

### Get Agent

```
GET /agents/{id}
```

**Response:** Single agent object.

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

Workflows pair a long-running agent with a GitHub issue source. The scheduler polls for new issues and dispatches them to the agent one at a time.

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
| `source_config` | object | yes | | Task source configuration (see below) |
| `prompt_template` | string | yes | | Template with `{{placeholders}}` for task data |
| `poll_interval_secs` | integer | no | `60` | Seconds between poll cycles |
| `enabled` | bool | no | `true` | Whether the workflow is active |

**Source config (GitHub Issues):**
```json
{
  "type": "github_issues",
  "owner": "org-or-user",
  "repo": "repo-name",
  "labels": ["agent"],
  "state": "open"
}
```

**Template placeholders:** `{{title}}`, `{{body}}`, `{{url}}`, `{{labels}}`, `{{assignee}}`, `{{source_id}}`, `{{metadata}}`

**Response:** `201 Created` with workflow object.

#### List Workflows

```
GET /workflows
```

#### Get Workflow

```
GET /workflows/{id}
```

#### Update Workflow

```
PUT /workflows/{id}
Content-Type: application/json
```

Supports partial updates: `name`, `prompt_template`, `poll_interval_secs`, `enabled`.

#### Delete Workflow

```
DELETE /workflows/{id}
```

#### Dispatch History

```
GET /workflows/{id}/history
```

Returns the log of dispatched tasks with their status (`dispatched`, `completed`, `failed`).

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

### Create an agent and send it work

```bash
# Create a long-running agent
AGENT=$(curl -s -X POST http://127.0.0.1:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "planner",
    "working_dir": "/home/user/project",
    "system_prompt": "You are a project planning agent."
  }')
AGENT_ID=$(echo "$AGENT" | jq -r '.id')

# Wait a moment for it to connect, then send a task
sleep 10
curl -s -X POST "http://127.0.0.1:17006/agents/$AGENT_ID/message" \
  -H 'Content-Type: application/json' \
  -d '{"content": "Analyze the codebase and propose improvements"}'

# Monitor the output
websocat "ws://127.0.0.1:17006/stream/$AGENT_ID"
```

### Send follow-up messages to a running agent

```bash
# After the agent completes its first task, send another
curl -s -X POST "http://127.0.0.1:17006/agents/$AGENT_ID/message" \
  -H 'Content-Type: application/json' \
  -d '{"content": "Now create GitHub issues for each improvement you identified"}'

# Send a correction mid-task
curl -s -X POST "http://127.0.0.1:17006/agents/$AGENT_ID/message" \
  -H 'Content-Type: application/json' \
  -d '{"content": "Use the label enhancement instead of improvement for those issues"}'
```

### Create a workflow to process GitHub issues

```bash
# First create a worker agent (no initial prompt — workflow sends tasks)
AGENT=$(curl -s -X POST http://127.0.0.1:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "worker",
    "working_dir": "/home/user/project",
    "system_prompt": "You are a worker agent. Implement the GitHub issue described in each task."
  }')
AGENT_ID=$(echo "$AGENT" | jq -r '.id')

# Wait for agent to connect
sleep 10

# Create workflow that polls for issues labeled "agent"
curl -s -X POST http://127.0.0.1:17006/workflows \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"issue-worker\",
    \"agent_id\": \"$AGENT_ID\",
    \"source_config\": {
      \"type\": \"github_issues\",
      \"owner\": \"myorg\",
      \"repo\": \"myrepo\",
      \"labels\": [\"agent\"],
      \"state\": \"open\"
    },
    \"prompt_template\": \"Work on issue #{{source_id}}: {{title}}\n\n{{body}}\n\nURL: {{url}}\",
    \"poll_interval_secs\": 60
  }"
```

### Spawn an interactive agent

```bash
curl -X POST http://127.0.0.1:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "debug-session",
    "working_dir": "/home/user/project",
    "interactive": true
  }'

# Then attach to work interactively
tmux attach -t agentd-orch-{id}
```

### Useful commands

```bash
# List running agents
curl -s http://127.0.0.1:17006/agents?status=running | jq

# Check health
curl -s http://127.0.0.1:17006/health | jq

# Terminate an agent
curl -X DELETE http://127.0.0.1:17006/agents/{id}

# Pause a workflow
curl -s -X PUT http://127.0.0.1:17006/workflows/{id} \
  -H 'Content-Type: application/json' -d '{"enabled": false}'

# View dispatch history
curl -s http://127.0.0.1:17006/workflows/{id}/history | jq
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
| `PORT` | `17006` | HTTP/WebSocket listen port |
| `RUST_LOG` | `info` | Log level (`debug`, `info`, `warn`, `error`) |
