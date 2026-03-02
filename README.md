# agentd

[![CI](https://github.com/geoffjay/agentd/actions/workflows/ci.yml/badge.svg)](https://github.com/geoffjay/agentd/actions/workflows/ci.yml)

A modular daemon system for managing AI agents, notifications, interactive questions, and system monitoring on macOS.

## Overview

**agentd** is a suite of services and tools designed to orchestrate AI agents and provide intelligent, context-aware notifications and interactions. It consists of:

- **agent** - Command-line interface for interacting with all services
- **agentd-orchestrator** - Agent lifecycle management, WebSocket SDK server, and workflow scheduler
- **agentd-notify** - Notification service with REST API and SQLite storage
- **agentd-ask** - Interactive question service with tmux integration
- **agentd-wrap** - Tmux session management for launching and managing agents
- **agentd-hook** - Shell hook integration service
- **agentd-monitor** - System monitoring service
- **agentd-orchestrator** - Agent lifecycle and workflow orchestration service

## Quick Start

```bash
# Clone the repository
git clone https://github.com/geoffjay/agentd.git
cd agentd

# Install (creates Agent.app bundle)
cargo xtask install-user
cargo xtask start-services

# Use the CLI
agent notify create --title "Hello" --message "agentd is working!"
agent notify list
```

## Features

### Orchestrator Service (agentd-orchestrator)

The orchestrator is the central service for managing AI agents and autonomous workflows.

- **Agent lifecycle management** - Create, monitor, and terminate AI agents running in tmux sessions
- **WebSocket SDK server** - Implements the Claude Code SDK protocol for programmatic agent control
- **Autonomous workflows** - Schedule workflows that poll GitHub Issues and dispatch tasks to agents
- **SQLite persistence** - Agent and workflow state survives restarts with automatic reconciliation
- **Monitoring streams** - Real-time WebSocket streams for observing agent output
- **Interactive and SDK modes** - Agents can run headless (SDK) or in interactive tmux sessions

### Wrap Service (agentd-wrap)

- **Tmux session management** - Launch and manage agent CLI sessions in tmux
- **Multi-agent support** - Claude Code, OpenCode, Gemini, and other agent types
- **Configurable layouts** - Custom tmux pane layouts via JSON configuration
- **REST API** for launching agents with project path, model, and provider settings

### Notification System (agentd-notify)

- **REST API** for creating and managing notifications
- **Multiple priority levels** (Low, Normal, High, Urgent)
- **Ephemeral and persistent** notifications
- **Response handling** for interactive notifications
- **SQLite storage** for persistence
- **Filtering and querying** (by status, priority, actionable)

### Ask Service (agentd-ask)

- **tmux integration** - Detects when no tmux sessions are running
- **Smart notifications** - Asks user questions based on system state
- **Cooldown logic** - Prevents notification spam
- **REST API** for triggering checks and answering questions

### CLI (agent)

- **Rich terminal output** with colors and formatted tables
- **Comprehensive commands** for all services (notify, ask, wrap, orchestrator)
- **Agent management** - Create, list, and terminate orchestrated agents
- **Workflow management** - Create and monitor autonomous GitHub issue workflows

## Installation

### Prerequisites

- macOS 14+ (tested)
- Rust 1.75+ ([Install Rust](https://rustup.rs/))
- Git
- tmux (for agent session management)

### Install

```bash
# Using cargo xtask (creates Agent.app bundle)
cargo xtask install-user
cargo xtask start-services

# Or use the interactive script
./contrib/scripts/install.sh
```

**Note:** Installation creates `/Applications/Agent.app` with all binaries and a symlink at `/usr/local/bin/agent`.

If you encounter permission errors:
```bash
sudo chown -R $(whoami) /usr/local
```

For detailed installation instructions, see [INSTALL.md](INSTALL.md).

## Usage

### CLI Commands

```bash
# Notifications
agent notify create --title "Task" --message "Remember this" --priority high
agent notify list --actionable
agent notify get <UUID>
agent notify respond <UUID> "My answer"
agent notify delete <UUID>

# Ask Service
agent ask trigger              # Trigger system checks
agent ask answer <UUID> "yes"  # Answer a question

# Wrap Service
agent wrap launch my-project \
  --path /path/to/project \
  --agent claude-code \
  --provider anthropic \
  --model claude-sonnet-4.5

# Orchestrator - Agent Management
agent orchestrator list-agents
agent orchestrator list-agents --status running
agent orchestrator create-agent \
  --name my-agent \
  --working-dir /path/to/project \
  --prompt "Analyze the codebase and suggest improvements"
agent orchestrator get-agent <UUID>
agent orchestrator delete-agent <UUID>

# Orchestrator - Workflow Management
agent orchestrator list-workflows
agent orchestrator create-workflow \
  --name issue-worker \
  --agent-id <AGENT_UUID> \
  --owner myorg --repo myrepo \
  --labels "agent" \
  --prompt-template "Work on issue #{{source_id}}: {{title}}\n\n{{body}}"
agent orchestrator workflow-history <WORKFLOW_UUID>
agent orchestrator delete-workflow <WORKFLOW_UUID>
```

### REST API

**Orchestrator Service (port 17006):**

```bash
# Health check
curl http://localhost:17006/health

# Create an agent
curl -X POST http://localhost:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-agent",
    "working_dir": "/path/to/project",
    "prompt": "Analyze the codebase"
  }'

# List running agents
curl "http://localhost:17006/agents?status=running"

# Send a message to a running agent
curl -X POST http://localhost:17006/agents/<UUID>/message \
  -H "Content-Type: application/json" \
  -d '{"content": "Now create issues for the gaps you found"}'

# Monitor agent output (WebSocket)
websocat ws://localhost:17006/stream/<AGENT_UUID>

# List workflows
curl http://localhost:17006/workflows
```

**Notify Service (port 17004):**

```bash
# Health check
curl http://localhost:17004/health

# List notifications
curl http://localhost:17004/notifications

# Create notification
curl -X POST http://localhost:17004/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "source": {"type": "system"},
    "lifetime": {"type": "persistent"},
    "priority": "normal",
    "title": "Test",
    "message": "Hello",
    "requires_response": false
  }'
```

**Ask Service (port 17001):**

```bash
# Health check
curl http://localhost:17001/health

# Trigger checks
curl -X POST http://localhost:17001/trigger
```

**Wrap Service (port 17005):**

```bash
# Launch an agent session
curl -X POST http://localhost:17005/launch \
  -H "Content-Type: application/json" \
  -d '{
    "project_name": "my-project",
    "project_path": "/path/to/project",
    "agent_type": "claude-code",
    "model_provider": "anthropic",
    "model_name": "claude-sonnet-4.5"
  }'
```

## Architecture

### Service Communication

```
                     ┌─────────────────────────────────────────────────┐
                     │                 agent (CLI)                      │
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

All services communicate via REST APIs. The orchestrator additionally provides WebSocket endpoints for the Claude Code SDK protocol and real-time monitoring streams.

### Installed Structure (macOS)
```
/Applications/Agent.app/
├── Contents/
│   ├── Info.plist
│   ├── MacOS/
│   │   ├── cli                    # CLI (symlinked from /usr/local/bin/agent)
│   │   ├── agentd-orchestrator    # Orchestrator service
│   │   ├── agentd-notify          # Notification service
│   │   ├── agentd-ask             # Ask service
│   │   ├── agentd-wrap            # Wrap service
│   │   ├── agentd-hook            # Hook service
│   │   └── agentd-monitor         # Monitor service
│   └── Resources/
│
/usr/local/bin/agent -> /Applications/Agent.app/Contents/MacOS/cli
~/Library/LaunchAgents/com.geoffjay.agentd-*.plist
```

## Development

### Building

```bash
# Build all crates
cargo build --release

# Build specific crate
cargo build -p cli --release
cargo build -p orchestrator --release
cargo build -p notify --release
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p cli
cargo test -p orchestrator
cargo test -p notify
cargo test -p ask

# Run with output
cargo test -- --nocapture
```

**Test Coverage:**
- CLI: 59 tests (unit + integration + doc tests)
- Notify: 64 tests (unit + integration + doc tests)
- Ask: 68 tests (unit + integration)
- Orchestrator: 28 tests (unit + integration)
- Wrap: 27 tests (unit + integration)
- **Total: 240+ tests**

### Running Services Locally

```bash
# Terminal 1: Orchestrator (port 17006)
cargo run -p agentd-orchestrator

# Terminal 2: Notify service (port 17004)
cargo run -p agentd-notify

# Terminal 3: Ask service (port 17001)
cargo run -p agentd-ask

# Terminal 4: CLI
cargo run -p cli -- orchestrator list-agents
cargo run -p cli -- notify list
```

### Code Quality

```bash
# Run clippy
cargo clippy --all-targets --all-features

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check
```

## Service Management

### Using cargo xtask

```bash
cargo xtask service-status  # Check if services are running
cargo xtask start-services  # Start all services
cargo xtask stop-services   # Stop all services
```

### Using launchctl

```bash
# Start a service
launchctl load ~/Library/LaunchAgents/com.geoffjay.agentd-notify.plist

# Stop a service
launchctl unload ~/Library/LaunchAgents/com.geoffjay.agentd-notify.plist

# List services
launchctl list | grep agentd
```

## Configuration

### Port Configuration

Each service uses a **development port** (17xxx) by default when started with `cargo run`, and a **production port** (7xxx) when running as a LaunchAgent. All ports are configurable via the `PORT` environment variable.

| Service | Dev Port | Prod Port | Description |
|---------|----------|-----------|-------------|
| agentd-ask | 17001 | 7001 | Interactive question service |
| agentd-hook | 17002 | 7002 | Shell hook integration |
| agentd-monitor | 17003 | 7003 | System monitoring |
| agentd-notify | 17004 | 7004 | Notification service |
| agentd-wrap | 17005 | 7005 | Tmux session management |
| agentd-orchestrator | 17006 | 7006 | Agent orchestration |

Production ports are set in the LaunchAgent plist files under `contrib/plists/`.

### Log Files

Logs are written to `/usr/local/var/log/`:
- `agentd-orchestrator.log` / `agentd-orchestrator.err`
- `agentd-notify.log` / `agentd-notify.err`
- `agentd-ask.log` / `agentd-ask.err`
- `agentd-wrap.log` / `agentd-wrap.err`
- `agentd-hook.log` / `agentd-hook.err`
- `agentd-monitor.log` / `agentd-monitor.err`
- `agentd-wrap.log` / `agentd-wrap.err`
- `agentd-orchestrator.log` / `agentd-orchestrator.err`

### Database

- **Notify service**: `~/Library/Application Support/agentd-notify/notify.db` (SQLite)
- **Orchestrator**: `~/Library/Application Support/agentd-orchestrator/orchestrator.db` (SQLite)

## Uninstallation

```bash
# Using cargo xtask
cargo xtask uninstall

# Or manually
launchctl unload ~/Library/LaunchAgents/com.geoffjay.agentd-*.plist
rm -f /usr/local/bin/agent
rm -f /usr/local/bin/agentd-*
rm -f ~/Library/LaunchAgents/com.geoffjay.agentd-*.plist
```

## Troubleshooting

### Services won't start

1. Check logs: `tail -f /usr/local/var/log/agentd-*.err`
2. Check status: `cargo xtask service-status`
3. Verify ports: `lsof -i :17004` and `lsof -i :17006`

### Permission errors

```bash
# Fix /usr/local permissions
sudo chown -R $(whoami) /usr/local/bin
sudo mkdir -p /usr/local/var/log
sudo chown -R $(whoami) /usr/local/var
```

### Cannot connect to service

```bash
# Test health endpoints (dev ports)
curl http://localhost:17006/health   # orchestrator
curl http://localhost:17004/health   # notify
curl http://localhost:17001/health   # ask

# Restart services
cargo xtask stop-services
cargo xtask start-services
```

For more troubleshooting, see [INSTALL.md](INSTALL.md).

## Project Status

**Completed:**
- ✅ Orchestrator service (agent lifecycle, WebSocket SDK, scheduler)
- ✅ Notification service (REST API, SQLite storage)
- ✅ Ask service (tmux integration, REST API)
- ✅ Wrap service (tmux session management, multi-agent support)
- ✅ CLI with commands for all services
- ✅ Comprehensive test suite (240+ tests)
- ✅ macOS LaunchAgent integration
- ✅ Installation automation
- ✅ GitHub Actions CI/CD pipeline

**In Progress:**
- 🔄 Hook service
- 🔄 Monitor service

**Planned:**
- 📋 Additional check types for ask service
- 📋 Web UI for notifications
- 📋 Plugin system
- 📋 AI integration via Ollama

## License

MIT OR Apache-2.0
