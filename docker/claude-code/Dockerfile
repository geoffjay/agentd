# syntax=docker/dockerfile:1
#
# Claude Code agent execution image for agentd.
#
# This image provides a minimal environment for running the Claude Code CLI
# inside Docker containers managed by the agentd orchestrator's DockerBackend.
#
# Build:
#   docker build -t agentd-claude:latest docker/claude-code/
#
# Run (requires ANTHROPIC_API_KEY):
#   docker run --rm -e ANTHROPIC_API_KEY="$ANTHROPIC_API_KEY" agentd-claude:latest --version
#
# Usage with agentd:
#   The orchestrator's DockerBackend creates containers from this image
#   automatically. API keys are passed as environment variables at runtime —
#   nothing secret is baked into the image.

FROM node:22-slim AS base

# ── System dependencies ───────────────────────────────────────────────
# Install common dev tools needed by Claude Code and agent workflows.
# - git: repository operations, diffs, commits
# - ca-certificates: HTTPS connections to APIs and registries
# - curl: health checks, API calls, downloading tools
# - jq: JSON processing in shell scripts
# - openssh-client: git operations over SSH
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        git \
        ca-certificates \
        curl \
        jq \
        openssh-client \
    && rm -rf /var/lib/apt/lists/*

# ── Claude Code CLI ──────────────────────────────────────────────────
# Install globally so it's available on PATH for all users.
RUN npm install -g @anthropic-ai/claude-code && \
    npm cache clean --force

# ── Non-root user ────────────────────────────────────────────────────
# Create a dedicated 'agent' user. The UID/GID 1000 matches the default
# user on most Linux hosts and the user set by DockerBackend (1000:1000).
RUN groupadd --gid 1000 agent && \
    useradd --uid 1000 --gid agent --create-home --shell /bin/bash agent

# ── Git configuration ────────────────────────────────────────────────
# Set safe.directory so git works in bind-mounted /workspace regardless
# of ownership. Also set a default identity for commits made by agents.
RUN git config --system safe.directory /workspace && \
    git config --system user.email "agent@agentd.local" && \
    git config --system user.name "agentd agent"

# ── Workspace ────────────────────────────────────────────────────────
RUN mkdir -p /workspace && chown agent:agent /workspace
WORKDIR /workspace

# ── Runtime ──────────────────────────────────────────────────────────
USER agent

# Verify the CLI is installed and accessible.
RUN claude --version

ENTRYPOINT ["claude"]
