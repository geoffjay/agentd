# agentd

[![CI](https://github.com/geoffjay/agentd/actions/workflows/ci.yml/badge.svg)](https://github.com/geoffjay/agentd/actions/workflows/ci.yml)

A modular daemon system for managing notifications, interactive questions, and system monitoring on macOS.

## Overview

**agentd** is a suite of services and tools designed to provide intelligent, context-aware notifications and interactions. It consists of:

- **agent** - Command-line interface for interacting with all services
- **agentd-notify** - Notification service with REST API
- **agentd-ask** - Interactive question service with tmux integration
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

- **Rich terminal output** with colors and tables
- **Comprehensive commands** for all services
- **Easy-to-use** interface for notification management

## Installation

### Prerequisites

- macOS 14+ (tested)
- Rust 1.75+ ([Install Rust](https://rustup.rs/))
- Git

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
```

### REST API

**Notify Service (default port 17004):**

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

**Ask Service (default port 17001):**

```bash
# Health check
curl http://localhost:17001/health

# Trigger checks
curl -X POST http://localhost:17001/trigger

# Answer question
curl -X POST http://localhost:17001/answer \
  -H "Content-Type: application/json" \
  -d '{
    "question_id": "UUID",
    "answer": "yes"
  }'
```

## Architecture

### Installed Structure (macOS)
```
/Applications/Agent.app/
├── Contents/
│   ├── Info.plist
│   ├── MacOS/
│   │   ├── cli                    # CLI (symlinked from /usr/local/bin/agent)
│   │   ├── agentd-notify          # Notification service
│   │   ├── agentd-ask             # Ask service
│   │   ├── agentd-hook            # Hook service
│   │   ├── agentd-monitor         # Monitor service
│   │   ├── agentd-wrap            # Wrap service
│   │   └── agentd-orchestrator    # Orchestrator service
│   └── Resources/
│
/usr/local/bin/agent -> /Applications/Agent.app/Contents/MacOS/cli
~/Library/LaunchAgents/com.geoffjay.agentd-*.plist
```

### Service Communication
Services communicate via REST APIs:
- agentd-ask: http://localhost:17001
- agentd-hook: http://localhost:17002
- agentd-monitor: http://localhost:17003
- agentd-notify: http://localhost:17004
- agentd-wrap: http://localhost:17005
- agentd-orchestrator: http://localhost:17006

## Development

### Building

```bash
# Build all crates
cargo build --release

# Build specific crate
cargo build -p cli --release
cargo build -p notify --release
cargo build -p ask --release
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p cli
cargo test -p ask

# Run with output
cargo test -- --nocapture
```

**Test Coverage:**
- CLI: 61 tests (30 unit + 31 integration)
- Ask Service: 87 tests (74 unit + 13 integration)
- **Total: 148+ tests**

### Running Services Locally

```bash
# Terminal 1: Notify service (port 17004)
cargo run -p agentd-notify

# Terminal 2: Ask service (port 17001)
cargo run -p agentd-ask

# Terminal 3: CLI
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
- `agentd-notify.log` / `agentd-notify.err`
- `agentd-ask.log` / `agentd-ask.err`
- `agentd-hook.log` / `agentd-hook.err`
- `agentd-monitor.log` / `agentd-monitor.err`
- `agentd-wrap.log` / `agentd-wrap.err`
- `agentd-orchestrator.log` / `agentd-orchestrator.err`

### Database

The notify service stores data in:
- `~/.local/share/agentd/notifications.db` (SQLite)

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
3. Verify ports: `lsof -i :17004` and `lsof -i :17001`

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
curl http://localhost:17004/health
curl http://localhost:17001/health

# Restart services
cargo xtask stop-services
cargo xtask start-services
```

For more troubleshooting, see [INSTALL.md](INSTALL.md).

## Project Status

**Completed:**
- ✅ Notification service (REST API, SQLite storage)
- ✅ Ask service (tmux integration, REST API)
- ✅ CLI with full notification commands
- ✅ Comprehensive test suite (148+ tests)
- ✅ macOS LaunchAgent integration
- ✅ Installation automation

**In Progress:**
- 🔄 Orchestration service
- 🔄 Hook service
- 🔄 Monitor service

**Planned:**
- 📋 Additional check types for ask service
- 📋 Web UI for notifications
- 📋 Plugin system
- 📋 AI integration via Ollama

## License

MIT OR Apache-2.0
