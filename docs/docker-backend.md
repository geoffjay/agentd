# Docker Execution Backend

The Docker execution backend runs each agent in an isolated Docker container
instead of a tmux session. This provides stronger isolation, reproducible
environments, resource limits, and network policy controls.

## Prerequisites

- **Docker Engine 20.10+** or **Docker Desktop** (macOS / Windows / Linux)
- The `agentd-claude:latest` image built locally:

```bash
docker build -t agentd-claude:latest docker/claude-code/
```

- API keys available as environment variables on the host (they are forwarded
  into containers at runtime — nothing is baked into the image):

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

## Quick Start

### 1. Build the agent image

```bash
docker build -t agentd-claude:latest docker/claude-code/
```

### 2. Start the orchestrator with Docker backend

```bash
AGENTD_BACKEND=docker cargo run -p agentd-orchestrator
```

### 3. Create an agent

```bash
agent orchestrator create-agent \
  --name my-agent \
  --working-dir /path/to/project \
  --docker-image agentd-claude:latest
```

### 4. Monitor and manage

```bash
# List agents
agent orchestrator list-agents

# View container logs
agent orchestrator logs --name my-agent --follow

# Terminate
agent orchestrator delete-agent <ID>
```

## Configuration Reference

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENTD_BACKEND` | `tmux` | Execution backend: `tmux` or `docker` |
| `AGENTD_DOCKER_IMAGE` | `agentd-claude:latest` | Default container image |
| `AGENTD_SHUTDOWN_LEAVE_RUNNING` | `false` | If `true`, leave containers running on orchestrator shutdown |
| `ANTHROPIC_API_KEY` | — | Forwarded into containers automatically |
| `OPENAI_API_KEY` | — | Forwarded into containers automatically |
| `GEMINI_API_KEY` | — | Forwarded into containers automatically |
| `ANTHROPIC_BASE_URL` | — | Forwarded into containers automatically |
| `OPENAI_BASE_URL` | — | Forwarded into containers automatically |

### CLI Flags (create-agent)

| Flag | Description |
|------|-------------|
| `--docker-image <IMAGE>` | Override the container image for this agent |
| `--cpu-limit <CPUS>` | CPU limit (e.g., `2.0` for 2 CPUs) |
| `--memory-limit <BYTES>` | Memory limit in bytes (e.g., `2147483648` for 2 GiB) |
| `--mount <HOST:CONTAINER[:ro\|rw]>` | Additional volume mounts (repeatable) |

### Network Policies

Network policies control container network access and how the container
reaches the orchestrator's WebSocket endpoint.

| Policy | Docker Mode | Internet Access | WebSocket Host | Platform |
|--------|------------|-----------------|----------------|----------|
| `internet` (default) | `bridge` | ✅ Full | `host.docker.internal` | All |
| `isolated` | `bridge` (no DNS) | ❌ Restricted | `host.docker.internal` | All |
| `host_network` | `host` | ✅ Full | `127.0.0.1` | Linux only |

**Note:** The `isolated` policy blocks DNS resolution but does not fully
prevent network access via hardcoded IP addresses. For complete network
isolation, use a custom Docker network with no default route.

### Resource Limits

Default resource limits per container:

| Resource | Default | Description |
|----------|---------|-------------|
| Memory | 2 GiB | Container memory limit (`--memory`) |
| CPU | 2 CPUs | Container CPU limit (`--cpus`) |

Override via CLI flags or the `ResourceLimits` struct in code.

## Container Architecture

### Image Layout

The `docker/claude-code/Dockerfile` builds a minimal image based on
`node:22-slim`:

```
/usr/local/bin/claude    ← Claude Code CLI (globally installed)
/workspace               ← Bind-mounted project directory
/home/agent              ← Non-root user home directory
```

Key features:
- **Non-root user** (`agent`, UID 1000) for security
- **Git pre-configured** with safe.directory and default identity
- **HEALTHCHECK** using `claude --version` (30s interval, 10s start period)
- **System tools**: git, curl, jq, openssh-client

### Container Labels

Each container is tagged with labels for filtering and tracking:

| Label | Example | Description |
|-------|---------|-------------|
| `agentd.prefix` | `agentd-orch` | Backend prefix for filtering |
| `agentd.session` | `agentd-orch-abc123` | Full session name |
| `agentd.agent-id` | `abc123` | Agent ID extracted from session name |

### Container Lifecycle

```
create_session()     →  Container created (not started)
launch_agent()       →  Container started (CMD runs)
session_exists()     →  Check if running/created
session_health()     →  HEALTHCHECK status (healthy/unhealthy/starting)
send_command()       →  docker exec into running container
kill_session()       →  Stop (graceful, 10s timeout) + remove
shutdown_all()       →  Stop and remove all labeled containers
```

### Networking Internals

For `internet` and `isolated` policies, the orchestrator adds an extra-hosts
entry so `host.docker.internal` resolves to the host gateway on all platforms:

```
--add-host host.docker.internal:host-gateway
```

The WebSocket URL is constructed as:
- Bridge: `ws://host.docker.internal:{port}/ws/{agent_id}`
- Host network: `ws://127.0.0.1:{port}/ws/{agent_id}`

## Platform Notes

### macOS (Docker Desktop)

- `host.docker.internal` works out of the box
- `host_network` policy is **not supported** (Docker Desktop limitation)
- File sharing must be configured in Docker Desktop preferences for
  bind-mounted working directories

### Linux (Docker Engine)

- `host.docker.internal` requires Docker Engine 20.10+ (the backend adds
  the extra-hosts entry automatically)
- `host_network` policy is fully supported
- No file sharing configuration needed

### Windows (Docker Desktop / WSL2)

- `host.docker.internal` works out of the box
- `host_network` policy is **not supported**
- Working directories must be accessible from the WSL2 distribution

## Reconciliation

The orchestrator periodically reconciles agent state with actual container
status:

1. **Missing containers**: If a container for a running agent is gone, the
   agent is marked `Failed` (non-zero exit) or `Stopped` (exit code 0).
2. **Orphaned containers**: Containers with the backend prefix but no
   matching database record are stopped and removed.
3. **Health monitoring**: Container health status is logged for
   observability during reconciliation.

## Graceful Shutdown

When the orchestrator receives a shutdown signal (SIGTERM/SIGINT):

1. All running agents are marked `Stopped` in the database
2. Unless `AGENTD_SHUTDOWN_LEAVE_RUNNING=true`, the backend stops and
   removes all managed containers
3. Each container gets a 10-second graceful timeout before SIGKILL

## Troubleshooting

### Container won't start

```bash
# Check Docker daemon is running
docker info

# Check the image exists
docker images agentd-claude

# Try running the container manually
docker run --rm agentd-claude:latest --version
```

### Agent can't connect to orchestrator

```bash
# Verify the orchestrator is listening
curl http://localhost:7006/health

# Check host.docker.internal resolves from inside a container
docker run --rm --add-host host.docker.internal:host-gateway \
  alpine ping -c1 host.docker.internal

# On Linux, ensure Docker Engine 20.10+
docker version
```

### Container marked unhealthy

The HEALTHCHECK runs `claude --version` every 30 seconds with a 10-second
start period. If the container is consistently unhealthy:

```bash
# Check container logs
docker logs <container-name>

# Check health check output
docker inspect --format='{{json .State.Health}}' <container-name> | jq
```

### Orphaned containers after crash

If the orchestrator crashes without graceful shutdown, containers may be
left running. Clean them up manually:

```bash
# List agentd containers
docker ps -a --filter "label=agentd.prefix"

# Remove all agentd containers
docker ps -a --filter "label=agentd.prefix" -q | xargs docker rm -f
```

### Permission errors on bind mounts

The container runs as UID 1000. Ensure the host working directory is
readable by UID 1000:

```bash
ls -la /path/to/project
# If needed:
chmod -R o+r /path/to/project
```
