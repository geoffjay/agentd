# Execution Backends

agentd supports three execution backends for running agent sessions. The active
backend is selected via the `AGENTD_BACKEND` environment variable and applies to
both the **orchestrator** (`agentd-orchestrator`) and the standalone **wrap**
service (`agentd-wrap`).

## Selecting a Backend

```bash
# tmux (default - no env var needed)
cargo run -p agentd-orchestrator

# PTY backend (in-process, supports web terminal)
AGENTD_BACKEND=pty cargo run -p agentd-orchestrator

# Docker backend (isolated containers)
AGENTD_BACKEND=docker cargo run -p agentd-orchestrator
```

## Backend Comparison

| Backend  | Dependency     | PTY Streaming | Health Check | Web Terminal | Use Case                             |
|----------|---------------|:-------------:|:------------:|:------------:|--------------------------------------|
| `tmux`   | tmux binary    | ❌            | ❌ (Unknown) | ❌           | Default, simple, battle-tested       |
| `docker` | Docker daemon  | ❌            | ✅           | ❌           | Isolated execution, resource limits  |
| `pty`    | None           | ✅            | ❌           | ✅           | Web terminal, no external deps       |

## Backend Details

### `tmux` (default)

Uses the system `tmux` binary to create and manage agent sessions. Each agent
runs in a dedicated tmux session. This is the simplest option and requires no
additional infrastructure.

**Capabilities:** `attach-tmux`

**Requirements:**
- `tmux` binary on `$PATH`

**Limitations:**
- No health checks - session status is always reported as `Unknown`
- No PTY streaming - the web Terminal tab is not available
- Requires a local tmux installation on the host

**When to use:** Development, local experimentation, or when you already use tmux.

---

### `docker`

Runs each agent in an isolated Docker container. Provides resource limits,
network policy enforcement, and container health checks.

**Capabilities:** `health-check`, `logs`

**Requirements:**
- Docker daemon running and accessible via the default socket
- The agent Docker image (default: `ghcr.io/geoffjay/agentd-agent:latest`)

**Environment variables:**

| Variable              | Default                                    | Description                    |
|-----------------------|--------------------------------------------|-------------------------------|
| `AGENTD_DOCKER_IMAGE` | `ghcr.io/geoffjay/agentd-agent:latest`    | Container image for agents     |

**Limitations:**
- No PTY streaming - the web Terminal tab is not available
- Docker daemon must be running at service startup; startup fails with a clear
  error if Docker is unreachable

**When to use:** Production deployments requiring isolation, resource limits, or
network policy enforcement.

---

### `pty`

Runs each agent as a child process attached to an in-process PTY (pseudo
terminal). Provides full terminal I/O streaming via WebSocket, enabling the
**Web Terminal** tab in the UI and `agent wrap attach` in the CLI.

**Capabilities:** `terminal`, `interactive`

**Requirements:**
- None - no external dependencies

**Limitations:**
- Sessions are in-process; restarting the service terminates all agents
- No container isolation or resource limits
- No structured health checks - process liveness is not reported via the `health-check` capability

**When to use:** Web terminal access, demos, local development without tmux, or
environments where you cannot install tmux or Docker.

## Capability Detection

The orchestrator and wrap service expose a `GET /info` endpoint that returns the
active backend and its capabilities:

```
GET http://localhost:17006/info
```

```json
{
  "backend_type": "pty",
  "version": "0.2.0",
  "capabilities": ["terminal", "interactive"]
}
```

### Capability strings

| Capability    | Backend(s)    | Description                                    |
|---------------|---------------|------------------------------------------------|
| `terminal`    | `pty`         | PTY output streaming via WebSocket             |
| `interactive` | `pty`         | Bidirectional keyboard input via WebSocket     |
| `health-check`| `docker`      | Container health status reported in sessions   |
| `logs`        | `docker`      | Structured container log retrieval             |
| `attach-tmux` | `tmux`        | Native tmux session attach support             |

### Feature gating in the UI

The UI fetches `/info` on the agent detail page and shows the **Terminal** tab
only when the `terminal` capability is present. On tmux and Docker backends the
tab is hidden and agents use the **Logs** tab instead.

### Feature gating in the CLI

`agent wrap info` displays the active backend and its capabilities:

```
$ agent wrap info
Backend:      pty
Version:      0.2.0
Capabilities: terminal, interactive
```

## Invalid Backend Values

Specifying an unrecognised `AGENTD_BACKEND` value causes the service to exit
immediately with a descriptive error:

```
Error: Unknown AGENTD_BACKEND value 'badvalue'. Valid options: tmux, docker, pty
```

This prevents silent degradation - an explicit configuration error is always
surfaced at startup rather than at runtime.
