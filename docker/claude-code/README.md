# Claude Code Agent Image

Docker image for running Claude Code CLI agents inside containers managed by
the agentd orchestrator's `DockerBackend`.

## Quick Start

```bash
# Build locally
make docker-build-claude

# Or build directly
docker build -t agentd-claude:latest docker/claude-code/

# Verify
docker run --rm agentd-claude:latest --version

# Run with API key
docker run --rm \
  -e ANTHROPIC_API_KEY="$ANTHROPIC_API_KEY" \
  -v "$(pwd):/workspace" \
  agentd-claude:latest --version
```

## What's Included

| Component | Purpose |
|-----------|---------|
| Node.js 22 (slim) | Runtime for Claude Code CLI |
| `@anthropic-ai/claude-code` | The Claude Code CLI itself |
| `git` | Repository operations |
| `curl` | HTTP requests, health checks |
| `jq` | JSON processing |
| `openssh-client` | Git over SSH |
| `ca-certificates` | TLS certificate validation |

## Security

- **Non-root**: Runs as `agent` (UID 1000) by default
- **No secrets**: API keys are passed at runtime via environment variables
- **Minimal base**: Uses `node:22-slim` to reduce attack surface

## Customization

### Adding tools

Create a derived Dockerfile:

```dockerfile
FROM ghcr.io/geoffjay/agentd-claude:latest

USER root
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 \
    ripgrep \
    && rm -rf /var/lib/apt/lists/*
USER agent
```

### Using a different base image

Fork this Dockerfile and change the `FROM` line. The key requirements are:
- Node.js (for the Claude Code CLI)
- A non-root user with UID 1000
- `/workspace` as the working directory

## Image Tags

| Tag | Description |
|-----|-------------|
| `latest` | Latest build from `main` branch |
| `sha-<hash>` | Pinned to a specific git commit |
| `v1.2.3` | Pinned to a semver release tag |

## How It Works with agentd

The `DockerBackend` in the orchestrator creates containers from this image:

1. The orchestrator calls `docker create` with this image
2. The host project directory is bind-mounted to `/workspace`
3. API keys are passed as environment variables
4. The `NetworkPolicy` controls container networking
5. Claude Code connects back to the orchestrator via WebSocket

The container's entrypoint is `claude`, and the orchestrator passes additional
arguments (like `--sdk-url`, `--output-format stream-json`) via `docker exec`.
