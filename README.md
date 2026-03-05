[![CI][ci-badge]][ci-url]
[![codecov][codecov-badge]][codecov-url]
[![MIT licensed][mit-badge]][mit-url]
[![Apache licensed][apache-badge]][apache-url]

[ci-badge]: https://github.com/geoffjay/agentd/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/geoffjay/agentd/actions/workflows/ci.yml
[codecov-badge]: https://codecov.io/gh/geoffjay/agentd/graph/badge.svg?token=knPW8TUmoJ
[codecov-url]: https://codecov.io/gh/geoffjay/agentd
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/geoffjay/agentd/blob/main/LICENSE-MIT
[apache-badge]: https://img.shields.io/badge/License-Apache_2.0-yellowgreen.svg
[apache-url]: https://github.com/geoffjay/agentd/blob/main/LICENSE-APACHE

# agentd

A modular daemon system for managing AI agents, notifications, interactive questions, and system monitoring on macOS.

## Overview

**agentd** is a suite of services and tools designed to orchestrate AI agents and provide intelligent, context-aware notifications and interactions. It consists of:

- **agent** — Command-line interface for interacting with all services
- **agentd-orchestrator** — Agent lifecycle management, WebSocket SDK server, workflow scheduler, and tool policy enforcement
- **agentd-notify** — Notification service with REST API and SQLite storage
- **agentd-ask** — Interactive question service with tmux integration
- **agentd-wrap** — Tmux session management for launching and managing agents
- **agentd-common** — Shared types, error handling, and utilities
- **agentd-hook** — Shell hook integration service (planned)
- **agentd-monitor** — System monitoring service (planned)

## Quick Start

```bash
# Clone and install
git clone https://github.com/geoffjay/agentd.git
cd agentd
cargo xtask install-user
cargo xtask start-services

# Launch agents from declarative YAML templates
agent apply .agentd/

# Or create an agent manually
agent orchestrator create-agent --name my-agent

# Monitor agent output in real-time
agent orchestrator stream --all

# Check all service health
agent status
```

For a complete walkthrough from first run to managing autonomous agents, see the **[Getting Started Guide](docs/public/getting-started.md)**.

## Features

### Declarative YAML Templates

Define agents and workflows as version-controlled YAML files in `.agentd/`:

```
.agentd/
  agents/
    worker.yml          # agent configuration
  workflows/
    issue-worker.yml    # workflow referencing agent by name
```

```bash
agent apply .agentd/                  # create agents + workflows
agent apply --dry-run .agentd/        # validate without creating
agent teardown .agentd/               # delete in reverse order
```

### Orchestrator Service (agentd-orchestrator)

- **Agent lifecycle management** — Create, monitor, attach, and terminate AI agents in tmux sessions
- **WebSocket SDK server** — Implements the Claude Code SDK protocol for programmatic agent control
- **Autonomous workflows** — Schedule workflows that poll GitHub Issues and dispatch tasks to agents
- **Tool policies** — Control which tools agents can use: `AllowAll`, `DenyAll`, `AllowList`, `DenyList`, `RequireApproval`
- **Human-in-the-loop approvals** — Hold tool requests for human review with configurable timeout
- **Real-time streaming** — Watch agent output via `agent orchestrator stream`
- **Interactive attach** — Connect to agent tmux sessions via `agent orchestrator attach`
- **Prompt template validation** — Validate `{{variable}}` placeholders before creating workflows
- **SQLite persistence** — Agent and workflow state survives restarts with automatic reconciliation
- **Prometheus metrics** — `/metrics` endpoint for observability

### CLI (agent)

- **Rich terminal output** with colors and formatted tables
- **Declarative templates** — `agent apply` / `agent teardown` for YAML-based agent and workflow management
- **Agent management** — create, list, get, delete, attach, send-message, stream
- **Workflow management** — create, list, get, update, delete, history, validate-template
- **Tool policies** — get-policy, set-policy, `--tool-policy` flag on create-agent
- **Approval management** — list-approvals, approve, deny (for RequireApproval policy)
- **Health monitoring** — `agent status` checks all services concurrently; per-service `health` commands
- **Shell completions** — `agent completions bash/zsh/fish/powershell`
- **`--json` flag** on all commands for scripting

### Notification System (agentd-notify)

- **REST API** for creating and managing notifications
- **Multiple priority levels** (Low, Normal, High, Urgent) with correct sort ordering
- **Ephemeral and persistent** notifications
- **Response handling** for interactive notifications
- **SQLite storage** for persistence
- **Prometheus metrics** — notifications_created_total by priority

### Wrap Service (agentd-wrap)

- **Tmux session management** — Launch and manage agent CLI sessions
- **Multi-agent support** — Claude Code, OpenCode, Gemini, and other agent types
- **Configurable layouts** — Custom tmux pane layouts via JSON
- **REST API** for launching, listing, and killing sessions

### Ask Service (agentd-ask)

- **tmux integration** — Detects when no tmux sessions are running
- **Smart notifications** — Creates notifications based on system state
- **Cooldown logic** — Prevents notification spam
- **REST API** for triggering checks and answering questions

## Installation

### Prerequisites

- macOS 14+ (tested) or Linux
- Rust 1.75+ ([Install Rust](https://rustup.rs/))
- Git
- tmux (for agent session management)

### Install

```bash
# Using cargo xtask (creates Agent.app bundle + shell completions)
cargo xtask install-user
cargo xtask start-services

# Or use the interactive script
./contrib/scripts/install.sh
```

For detailed installation instructions, see [INSTALL.md](INSTALL.md). Once installed, follow the **[Getting Started Guide](docs/public/getting-started.md)** to learn the full workflow.

## Usage

### YAML Templates (Recommended)

```bash
# Apply a project directory (agents first, then workflows)
agent apply .agentd/

# Apply a single workflow template
agent apply .agentd/workflows/issue-worker.yml

# Validate without creating
agent apply --dry-run .agentd/

# Tear down all resources
agent teardown .agentd/
```

### CLI Commands

```bash
# Service health
agent status                                    # all services
agent orchestrator health                       # single service

# Agent management
agent orchestrator create-agent --name my-agent
agent orchestrator list-agents --status running
agent orchestrator attach --name my-agent       # tmux session
agent orchestrator stream --all                 # live output
agent orchestrator send-message <ID> "Do this"

# Tool policies
agent orchestrator set-policy <ID> '{"mode":"allow_list","tools":["Read","Grep"]}'
agent orchestrator get-policy <ID>

# Approval management (for RequireApproval policy)
agent orchestrator list-approvals
agent orchestrator approve <APPROVAL_ID>
agent orchestrator deny <APPROVAL_ID>

# Workflows
agent orchestrator create-workflow \
  --name issue-worker \
  --agent-name my-agent \
  --owner myorg --repo myrepo \
  --labels "agent" \
  --prompt-template "Fix: {{title}}\n{{body}}"
agent orchestrator validate-template "{{title}} {{body}}"
agent orchestrator workflow-history <ID>

# Notifications
agent notify create --title "Task" --message "Hello" --priority high
agent notify list --actionable
agent notify respond <UUID> "Done"

# Shell completions
agent completions bash > ~/.local/share/bash-completion/completions/agent
agent completions zsh > ~/.zfunc/_agent
```

### REST API

Full API reference: [Orchestrator](docs/public/services/orchestrator.md) | [Notify](docs/public/services/notify.md) | [Ask](docs/public/services/ask.md) | [Wrap](docs/public/services/wrap.md)

```bash
# Health check (all services expose GET /health)
curl http://localhost:17006/health

# Prometheus metrics (all services expose GET /metrics)
curl http://localhost:17006/metrics

# Create an agent
curl -X POST http://localhost:17006/agents \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent", "working_dir": "/path/to/project"}'

# Monitor agent output (WebSocket)
agent orchestrator stream --all    # CLI (preferred)
websocat ws://localhost:17006/stream  # or raw WebSocket
```

## Architecture

### Service Communication

```
                     ┌─────────────────────────────────────────────────┐
                     │                 agent (CLI)                      │
                     │  apply / status / stream / attach / approve      │
                     └──┬──────────┬──────────┬──────────┬─────────────┘
                        │          │          │          │
                        ▼          ▼          ▼          ▼
                ┌──────────┐ ┌─────────┐ ┌────────┐ ┌──────────────┐
                │  notify  │ │   ask   │ │  wrap  │ │ orchestrator │
                │  :17004  │ │  :17001 │ │ :17005 │ │    :17006    │
                └──────────┘ └────┬────┘ └────────┘ └──┬───────────┘
                      ▲           │                    │
                      │           │                    │  WebSocket
                      └───────────┘                    │  (SDK protocol)
                   ask creates notifications           │
                   in the notify service               ▼
                                              ┌────────────────┐
                                              │  tmux sessions  │
                                              │  (claude-code)  │
                                              └────────────────┘
```

All services communicate via REST APIs. The orchestrator additionally provides WebSocket endpoints for the Claude Code SDK protocol, real-time monitoring streams, and tool approval workflows.

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `cli` | Command-line interface with apply/teardown, all service commands |
| `orchestrator` | Agent lifecycle, WebSocket SDK, scheduler, tool policies, approvals |
| `notify` | Notification CRUD with SQLite, priority ordering, expiration |
| `ask` | System checks, tmux detection, question/answer flow |
| `wrap` | Tmux session management, multi-agent launch |
| `common` | Shared types (PaginatedResponse, HealthResponse, ApiError), utilities |
| `hook` | Shell hook integration (planned) |
| `monitor` | System monitoring (planned) |

## Development

### Building

```bash
cargo build --release           # all crates
cargo build -p cli --release    # specific crate
```

### Testing

```bash
cargo test                      # all tests
cargo test -p cli               # specific crate
cargo test -- --nocapture       # with output
```

### Running Services Locally

```bash
# Start services (separate terminals or via xtask)
cargo run -p agentd-orchestrator
cargo run -p agentd-notify
cargo run -p agentd-ask

# Use the CLI
cargo run -p cli -- orchestrator list-agents
cargo run -p cli -- apply .agentd/
```

## Docker

All services ship as Docker images built from a single multi-stage `Dockerfile`.
Each service has its own build target and is packaged separately in `docker-compose.yml`.

### Quick Start

```bash
# Build and start all services (production config)
docker compose up --build

# Build and start with dev overrides (debug logging, text format)
docker compose -f docker-compose.yml -f docker-compose.dev.yml up --build

# Build a single image manually
docker build --target notify -t agentd-notify .
```

### Services

| Service | Image Target | Default Port | Volume |
|---------|-------------|--------------|--------|
| agentd-notify | `notify` | 17004 | `notify-data:/data` |
| agentd-orchestrator | `orchestrator` | 17006 | `orchestrator-data:/data` |
| agentd-ask | `ask` | 17001 | — |
| agentd-wrap | `wrap` | 17005 | — |

### Environment Variables (Docker)

Each service reads the following variables (all have sensible defaults):

| Variable | Services | Default | Description |
|----------|----------|---------|-------------|
| `HOST` | all | `0.0.0.0` | Bind address inside the container |
| `PORT` | all | per-service | Port the service listens on |
| `RUST_LOG` | all | `info` | Log level (`debug`, `info`, `warn`, `error`) |
| `LOG_FORMAT` | all | `json` | Log format (`json` or `text`) |
| `XDG_DATA_HOME` | notify, orchestrator | `/data` | SQLite database location |
| `NOTIFY_SERVICE_URL` | ask | `http://notify:17004` | Notify service URL |
| `WS_BASE_URL` | orchestrator | `ws://orchestrator:17006` | WebSocket base URL for agent callbacks |

Override any variable in `docker-compose.yml` or via `-e` flags:

```bash
docker compose up -e RUST_LOG=debug -e PORT=18001
```

### Data Persistence

`agentd-notify` and `agentd-orchestrator` store their SQLite databases in `/data`
inside the container.  Named volumes are used so data survives container restarts:

```yaml
volumes:
  notify-data:       # mounted at /data in the notify container
  orchestrator-data: # mounted at /data in the orchestrator container
```

To reset a service's state, remove its volume:

```bash
docker compose down -v          # remove all volumes
docker volume rm agentd_notify-data   # remove a single volume
```

### Health Checks

Every service exposes a `GET /health` endpoint.  Docker polls it every 30 s with a
5 s timeout (3 retries, 15 s start-up grace period).  The `ask` service waits for
`notify` to be healthy before it starts.

```bash
# Check service health
docker compose ps
docker inspect --format '{{.State.Health.Status}}' agentd-notify-1
```

## Configuration

For the complete configuration reference including all environment variables, data storage paths, and plist/systemd customization, see the **[Configuration Guide](docs/public/configuration.md)**.

### Port Configuration

| Service | Dev Port | Prod Port | Description |
|---------|----------|-----------|-------------|
| agentd-ask | 17001 | 7001 | Interactive question service |
| agentd-hook | 17002 | 7002 | Shell hook integration |
| agentd-monitor | 17003 | 7003 | System monitoring |
| agentd-notify | 17004 | 7004 | Notification service |
| agentd-wrap | 17005 | 7005 | Tmux session management |
| agentd-orchestrator | 17006 | 7006 | Agent orchestration |

### Environment Variables

- `RUST_LOG` — Log level filter (default: `info`)
- `LOG_FORMAT` — Set to `json` for structured JSON output
- `PORT` — Override the default port for any service
- `HOST` — Bind address for any service (default: `127.0.0.1` locally, `0.0.0.0` in Docker)
- `ORCHESTRATOR_SERVICE_URL` — Override orchestrator URL for CLI (default: `http://localhost:7006`)

## Project Status

**Core Platform:**
- ✅ Orchestrator service (agent lifecycle, WebSocket SDK, scheduler)
- ✅ Notification service (REST API, SQLite, priority ordering)
- ✅ Ask service (tmux integration, REST API)
- ✅ Wrap service (tmux session management, multi-agent)
- ✅ CLI with commands for all services
- ✅ Shared common crate (types, errors, server utilities, storage)

**Agent Management:**
- ✅ Tool policies (AllowAll, DenyAll, AllowList, DenyList, RequireApproval)
- ✅ Human-in-the-loop tool approval with 5-minute timeout
- ✅ Real-time agent output streaming (CLI + WebSocket)
- ✅ Interactive tmux attach by name or ID
- ✅ Send messages to running agents
- ✅ Workflow prompt template validation

**Declarative Templates:**
- ✅ YAML agent templates (`.agentd/agents/*.yml`)
- ✅ YAML workflow templates with agent name references
- ✅ Composite `agent apply` / `agent teardown`
- ✅ Example workflow templates in `examples/workflows/`

**Observability:**
- ✅ Prometheus `/metrics` endpoints on all services
- ✅ Standardized HealthResponse across services
- ✅ Shell completions (bash, zsh, fish, PowerShell)
- ✅ Structured JSON logging (`LOG_FORMAT=json`)
- ✅ GitHub Actions CI/CD pipeline
- ✅ Docker multi-stage images with docker-compose

**In Progress:**
- 🔄 Hook service
- 🔄 Monitor service

## License

MIT OR Apache-2.0
