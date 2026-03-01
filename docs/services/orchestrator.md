# agentd-orchestrator Service

The orchestrator service is a daemon that manages AI agent processes. It spawns Claude Code instances in tmux sessions, provides a WebSocket server implementing the Claude Code SDK protocol, and exposes a REST API for agent lifecycle management.

## Base URL

```
http://127.0.0.1:17006
```

Port defaults to `17006` (dev) or `7006` (production), configurable via the `PORT` environment variable.

## Architecture

```
                          ┌─────────────────────────┐
                          │   agentd-orchestrator    │
                          │                          │
  curl/client ──REST──▶   │  ┌─────────┐ ┌────────┐ │
                          │  │ REST API │ │ WS API │ │
                          │  └────┬─────┘ └───┬────┘ │
                          │       │           │      │
                          │  ┌────▼───────────▼───┐  │
                          │  │   Agent Manager    │  │
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
| `prompt` | string | no | | Initial prompt to execute (`-p`) |
| `worktree` | bool | no | `false` | Start with `--worktree` for isolated git worktree |
| `system_prompt` | string | no | | System prompt (`--system-prompt`) |

**Minimal request:**
```json
{
  "name": "my-agent",
  "working_dir": "/path/to/project"
}
```

**Full request:**
```json
{
  "name": "code-reviewer",
  "working_dir": "/home/user/project",
  "user": "deploy",
  "shell": "bash",
  "interactive": false,
  "prompt": "Review the latest commit for security issues",
  "worktree": true,
  "system_prompt": "You are a security-focused code reviewer."
}
```

**Response:** `201 Created` with agent object.

---

### Get Agent

```
GET /agents/{id}
```

**Response:** Single agent object.

---

### Terminate Agent

Kill the agent's tmux session and mark it as stopped.

```
DELETE /agents/{id}
```

**Response:** Agent object with `"status": "stopped"`.

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
- Maintains keep-alive with 10-second ping/pong

### Interactive Mode

When `interactive` is `true`, the agent is launched as a plain `claude` process without SDK flags:

```
claude
```

The user can attach to the tmux session and interact with Claude Code directly. This is useful for:
- Manual prompt execution and debugging
- Exploratory work where programmatic control isn't needed
- Troubleshooting agent behavior

Both modes support `--worktree`, `--system-prompt`, and `-p` flags when the corresponding options are provided.

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

## Startup Reconciliation

When the orchestrator starts, it reconciles database state with actual tmux sessions. Any agent marked as `running` in the database whose tmux session no longer exists is automatically marked as `failed`.

## Storage

Agent records are stored in SQLite at:
```
~/Library/Application Support/agentd-orchestrator/orchestrator.db
```

### Schema

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT (PK) | Agent UUID |
| `name` | TEXT | Agent name |
| `status` | TEXT | `pending`, `running`, `stopped`, `failed` |
| `working_dir` | TEXT | Working directory |
| `user` | TEXT | OS user (nullable) |
| `shell` | TEXT | Shell (`bash`, `zsh`) |
| `interactive` | INTEGER | 0 = SDK mode, 1 = interactive |
| `prompt` | TEXT | Initial prompt (nullable) |
| `worktree` | INTEGER | 0 = normal, 1 = worktree mode |
| `system_prompt` | TEXT | System prompt (nullable) |
| `tmux_session` | TEXT | Tmux session name (nullable) |
| `created_at` | TEXT | ISO 8601 timestamp |
| `updated_at` | TEXT | ISO 8601 timestamp |

## Data Models

### AgentStatus

```rust
enum AgentStatus {
    Pending,  // Record created, not yet running
    Running,  // Active in a tmux session
    Stopped,  // Explicitly terminated
    Failed,   // Process crashed or session lost
}
```

### AgentConfig

```rust
struct AgentConfig {
    working_dir: String,
    user: Option<String>,
    shell: String,           // default: "zsh"
    interactive: bool,       // default: false
    prompt: Option<String>,
    worktree: bool,          // default: false
    system_prompt: Option<String>,
}
```

## Usage Examples

### Spawn an SDK-controlled agent

```bash
curl -X POST http://127.0.0.1:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "task-runner",
    "working_dir": "/home/user/project",
    "prompt": "Fix all failing tests"
  }'
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

### Spawn an agent in a worktree with a system prompt

```bash
curl -X POST http://127.0.0.1:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "reviewer",
    "working_dir": "/home/user/project",
    "worktree": true,
    "system_prompt": "You are a code reviewer. Focus on security and performance."
  }'
```

### List running agents

```bash
curl http://127.0.0.1:17006/agents?status=running
```

### Terminate an agent

```bash
curl -X DELETE http://127.0.0.1:17006/agents/{id}
```

### Check service health

```bash
curl http://127.0.0.1:17006/health
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
