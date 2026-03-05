# Multi-stage Dockerfile for building all agentd services
# Produces a slim runtime image with all service binaries

# =============================================================================
# Stage 1: Builder
# =============================================================================
FROM rust:1.75-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifest and crate metadata
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build all binaries in release mode
# This creates a single build layer that can be cached
RUN cargo build --release --workspace --bins

# =============================================================================
# Stage 2: Runtime
# =============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies (tmux for wrap/orchestrator, ca-certificates for HTTPS)
RUN apt-get update && apt-get install -y --no-install-recommends \
    tmux \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /bin/bash agentd

# Create directories for data storage
RUN mkdir -p /var/lib/agentd/notify \
    && mkdir -p /var/lib/agentd/orchestrator \
    && mkdir -p /var/log/agentd \
    && chown -R agentd:agentd /var/lib/agentd \
    && chown -R agentd:agentd /var/log/agentd

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/agentd-ask /usr/local/bin/
COPY --from=builder /app/target/release/agentd-notify /usr/local/bin/
COPY --from=builder /app/target/release/agentd-orchestrator /usr/local/bin/
COPY --from=builder /app/target/release/agentd-wrap /usr/local/bin/
COPY --from=builder /app/target/release/agentd-hook /usr/local/bin/
COPY --from=builder /app/target/release/agentd-monitor /usr/local/bin/
COPY --from=builder /app/target/release/agent /usr/local/bin/

# Switch to non-root user
USER agentd

# Set environment variables
ENV RUST_LOG=info
ENV LOG_FORMAT=json

# Expose service ports
# Ask: 17001, Hook: 17002, Monitor: 17003, Notify: 17004, Wrap: 17005, Orchestrator: 17006
EXPOSE 17001 17002 17003 17004 17005 17006

# Default command - this is overridden in docker-compose
CMD ["echo", "agentd services built successfully. Use docker-compose to run services."]
