---
name: service-ops
description: Start, stop, and troubleshoot agentd services. Use when checking service health, starting/stopping services, viewing logs, or diagnosing connectivity issues between services.
---

# Service Operations

Operational skill for managing the agentd service fleet.

## Service Inventory

| Service | Dev Port | Prod Port | Binary | Env Var |
|---------|----------|-----------|--------|---------|
| Ask | 17001 | 7001 | `agentd-ask` | `AGENTD_ASK_SERVICE_URL` |
| Notify | 17004 | 7004 | `agentd-notify` | `AGENTD_NOTIFY_SERVICE_URL` |
| Wrap | 17005 | 7005 | `agentd-wrap` | `AGENTD_WRAP_SERVICE_URL` |
| Orchestrator | 17006 | 7006 | `agentd-orchestrator` | `AGENTD_ORCHESTRATOR_SERVICE_URL` |

## Quick Health Check

```bash
# Check all services at once
agent status

# Check individual service health
curl -s http://localhost:7006/health | jq .
curl -s http://localhost:7004/health | jq .
curl -s http://localhost:7005/health | jq .
curl -s http://localhost:7001/health | jq .
```

## Starting Services

```bash
# Start all services (recommended)
cargo xtask start-services

# Start individual services for development
RUST_LOG=debug cargo run -p agentd-orchestrator
RUST_LOG=debug cargo run -p agentd-notify
RUST_LOG=debug cargo run -p agentd-wrap
RUST_LOG=debug cargo run -p agentd-ask

# Start with dev ports (prefix AGENTD_PORT=)
AGENTD_PORT=17006 cargo run -p agentd-orchestrator
```

## Stopping Services

```bash
# Kill specific service
pkill -f agentd-orchestrator
pkill -f agentd-notify
pkill -f agentd-wrap
pkill -f agentd-ask

# Kill all agentd services
pkill -f agentd-
```

## Checking Metrics

All services expose Prometheus metrics:

```bash
curl -s http://localhost:7006/metrics
curl -s http://localhost:7004/metrics
```

## Troubleshooting

### Service won't start
1. Check if port is already in use: `lsof -i :<port>`
2. Check logs: set `RUST_LOG=debug` for verbose output
3. Check database permissions at data directory

### Services can't communicate
1. Verify correct ports with `agent status`
2. Check environment variables are set correctly
3. Ensure all services are on the same port scheme (dev vs prod)

### Database issues
- Orchestrator DB: `~/.local/share/agentd-orchestrator/` or `~/Library/Application Support/agentd-orchestrator/`
- Notify DB: `~/.local/share/agentd-notify/` or `~/Library/Application Support/agentd-notify/`
- Migrations run automatically on startup
- To reset: delete the `.db` file and restart the service

### Viewing service logs
```bash
# Structured JSON logging
AGENTD_LOG_FORMAT=json RUST_LOG=debug cargo run -p agentd-orchestrator

# Filter by module
RUST_LOG=agentd_orchestrator::scheduler=debug cargo run -p agentd-orchestrator
```

## Building

```bash
# Debug build (fast)
cargo build --workspace

# Release build
cargo build --release --workspace

# Install to ~/.local/bin
PREFIX=$HOME/.local cargo xtask install-user
```
