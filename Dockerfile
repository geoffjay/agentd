# Multi-stage Dockerfile for agentd services.
#
# Build targets (used by docker-compose via `target:`):
#   ask          — agentd-ask service
#   notify       — agentd-notify service
#   orchestrator — agentd-orchestrator service
#   wrap         — agentd-wrap service
#
# Build all images at once:
#   docker compose build
#
# Build a single image:
#   docker build --target notify -t agentd-notify .

# ── Builder ────────────────────────────────────────────────────────────────────
FROM rust:1.83-slim-bookworm AS builder

# System dependencies needed at compile time.
# libsqlite3-dev: SQLite (sqlx uses system lib — no `bundled` feature)
# pkg-config + libssl-dev: may be required by transitive build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests first so Docker can cache the dependency layer.
COPY Cargo.toml Cargo.lock ./

# Copy per-crate Cargo.toml files.
COPY crates/ask/Cargo.toml        crates/ask/
COPY crates/baml/Cargo.toml       crates/baml/
COPY crates/cli/Cargo.toml        crates/cli/
COPY crates/common/Cargo.toml     crates/common/
COPY crates/hook/Cargo.toml       crates/hook/
COPY crates/monitor/Cargo.toml    crates/monitor/
COPY crates/notify/Cargo.toml     crates/notify/
COPY crates/orchestrator/Cargo.toml crates/orchestrator/
COPY crates/ollama/Cargo.toml     crates/ollama/
COPY crates/wrap/Cargo.toml       crates/wrap/
COPY crates/xtask/Cargo.toml      crates/xtask/

# Create minimal stub source files so `cargo build` can resolve and cache
# all dependencies without compiling the real application code.
RUN for crate in ask baml cli common hook monitor notify orchestrator ollama wrap xtask; do \
        mkdir -p crates/$crate/src; \
        echo 'fn main() {}' > crates/$crate/src/main.rs; \
        touch crates/$crate/src/lib.rs; \
    done \
    && cargo build --release 2>&1 | tail -5 || true

# Copy the real source tree and rebuild (only changed crates recompile).
COPY . .
# Touch source files to invalidate the stub-built artifacts.
RUN find crates -name '*.rs' -exec touch {} + \
    && cargo build --release

# ── Runtime base ───────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime-base

# Runtime dependencies:
#   ca-certificates: TLS root certificates (reqwest uses rustls)
#   libsqlite3-0:    SQLite shared library (notify + orchestrator)
#   curl:            Used by HEALTHCHECK instructions
#   tmux:            Required by agentd-wrap and agentd-orchestrator (agent sessions)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libsqlite3-0 \
    curl \
    tmux \
    && rm -rf /var/lib/apt/lists/*

# Use a dedicated non-root user for all services.
RUN useradd --system --create-home --home-dir /home/agentd --shell /usr/sbin/nologin agentd

USER agentd
WORKDIR /home/agentd

# Default environment shared across services.
ENV RUST_LOG=info \
    LOG_FORMAT=json

# ── agentd-ask ────────────────────────────────────────────────────────────────
FROM runtime-base AS ask

COPY --from=builder --chown=agentd:agentd \
    /build/target/release/agentd-ask /usr/local/bin/agentd-ask

# ask binds to 0.0.0.0 by default, so HOST is not strictly needed here,
# but we export it for consistency.
ENV PORT=17001 \
    HOST=0.0.0.0 \
    NOTIFY_SERVICE_URL=http://notify:17004

EXPOSE 17001

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://localhost:${PORT}/health > /dev/null

CMD ["agentd-ask"]

# ── agentd-notify ─────────────────────────────────────────────────────────────
FROM runtime-base AS notify

COPY --from=builder --chown=agentd:agentd \
    /build/target/release/agentd-notify /usr/local/bin/agentd-notify

ENV PORT=17004 \
    HOST=0.0.0.0 \
    # Redirect XDG data to /data so it can be backed by a named volume.
    XDG_DATA_HOME=/data

EXPOSE 17004

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://localhost:${PORT}/health > /dev/null

# The /data volume holds notify.db; ensure it is writable by the agentd user.
VOLUME ["/data"]

CMD ["agentd-notify"]

# ── agentd-orchestrator ───────────────────────────────────────────────────────
FROM runtime-base AS orchestrator

COPY --from=builder --chown=agentd:agentd \
    /build/target/release/agentd-orchestrator /usr/local/bin/agentd-orchestrator

ENV PORT=17006 \
    HOST=0.0.0.0 \
    # WS_BASE_URL is used by agents to connect back; override if using a
    # reverse proxy or exposing on a non-default host/port.
    WS_BASE_URL=ws://orchestrator:17006 \
    # Redirect XDG data to /data so it can be backed by a named volume.
    XDG_DATA_HOME=/data

EXPOSE 17006

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://localhost:${PORT}/health > /dev/null

# The /data volume holds orchestrator.db; ensure it is writable by the agentd user.
VOLUME ["/data"]

CMD ["agentd-orchestrator"]

# ── agentd-wrap ───────────────────────────────────────────────────────────────
FROM runtime-base AS wrap

COPY --from=builder --chown=agentd:agentd \
    /build/target/release/agentd-wrap /usr/local/bin/agentd-wrap

ENV PORT=17005 \
    HOST=0.0.0.0

EXPOSE 17005

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://localhost:${PORT}/health > /dev/null

CMD ["agentd-wrap"]
