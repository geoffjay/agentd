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

FROM node:22-slim

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
# Pin the version for reproducible builds. Update deliberately or via
# Renovate / Dependabot when a new release is needed.
ARG CLAUDE_CODE_VERSION=0.2.70
RUN npm install -g @anthropic-ai/claude-code@${CLAUDE_CODE_VERSION} && \
    npm cache clean --force

# ── Non-root user ────────────────────────────────────────────────────
# The node:22-slim base image already has a `node` user at UID 1000.
# Rename it to `agent` for clarity and to match DockerBackend's default
# user (1000:1000). Using usermod/groupmod avoids the "UID already
# exists" error from useradd.
RUN groupmod --new-name agent node && \
    usermod --login agent --home /home/agent --move-home --comment "agentd agent" node

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

CMD ["claude"]
