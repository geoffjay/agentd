# Configuration Reference

This document covers all environment variables, port assignments, data storage locations, and deployment differences for agentd services.

## Environment Variables

### Service Ports

Every service with an HTTP server reads the `AGENTD_PORT` environment variable to determine which port to bind to. If not set, each service uses its built-in development default.

| Variable | Service(s) | Dev Default | Prod Default | Description |
|----------|-----------|-------------|--------------|-------------|
| `AGENTD_PORT` | agentd-ask | `17001` | `7001` | HTTP listen port |
| `AGENTD_PORT` | agentd-hook | `17002` | `7002` | HTTP listen port |
| `AGENTD_PORT` | agentd-monitor | `17003` | `7003` | HTTP listen port |
| `AGENTD_PORT` | agentd-notify | `17004` | `7004` | HTTP listen port |
| `AGENTD_PORT` | agentd-wrap | `17005` | `7005` | HTTP listen port |
| `AGENTD_PORT` | agentd-orchestrator | `17006` | `7006` | HTTP/WebSocket listen port |
| `AGENTD_PORT` | agentd-communicate | `17010` | `7010` | HTTP/WebSocket listen port |

### Service URLs

These variables tell services and the CLI how to reach other services.

| Variable | Used by | Default | Description |
|----------|---------|---------|-------------|
| `AGENTD_NOTIFY_SERVICE_URL` | agentd-ask, agent (CLI) | `http://localhost:7004` | Base URL for the notification service |
| `AGENTD_ASK_SERVICE_URL` | agent (CLI) | `http://localhost:7001` | Base URL for the ask service |
| `AGENTD_WRAP_SERVICE_URL` | agent (CLI) | `http://localhost:7005` | Base URL for the wrap service |
| `AGENTD_ORCHESTRATOR_SERVICE_URL` | agent (CLI) | `http://localhost:7006` | Base URL for the orchestrator service |
| `AGENTD_COMMUNICATE_SERVICE_URL` | agent (CLI) | `http://localhost:7010` | Base URL for the communicate service |

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
| agentd-communicate | 17010 |

```bash
# These all use dev ports automatically
cargo run -p agentd-notify        # → :17004
cargo run -p agentd-orchestrator  # → :17006
cargo run -p agentd-communicate   # → :17010
```

### Production Ports (7xxx)

Used when services are installed as LaunchAgents (macOS) or systemd units (Linux). The production port is set via the `AGENTD_PORT` environment variable in the service configuration files:

| Service | Port |
|---------|------|
| agentd-ask | 7001 |
| agentd-hook | 7002 |
| agentd-monitor | 7003 |
| agentd-notify | 7004 |
| agentd-wrap | 7005 |
| agentd-orchestrator | 7006 |
| agentd-communicate | 7010 |

### Overriding Ports

You can override any service's port:

```bash
# Run notify on a custom port
AGENTD_PORT=9004 cargo run -p agentd-notify

# Run orchestrator on port 8080
AGENTD_PORT=8080 cargo run -p agentd-orchestrator
```

## Data Storage

### SQLite Databases

Services that persist data use SQLite databases stored in platform-specific user data directories (via the [`directories`](https://crates.io/crates/directories) crate):

| Service | Database File |
|---------|--------------|
| agentd-notify | `notify.db` |
| agentd-orchestrator | `orchestrator.db` |
| agentd-communicate | `communicate.db` |

**Paths by platform:**

| Platform | Notify | Orchestrator | Communicate |
|----------|--------|--------------|-------------|
| **macOS** | `~/Library/Application Support/agentd-notify/notify.db` | `~/Library/Application Support/agentd-orchestrator/orchestrator.db` | `~/Library/Application Support/agentd-communicate/communicate.db` |
| **Linux** | `~/.local/share/agentd-notify/notify.db` | `~/.local/share/agentd-orchestrator/orchestrator.db` | `~/.local/share/agentd-communicate/communicate.db` |

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

LaunchAgent plist files are installed to `~/Library/LaunchAgents/` and define how macOS manages each service. They are generated at install time by the `agentd-install` crate (`generate_plist` in `crates/install/src/platform/macos.rs`) — one per service in the canonical `SERVICES` list — so there is no separate source file to edit.

### Plist Structure

Each plist configures:

- **Label**: `com.geoffjay.agentd-{service}` - unique identifier for launchd
- **ProgramArguments**: Path to the binary in `/Applications/Agent.app/Contents/MacOS/`
- **RunAtLoad**: `true` - service starts automatically at login
- **KeepAlive/SuccessfulExit**: `false` - automatically restarts on crash
- **StandardOutPath/StandardErrorPath**: Log file locations
- **EnvironmentVariables**: `AGENTD_PORT`, `RUST_LOG`, and any service-specific vars
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
<key>AGENTD_PORT</key>
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

- **Type**: `simple` - the process is the main service
- **ExecStart**: Path to the installed binary
- **Restart**: `on-failure` with 5-second delay
- **Environment**: `AGENTD_PORT`, `RUST_LOG`, and service-specific vars
- **WantedBy**: `default.target` - starts when the user session begins

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
| **Ports** | 17001–17006 (hardcoded defaults) | 7001–7006 (set via `AGENTD_PORT` env var) |
| **Binary location** | `target/debug/` or `target/release/` | `/Applications/Agent.app/Contents/MacOS/` (macOS) or `~/.local/bin/` (Linux) |
| **Service manager** | Manual (run in terminal) | launchd (macOS) or systemd (Linux) |
| **Logs** | Terminal stderr | File-based (macOS) or journald (Linux) |
| **Auto-restart** | No | Yes (on crash) |
| **Auto-start at login** | No | Yes |
| **CLI default URLs** | Needs `source .env` for dev ports | Works out of the box (7xxx) |
| **Database location** | Same platform-specific path | Same platform-specific path |

### Using the .env File for Development

The project includes a `.env` file that sets `AGENTD_*_SERVICE_URL` variables to dev ports:

```bash
# Source the dev environment
source .env

# Now the CLI connects to dev ports
agent notify list                    # → http://localhost:17004
agent orchestrator list-agents       # → http://localhost:17006
```

Contents of `.env`:

```bash
export AGENTD_ASK_SERVICE_URL=http://localhost:17001
export AGENTD_HOOK_SERVICE_URL=http://localhost:17002
export AGENTD_MONITOR_SERVICE_URL=http://localhost:17003
export AGENTD_NOTIFY_SERVICE_URL=http://localhost:17004
export AGENTD_WRAP_SERVICE_URL=http://localhost:17005
export AGENTD_ORCHESTRATOR_SERVICE_URL=http://localhost:17006
export AGENTD_COMMUNICATE_SERVICE_URL=http://localhost:17010
export RUST_LOG=info
```

---

## Configuration File (TOML)

In addition to environment variables, agentd reads a TOML configuration file. This section documents the file format, precedence, the complete per-service key reference, example configs, and migration from env-var-only setups.

agentd uses a three-layer configuration system. Settings are resolved in order of ascending precedence:

```
compiled defaults  <  config file  <  environment variables
```

Any layer can be omitted. A missing config file is silently ignored - compiled defaults and environment variables still apply.

### Quick Start

Generate a fully-commented default config file:

```bash
agent config init
```

This writes `~/.config/agentd/config.toml` with every section and key documented inline. Edit only the values you want to change; omit a key to keep the compiled default.

Show the fully resolved configuration (defaults + file + environment variables):

```bash
agent config show
```

Show only the raw on-disk config file (no env var overlay):

```bash
agent config show --raw
```

Show the resolved config as JSON (useful for scripting):

```bash
agent config show --json
```

### Config File Location

The config file is searched for in this order:

| Priority | Location |
|----------|----------|
| 1 (highest) | Path in `AGENTD_CONFIG` environment variable (if set and non-empty) |
| 2 | `$XDG_CONFIG_HOME/agentd/config.toml` |
| 3 (lowest) | `~/.config/agentd/config.toml` |

**Examples:**

```bash
# Use a custom path for this invocation
AGENTD_CONFIG=/etc/agentd/config.toml agent config show

# Default path (macOS and Linux)
~/.config/agentd/config.toml
```

### Precedence Rules

The three-layer system means every setting can be controlled at whatever level is appropriate for your environment.

**Example:** Consider `services.notify.port`:

| What you do | Result |
|-------------|--------|
| Do nothing | `17004` (compiled default) |
| Set `port = 19004` in `[services.notify]` in the config file | `19004` |
| Set `AGENTD_NOTIFY_PORT=29004` in the environment | `29004` (even if config file says `19004`) |

**Concrete precedence demonstration:**

```toml
# ~/.config/agentd/config.toml
[services.notify]
port = 19004      # beats the compiled default of 17004
```

```bash
# Environment variable beats both the file and compiled default
AGENTD_NOTIFY_PORT=29004 agent config show
# -> services.notify.port = 29004
```

```bash
# No env var set: file value wins over default
agent config show
# -> services.notify.port = 19004
```

### Complete Key Reference

All configuration keys with their TOML path, environment variable, default value, and description.

#### General Settings

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `general.log_level` | `AGENTD_LOG_LEVEL` | `info` | Log level filter: `trace`, `debug`, `info`, `warn`, `error` |
| `general.log_format` | `AGENTD_LOG_FORMAT` | `text` | Log output format: `text` (human-readable) or `json` (structured) |
| `general.host` | `AGENTD_HOST` | `127.0.0.1` | Default bind address for all services |

#### agentd-core (port 17000)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.core.port` | `AGENTD_CORE_PORT` | `17000` | HTTP listen port |

#### agentd-ask (port 17001)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.ask.port` | `AGENTD_ASK_PORT` | `17001` | HTTP listen port |
| `services.ask.orchestrator_url` | `AGENTD_ASK_ORCHESTRATOR_URL` | `http://localhost:17006` | Orchestrator callback URL |

#### agentd-hook (port 17002)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.hook.port` | `AGENTD_HOOK_PORT` | `17002` | HTTP listen port |
| `services.hook.history_size` | `AGENTD_HISTORY_SIZE` | `500` | Maximum shell-event history retained in memory |
| `services.hook.notify_service_url` | `AGENTD_NOTIFY_SERVICE_URL` | *(unset)* | Optional notify service URL for forwarding events |

#### agentd-monitor (port 17003)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.monitor.port` | `AGENTD_MONITOR_PORT` | `17003` | HTTP listen port |
| `services.monitor.collection_interval_secs` | `AGENTD_COLLECTION_INTERVAL_SECS` | `15` | Metrics collection interval in seconds |

#### agentd-notify (port 17004)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.notify.port` | `AGENTD_NOTIFY_PORT` | `17004` | HTTP listen port |

#### agentd-wrap (port 17005)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.wrap.port` | `AGENTD_WRAP_PORT` | `17005` | HTTP listen port |
| `services.wrap.backend` | `AGENTD_WRAP_BACKEND` | `tmux` | Execution backend: `tmux`, `docker`, `pty`, `subprocess` |

#### agentd-orchestrator (port 17006)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.orchestrator.port` | `AGENTD_ORCHESTRATOR_PORT` | `17006` | HTTP listen port |
| `services.orchestrator.backend` | `AGENTD_ORCHESTRATOR_BACKEND` | `tmux` | Execution backend: `tmux`, `docker`, `pty`, `subprocess` |
| `services.orchestrator.communicate_url` | `AGENTD_ORCHESTRATOR_COMMUNICATE_URL` | `http://localhost:17010` | Communicate service URL for agent message delivery |
| `services.orchestrator.reconcile_interval_secs` | `AGENTD_RECONCILE_INTERVAL_SECS` | `30` | Agent reconciliation interval in seconds |

#### agentd-memory (port 17008)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.memory.port` | `AGENTD_MEMORY_PORT` | `17008` | HTTP listen port |
| `services.memory.embedding_provider` | `AGENTD_MEMORY_EMBEDDING_PROVIDER` | `none` | Embedding provider: `none`, `ollama`, `openai` |
| `services.memory.embedding_model` | `AGENTD_MEMORY_EMBEDDING_MODEL` | `text-embedding-3-small` | Embedding model name |
| `services.memory.lance_path` | `AGENTD_MEMORY_LANCE_PATH` | XDG data dir | LanceDB storage directory |

#### agentd-ui (port 17009)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.ui.port` | `AGENTD_UI_PORT` | `17009` | HTTP listen port |
| `services.ui.ui_dir` | `AGENTD_UI_DIR` | `./ui/dist` | Directory containing compiled frontend assets |

#### agentd-communicate (port 17010)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.communicate.port` | `AGENTD_COMMUNICATE_PORT` | `17010` | HTTP listen port |

#### agentd-mcp (stdio transport)

The MCP server uses stdio transport and has no dedicated port of its own. These settings control how the MCP tools reach the other agentd services.

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.mcp.orchestrator_url` | `AGENTD_MCP_ORCHESTRATOR_URL` | `http://127.0.0.1:17006` | Orchestrator service URL |
| `services.mcp.notify_url` | `AGENTD_MCP_NOTIFY_URL` | `http://127.0.0.1:17004` | Notify service URL |
| `services.mcp.ask_url` | `AGENTD_MCP_ASK_URL` | `http://127.0.0.1:17001` | Ask service URL |
| `services.mcp.memory_url` | `AGENTD_MCP_MEMORY_URL` | `http://127.0.0.1:17008` | Memory service URL |
| `services.mcp.communicate_url` | `AGENTD_MCP_COMMUNICATE_URL` | `http://127.0.0.1:17010` | Communicate service URL |
| `services.mcp.wrap_url` | `AGENTD_MCP_WRAP_URL` | `http://127.0.0.1:17005` | Wrap service URL |
| `services.mcp.monitor_url` | `AGENTD_MCP_MONITOR_URL` | `http://127.0.0.1:17003` | Monitor service URL |
| `services.mcp.hook_url` | `AGENTD_MCP_HOOK_URL` | `http://127.0.0.1:17002` | Hook service URL |

### Per-Service Configuration Sections

#### Execution Backends

Both `agentd-orchestrator` and `agentd-wrap` support four execution backends. Set `backend` to one of:

| Backend | Description | Use when |
|---------|-------------|----------|
| `tmux` | Launches agents in tmux sessions (default) | Local development, interactive attach |
| `subprocess` | Launches agents as direct child processes with stdio | Lightweight, no tmux required |
| `pty` | Launches agents in a pseudo-terminal | Interactive terminal emulation |
| `docker` | Launches agents in Docker containers | Isolation, reproducible environments |

#### Embedding Providers

**agentd-memory** supports:
- `none` - No embeddings; memory search uses exact-match only
- `ollama` - Local Ollama instance (default model: `nomic-embed-text`)
- `openai` - OpenAI API (default model: `text-embedding-3-small`)

#### LanceDB Paths

The `lance_path` for the memory service defaults to the platform-specific XDG data directory:

- **Linux:** `~/.local/share/agentd-memory/lancedb`
- **macOS:** `~/Library/Application Support/agentd-memory/lancedb`

Override with the environment variable or a TOML entry when you want vector data on a specific volume.

#### Hook Notify Service URL

`services.hook.notify_service_url` is optional (no default). When set, the hook service forwards notable shell events to the notify service at that URL. Leave it unset to disable this integration.

```toml
[services.hook]
notify_service_url = "http://localhost:17004"
```

### Example Configurations

#### Minimal Config (override just a few values)

```toml
# ~/.config/agentd/config.toml

[general]
log_level = "debug"

[services.orchestrator]
backend = "subprocess"
```

#### Development Config

All services on localhost with verbose logging:

```toml
[general]
log_level = "debug"
log_format = "text"
host = "127.0.0.1"

[services.orchestrator]
backend = "tmux"
reconcile_interval_secs = 10

[services.memory]
embedding_provider = "ollama"
embedding_model = "nomic-embed-text"

[services.hook]
history_size = 1000
notify_service_url = "http://localhost:17004"

[services.monitor]
collection_interval_secs = 5
```

#### Production Config (custom ports, external URLs)

```toml
[general]
log_level = "warn"
log_format = "json"
host = "0.0.0.0"

[services.core]
port = 8000

[services.ask]
port = 8001
orchestrator_url = "http://orchestrator.internal:8006"

[services.hook]
port = 8002

[services.monitor]
port = 8003
collection_interval_secs = 30

[services.notify]
port = 8004

[services.wrap]
port = 8005
backend = "docker"

[services.orchestrator]
port = 8006
backend = "docker"
communicate_url = "http://communicate.internal:8010"
reconcile_interval_secs = 60

[services.memory]
port = 8008
embedding_provider = "openai"
embedding_model = "text-embedding-3-small"
lance_path = "/data/agentd/memory/lancedb"

[services.ui]
port = 8009

[services.communicate]
port = 8010

[services.mcp]
orchestrator_url = "http://orchestrator.internal:8006"
notify_url = "http://notify.internal:8004"
ask_url = "http://ask.internal:8001"
memory_url = "http://memory.internal:8008"
communicate_url = "http://communicate.internal:8010"
wrap_url = "http://wrap.internal:8005"
monitor_url = "http://monitor.internal:8003"
hook_url = "http://hook.internal:8002"
```

#### Docker / Container Config

When services run in separate containers and communicate over a shared network:

```toml
[general]
log_level = "info"
log_format = "json"
host = "0.0.0.0"        # bind on all interfaces inside the container

[services.ask]
orchestrator_url = "http://orchestrator:17006"

[services.orchestrator]
backend = "subprocess"  # no tmux inside containers
communicate_url = "http://communicate:17010"

[services.memory]
lance_path = "/data/lancedb"

[services.mcp]
orchestrator_url = "http://orchestrator:17006"
notify_url       = "http://notify:17004"
ask_url          = "http://ask:17001"
memory_url       = "http://memory:17008"
communicate_url  = "http://communicate:17010"
wrap_url         = "http://wrap:17005"
monitor_url      = "http://monitor:17003"
hook_url         = "http://hook:17002"
```

Pair with a `docker-compose.yml` that uses `AGENTD_CONFIG=/etc/agentd/config.toml` or volume-mounts the file.

### Migration Guide: From Env-Var-Only to TOML

If you have been configuring agentd purely through environment variables, migrating to a TOML file is straightforward and non-breaking.

#### Step 1 - Generate the default file

```bash
agent config init
```

#### Step 2 - Verify current resolved config

```bash
# With your current env vars still set:
agent config show
```

#### Step 3 - Move settings into the file

For each environment variable you have set, find the corresponding TOML key in the [Complete Key Reference](#complete-key-reference) table above and add it to your config file.

**Example:** If you have these environment variables:

```bash
export AGENTD_ORCHESTRATOR_BACKEND=docker
export AGENTD_MEMORY_EMBEDDING_PROVIDER=openai
export AGENTD_LOG_LEVEL=debug
```

Add the equivalent TOML:

```toml
[general]
log_level = "debug"

[services.orchestrator]
backend = "docker"

[services.memory]
embedding_provider = "openai"
```

#### Step 4 - Remove the environment variables

Once the TOML file is in place, unset the corresponding environment variables. Environment variables always win over the file, so leaving them set will shadow the file values.

```bash
unset AGENTD_ORCHESTRATOR_BACKEND
unset AGENTD_MEMORY_EMBEDDING_PROVIDER
unset AGENTD_LOG_LEVEL
```

#### Step 5 - Verify again

```bash
agent config show
```

The output should match what you had before.

#### Notes

- You do not need to migrate all variables at once. The three-layer system means you can move settings gradually.
- Environment variables are still the right tool for secrets (API keys, tokens) and per-deployment overrides that differ between machines.
- The config file is best for settings that are stable across deployments and should be version-controlled.

### Validation

Every service validates its configuration section at startup. If a setting is invalid (for example, port `0` or an unrecognised backend name), the service exits with a clear error message listing all invalid fields at once.

Common validation rules:

- **port**: Must be non-zero (`1`-`65535`)
- **backend**: Must be one of `tmux`, `docker`, `pty`, `subprocess`
- **embedding_provider (memory)**: Must be one of `none`, `ollama`, `openai`
- **URL fields**: Must start with `http://` or `https://`
- **hook.history_size**: Must be greater than `0`
- **monitor.collection_interval_secs**: Must be greater than `0`
- **orchestrator.reconcile_interval_secs**: Must be greater than `0`
- **ui.ui_dir**: Must not be empty

Example error output:

```
Error: configuration validation failed:
  [services.orchestrator]: orchestrator.backend must be one of tmux, docker, pty, subprocess; got: invalid
  [services.memory]: memory.embedding_provider must be one of none, ollama, openai; got: huggingface
```
