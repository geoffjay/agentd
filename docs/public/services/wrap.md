# agentd-wrap API Documentation

The wrap service manages tmux sessions for launching AI agent CLIs. It provides a REST API for creating configured tmux sessions with various agent types and model providers.

## Base URL

```
http://127.0.0.1:17005
```

Port defaults to `17005` (dev) or `7005` (production), configurable via the `PORT` environment variable.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `17005` | HTTP listen port |
| `RUST_LOG` | `info` | Log level filter |

## Endpoints

### Health Check

```
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "version": "0.2.0"
}
```

**Example:**
```bash
curl -s http://127.0.0.1:17005/health | jq
```

---

### Launch Agent Session

Create a tmux session and launch an agent inside it.

```
POST /launch
Content-Type: application/json
```

**Request Body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `project_name` | string | yes | | Name for the tmux session (also used as project identifier) |
| `project_path` | string | yes | | Absolute path to the project working directory |
| `agent_type` | string | yes | | Agent CLI to launch (see [Agent Types](#agent-types)) |
| `model_provider` | string | yes | | AI model provider (see [Model Providers](#model-providers)) |
| `model_name` | string | yes | | Model identifier to use |
| `layout` | object | no | `null` | Tmux layout configuration (see [Tmux Layouts](#tmux-layouts)) |

**Example Request:**
```json
{
  "project_name": "my-project",
  "project_path": "/home/user/projects/my-project",
  "agent_type": "claude-code",
  "model_provider": "anthropic",
  "model_name": "claude-sonnet-4.5"
}
```

**Success Response:**
```json
{
  "success": true,
  "session_name": "my-project",
  "message": "Agent launched successfully in session: my-project",
  "error": null
}
```

**Failure Response:**
```json
{
  "success": false,
  "session_name": "my-project",
  "message": "Failed to launch agent: project path does not exist",
  "error": "project path does not exist"
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | boolean | Whether the agent was launched successfully |
| `session_name` | string? | Name of the created tmux session (null on failure) |
| `message` | string | Human-readable status message |
| `error` | string? | Error details (null on success) |

!!! note
    The endpoint always returns HTTP 200, even on failure. Check the `success` field to determine the outcome.

**Errors in the response body:**

| Error | Cause |
|-------|-------|
| `project path does not exist` | The `project_path` directory doesn't exist |
| `Failed to create tmux session` | tmux is not installed or session creation failed |
| `Failed to launch agent` | Agent CLI not found or failed to start |

**Examples:**

```bash
# Launch Claude Code agent
curl -s -X POST http://127.0.0.1:17005/launch \
  -H "Content-Type: application/json" \
  -d '{
    "project_name": "my-project",
    "project_path": "/home/user/projects/my-project",
    "agent_type": "claude-code",
    "model_provider": "anthropic",
    "model_name": "claude-sonnet-4.5"
  }' | jq

# Launch with a vertical split layout
curl -s -X POST http://127.0.0.1:17005/launch \
  -H "Content-Type: application/json" \
  -d '{
    "project_name": "split-session",
    "project_path": "/home/user/projects/my-project",
    "agent_type": "claude-code",
    "model_provider": "anthropic",
    "model_name": "claude-sonnet-4.5",
    "layout": {
      "type": "vertical",
      "panes": 2
    }
  }' | jq

# Launch a general shell session
curl -s -X POST http://127.0.0.1:17005/launch \
  -H "Content-Type: application/json" \
  -d '{
    "project_name": "shell-session",
    "project_path": "/home/user",
    "agent_type": "general",
    "model_provider": "none",
    "model_name": "none"
  }' | jq
```

---

## Agent Types

The `agent_type` field determines which CLI is launched inside the tmux session.

| Agent Type | Command | Description |
|------------|---------|-------------|
| `claude-code` | `claude` | Claude Code CLI (Anthropic) |
| `crush` | `crush` | Crush agent CLI |
| `opencode` | `opencode --model-provider <provider> --model <model>` | OpenCode with configurable provider |
| `gemini` | `gemini --model <model>` | Google Gemini CLI |
| `general` | `$SHELL` (or `/bin/bash`) | Plain shell session, no agent |

## Model Providers

The `model_provider` field is passed to agents that support provider selection (currently `opencode`). Common values:

| Provider | Used With | Example Models |
|----------|-----------|---------------|
| `anthropic` | claude-code, opencode | `claude-sonnet-4.5`, `claude-opus-4` |
| `openai` | opencode | `gpt-4`, `gpt-4o` |
| `ollama` | opencode | `llama3`, `codellama` |
| `none` | general | N/A |

## Tmux Layouts

The optional `layout` field configures the tmux pane layout for the session.

**Layout Object:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | yes | Layout type |
| `panes` | integer | no | Number of panes for split layouts |

**Layout Types:**

| Type | Description |
|------|-------------|
| `single` | Single pane (default when `layout` is omitted) |
| `horizontal` | Split panes side by side (left/right) |
| `vertical` | Split panes stacked (top/bottom) |
| `tiled` | Evenly tiled panes |

**Example:**
```json
{
  "type": "vertical",
  "panes": 3
}
```

This creates a tmux session with 3 vertically stacked panes.

## Working with Sessions

After launching a session via the API, you can interact with it using tmux commands:

```bash
# List all sessions created by the wrap service
tmux list-sessions

# Attach to a session
tmux attach -t my-project

# Detach from session (inside tmux)
# Press Ctrl-b d

# Kill a session
tmux kill-session -t my-project
```

## Running the Service

```bash
# Development
cargo run -p agentd-wrap

# With debug logging
RUST_LOG=debug cargo run -p agentd-wrap

# Custom port
PORT=18005 cargo run -p agentd-wrap
```
