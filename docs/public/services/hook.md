# agentd-hook

Shell and git hook integration daemon. Receives hook events from shell and git integrations and creates notifications in the notify service when user intervention may be required (e.g., command failures, long-running operations). Supports shell integration script generation for bash, zsh, and fish.

## Base URL

```
http://127.0.0.1:17002
```

Port defaults to `17002` in development and `7002` in production, configurable via the `AGENTD_PORT` environment variable.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENTD_PORT` | `17002` | HTTP listen port |
| `AGENTD_HISTORY_SIZE` | `500` | Number of events to retain in memory |
| `AGENTD_NOTIFY_ON_FAILURE` | `true` | Send notification when a command exits non-zero |
| `AGENTD_NOTIFY_ON_LONG_RUNNING` | `true` | Send notification on long-running commands |
| `AGENTD_LONG_RUNNING_THRESHOLD_MS` | `30000` | Threshold in ms for long-running detection |
| `AGENTD_NOTIFY_SERVICE_URL` | (unset) | URL for the notify service (optional) |
| `AGENTD_LOG_FORMAT` | `text` | Log format: `text` or `json` |

## Endpoints

### `GET /health`

Standard health check. Returns service name, version, event count, and notification settings.

```bash
curl http://localhost:17002/health
```

### `POST /events`

Receive and record a hook event. Evaluates the event against notification eligibility rules and optionally creates a notification.

```bash
curl -X POST http://localhost:17002/events \
  -H "Content-Type: application/json" \
  -d '{
    "kind": "shell",
    "command": "cargo build",
    "exit_code": 1,
    "duration_ms": 45000,
    "output": "error[E0308]: mismatched types",
    "metadata": {
      "cwd": "/home/user/project",
      "shell": "zsh"
    }
  }'
```

**Response:**

```json
{
  "success": true,
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "message": "Event recorded"
}
```

### `GET /events`

List recent events, newest first. Accepts an optional `?limit=N` query parameter (default: 50, max: 500).

```bash
curl http://localhost:17002/events?limit=10
```

### `GET /events/{id}`

Fetch a single event by UUID. Returns `404` if not found.

```bash
curl http://localhost:17002/events/550e8400-e29b-41d4-a716-446655440000
```

### `GET /shell/{shell}`

Generate a shell integration script for the specified shell. Supported values: `zsh`, `bash`, `fish`.

```bash
# Generate and source the zsh integration
curl http://localhost:17002/shell/zsh >> ~/.zshrc
```

The integration script hooks into the shell's precmd/preexec functions to automatically report command completions to the hook service.

## Data Models

### HookKind

| Value | Description |
|-------|-------------|
| `shell` | A shell command completed (success or failure) |
| `git` | A git hook fired (pre-commit, post-commit, etc.) |
| `system` | A generic system event |

### HookEvent (request payload)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `kind` | `HookKind` | yes | Type of hook event |
| `command` | `string` | yes | Command or hook name that triggered the event |
| `exit_code` | `integer` | yes | Process exit code (0 = success) |
| `duration_ms` | `integer` | no | Execution duration in milliseconds (default: 0) |
| `output` | `string` | no | Captured output (stdout/stderr) |
| `metadata` | `object` | no | Arbitrary key-value metadata |

### RecordedEvent (stored/returned)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` (UUID) | Unique event identifier |
| `received_at` | `string` (datetime) | When event was received |
| `kind` | `HookKind` | Type of hook event |
| `command` | `string` | Command or hook name |
| `exit_code` | `integer` | Process exit code |
| `duration_ms` | `integer` | Execution duration |
| `output` | `string` | Captured output |
| `metadata` | `object` | Arbitrary metadata |

## Notification Eligibility

The hook service creates notifications in the notify service when either condition is met:

1. **Command failure** (`AGENTD_NOTIFY_ON_FAILURE=true`): A hook event with a non-zero `exit_code`.
2. **Long-running command** (`AGENTD_NOTIFY_ON_LONG_RUNNING=true`): A hook event with `duration_ms` exceeding `AGENTD_LONG_RUNNING_THRESHOLD_MS`.

Both conditions can be independently enabled or disabled.

## Shell Integration

Source the generated script to automatically report command completions:

```bash
# zsh
eval "$(curl -s http://localhost:17002/shell/zsh)"

# bash
eval "$(curl -s http://localhost:17002/shell/bash)"

# fish
curl -s http://localhost:17002/shell/fish | source
```

The integration captures the command name, exit code, and duration for each command and posts it to the hook service.
