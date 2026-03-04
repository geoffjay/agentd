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

```
GET  /approvals                    # list all pending approvals
GET  /approvals/{id}               # get single approval
POST /approvals/{id}/approve       # approve tool request
POST /approvals/{id}/deny          # deny tool request
GET  /agents/{id}/approvals        # list approvals for an agent
```

**CLI:**
```bash
agent orchestrator list-approvals
agent orchestrator approve <APPROVAL_ID>
agent orchestrator deny <APPROVAL_ID>
```

Pending approvals auto-deny after 5 minutes if not acted on. Approval events are broadcast on the `/stream` WebSocket.

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
  --prompt "Analyze the codebase and propose improvements"

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
| `PORT` | `17006` | HTTP/WebSocket listen port |
| `RUST_LOG` | `info` | Log level (`debug`, `info`, `warn`, `error`) |
