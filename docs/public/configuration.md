# Configuration Reference

This document covers all environment variables, port assignments, data storage locations, and deployment differences for agentd services.

## Environment Variables

### Service Ports

Every service with an HTTP server reads the `PORT` environment variable to determine which port to bind to. If not set, each service uses its built-in development default.

| Variable | Service(s) | Dev Default | Prod Default | Description |
|----------|-----------|-------------|--------------|-------------|
| `PORT` | agentd-ask | `17001` | `7001` | HTTP listen port |
| `PORT` | agentd-hook | `17002` | `7002` | HTTP listen port |
| `PORT` | agentd-monitor | `17003` | `7003` | HTTP listen port |
| `PORT` | agentd-notify | `17004` | `7004` | HTTP listen port |
| `PORT` | agentd-wrap | `17005` | `7005` | HTTP listen port |
| `PORT` | agentd-orchestrator | `17006` | `7006` | HTTP/WebSocket listen port |

### Service URLs

These variables tell services and the CLI how to reach other services.

| Variable | Used by | Default | Description |
|----------|---------|---------|-------------|
| `NOTIFY_SERVICE_URL` | agentd-ask, agent (CLI) | `http://localhost:7004` | Base URL for the notification service |
| `ASK_SERVICE_URL` | agent (CLI) | `http://localhost:7001` | Base URL for the ask service |
| `WRAP_SERVICE_URL` | agent (CLI) | `http://localhost:7005` | Base URL for the wrap service |
| `ORCHESTRATOR_SERVICE_URL` | agent (CLI) | `http://localhost:7006` | Base URL for the orchestrator service |

!!! note "CLI defaults to production ports"
    The `agent` CLI defaults to **production ports** (7xxx) because it's typically used after installation with `cargo xtask install-user`. When developing, source the `.env` file to use dev ports:
    ```bash
    source .env
    agent notify list   # now connects to localhost:17004
    ```

### Logging

| Variable | Used by | Default | Description |
|----------|---------|---------|-------------|
| `RUST_LOG` | all services | `info` | Log level filter (uses `tracing_subscriber::EnvFilter` syntax) |

Common values:

```bash
# Show only warnings and errors
RUST_LOG=warn cargo run -p agentd-notify

# Show debug output for a specific service
RUST_LOG=debug cargo run -p agentd-orchestrator

# Fine-grained control
RUST_LOG=agentd_notify=debug,tower_http=info cargo run -p agentd-notify
```

### Installation

| Variable | Used by | Default | Description |
|----------|---------|---------|-------------|
| `PREFIX` | cargo xtask | `/usr/local` (macOS), `~/.local` (Linux) | Install prefix for binaries and logs |
| `HOME` | cargo xtask, all services | (system) | Home directory (used for plist/unit file paths and database locations) |
| `XDG_CONFIG_HOME` | cargo xtask (Linux) | `~/.config` | Systemd user unit file directory base |
| `XDG_DATA_HOME` | cargo xtask (Linux) | `~/.local/share` | Log directory base on Linux |

## Port Allocation

agentd uses a dual-port scheme to keep development and production environments separate.

### Development Ports (17xxx)

Used when running services directly with `cargo run`. These are the **hardcoded defaults** in each service's `main.rs`:

| Service | Port |
|---------|------|
| agentd-ask | 17001 |
| agentd-hook | 17002 |
| agentd-monitor | 17003 |
| agentd-notify | 17004 |
| agentd-wrap | 17005 |
| agentd-orchestrator | 17006 |

```bash
# These all use dev ports automatically
cargo run -p agentd-notify        # → :17004
cargo run -p agentd-orchestrator  # → :17006
```

### Production Ports (7xxx)

Used when services are installed as LaunchAgents (macOS) or systemd units (Linux). The production port is set via the `PORT` environment variable in the service configuration files:

| Service | Port |
|---------|------|
| agentd-ask | 7001 |
| agentd-hook | 7002 |
| agentd-monitor | 7003 |
| agentd-notify | 7004 |
| agentd-wrap | 7005 |
| agentd-orchestrator | 7006 |

### Overriding Ports

You can override any service's port:

```bash
# Run notify on a custom port
PORT=9004 cargo run -p agentd-notify

# Run orchestrator on port 8080
PORT=8080 cargo run -p agentd-orchestrator
```

## Data Storage

### SQLite Databases

Services that persist data use SQLite databases stored in platform-specific user data directories (via the [`directories`](https://crates.io/crates/directories) crate):

| Service | Database File |
|---------|--------------|
| agentd-notify | `notify.db` |
| agentd-orchestrator | `orchestrator.db` |

**Paths by platform:**

| Platform | Notify | Orchestrator |
|----------|--------|--------------|
| **macOS** | `~/Library/Application Support/agentd-notify/notify.db` | `~/Library/Application Support/agentd-orchestrator/orchestrator.db` |
| **Linux** | `~/.local/share/agentd-notify/notify.db` | `~/.local/share/agentd-orchestrator/orchestrator.db` |

Databases are created automatically on first run. To reset a service's data, stop it and delete the database file.

### Log Files

#### Production (installed services)

When running as LaunchAgents (macOS) or systemd units, logs are written to:

**macOS** (`/usr/local/var/log/` or `$PREFIX/var/log/`):

| File | Contents |
|------|----------|
| `agentd-ask.log` | Standard output |
| `agentd-ask.err` | Standard error (tracing output) |
| `agentd-notify.log` | Standard output |
| `agentd-notify.err` | Standard error |
| `agentd-orchestrator.log` | Standard output |
| `agentd-orchestrator.err` | Standard error |
| `agentd-wrap.log` | Standard output |
| `agentd-wrap.err` | Standard error |
| `agentd-hook.log` | Standard output |
| `agentd-hook.err` | Standard error |
| `agentd-monitor.log` | Standard output |
| `agentd-monitor.err` | Standard error |

**Linux**: When running as systemd user units, logs go to journald by default:

```bash
# View logs for a specific service
journalctl --user -u agentd-notify.service

# Follow logs in real time
journalctl --user -u agentd-orchestrator.service -f

# View logs since boot
journalctl --user -u agentd-ask.service -b
```

#### Development

When running with `cargo run`, all log output goes to the terminal's stderr (controlled by `RUST_LOG`).

## LaunchAgent Plist Configuration (macOS)

LaunchAgent plist files are installed to `~/Library/LaunchAgents/` and define how macOS manages each service. The source files live in `contrib/plists/`.

### Plist Structure

Each plist configures:

- **Label**: `com.geoffjay.agentd-{service}` — unique identifier for launchd
- **ProgramArguments**: Path to the binary in `/Applications/Agent.app/Contents/MacOS/`
- **RunAtLoad**: `true` — service starts automatically at login
- **KeepAlive/SuccessfulExit**: `false` — automatically restarts on crash
- **StandardOutPath/StandardErrorPath**: Log file locations
- **EnvironmentVariables**: `PORT`, `RUST_LOG`, and any service-specific vars
- **WorkingDirectory**: `/usr/local`

### Customizing Plists

To customize a service after installation, edit the plist directly:

```bash
# Edit the notify service configuration
vi ~/Library/LaunchAgents/com.geoffjay.agentd-notify.plist

# Reload after editing
launchctl unload ~/Library/LaunchAgents/com.geoffjay.agentd-notify.plist
launchctl load ~/Library/LaunchAgents/com.geoffjay.agentd-notify.plist
```

Common customizations:

```xml
<!-- Change the port -->
<key>PORT</key>
<string>9004</string>

<!-- Enable debug logging -->
<key>RUST_LOG</key>
<string>debug</string>

<!-- Add custom environment variables -->
<key>MY_CUSTOM_VAR</key>
<string>my-value</string>
```

## Systemd Unit Configuration (Linux)

On Linux, `cargo xtask install-user` generates systemd user unit files in `~/.config/systemd/user/`.

### Unit Structure

Each unit file configures:

- **Type**: `simple` — the process is the main service
- **ExecStart**: Path to the installed binary
- **Restart**: `on-failure` with 5-second delay
- **Environment**: `PORT`, `RUST_LOG`, and service-specific vars
- **WantedBy**: `default.target` — starts when the user session begins

### Managing Services

```bash
# Start a service
systemctl --user start agentd-notify.service

# Stop a service
systemctl --user stop agentd-notify.service

# Enable auto-start at login
systemctl --user enable agentd-notify.service

# Check status
systemctl --user status agentd-orchestrator.service

# Reload after editing unit files
systemctl --user daemon-reload
```

### Customizing Units

Edit the generated unit files:

```bash
vi ~/.config/systemd/user/agentd-notify.service
systemctl --user daemon-reload
systemctl --user restart agentd-notify.service
```

## Development vs Production

| Aspect | Development (`cargo run`) | Production (installed) |
|--------|--------------------------|----------------------|
| **Ports** | 17001–17006 (hardcoded defaults) | 7001–7006 (set via `PORT` env var) |
| **Binary location** | `target/debug/` or `target/release/` | `/Applications/Agent.app/Contents/MacOS/` (macOS) or `~/.local/bin/` (Linux) |
| **Service manager** | Manual (run in terminal) | launchd (macOS) or systemd (Linux) |
| **Logs** | Terminal stderr | File-based (macOS) or journald (Linux) |
| **Auto-restart** | No | Yes (on crash) |
| **Auto-start at login** | No | Yes |
| **CLI default URLs** | Needs `source .env` for dev ports | Works out of the box (7xxx) |
| **Database location** | Same platform-specific path | Same platform-specific path |

### Using the .env File for Development

The project includes a `.env` file that sets `*_SERVICE_URL` variables to dev ports:

```bash
# Source the dev environment
source .env

# Now the CLI connects to dev ports
agent notify list                    # → http://localhost:17004
agent orchestrator list-agents       # → http://localhost:17006
```

Contents of `.env`:

```bash
export ASK_SERVICE_URL=http://localhost:17001
export HOOK_SERVICE_URL=http://localhost:17002
export MONITOR_SERVICE_URL=http://localhost:17003
export NOTIFY_SERVICE_URL=http://localhost:17004
export WRAP_SERVICE_URL=http://localhost:17005
export ORCHESTRATOR_SERVICE_URL=http://localhost:17006
export RUST_LOG=info
```
