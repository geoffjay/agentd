# agentd Configuration Reference

agentd uses a three-layer configuration system. Settings are resolved in order of ascending precedence:

```
compiled defaults  <  config file  <  environment variables
```

Any layer can be omitted. A missing config file is silently ignored - compiled defaults and environment variables still apply.

## Quick Start

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

## Config File Location

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

## Precedence Rules

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

## Complete Key Reference

All configuration keys with their TOML path, environment variable, default value, and description.

### General Settings

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `general.log_level` | `AGENTD_LOG_LEVEL` | `info` | Log level filter: `trace`, `debug`, `info`, `warn`, `error` |
| `general.log_format` | `AGENTD_LOG_FORMAT` | `text` | Log output format: `text` (human-readable) or `json` (structured) |
| `general.host` | `AGENTD_HOST` | `127.0.0.1` | Default bind address for all services |

### agentd-core (port 17000)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.core.port` | `AGENTD_CORE_PORT` | `17000` | HTTP listen port |

### agentd-ask (port 17001)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.ask.port` | `AGENTD_ASK_PORT` | `17001` | HTTP listen port |
| `services.ask.orchestrator_url` | `AGENTD_ASK_ORCHESTRATOR_URL` | `http://localhost:17006` | Orchestrator callback URL |

### agentd-hook (port 17002)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.hook.port` | `AGENTD_HOOK_PORT` | `17002` | HTTP listen port |
| `services.hook.history_size` | `AGENTD_HISTORY_SIZE` | `500` | Maximum shell-event history retained in memory |
| `services.hook.notify_service_url` | `AGENTD_NOTIFY_SERVICE_URL` | *(unset)* | Optional notify service URL for forwarding events |

### agentd-monitor (port 17003)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.monitor.port` | `AGENTD_MONITOR_PORT` | `17003` | HTTP listen port |
| `services.monitor.collection_interval_secs` | `AGENTD_COLLECTION_INTERVAL_SECS` | `15` | Metrics collection interval in seconds |

### agentd-notify (port 17004)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.notify.port` | `AGENTD_NOTIFY_PORT` | `17004` | HTTP listen port |

### agentd-wrap (port 17005)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.wrap.port` | `AGENTD_WRAP_PORT` | `17005` | HTTP listen port |
| `services.wrap.backend` | `AGENTD_WRAP_BACKEND` | `tmux` | Execution backend: `tmux`, `docker`, `pty`, `subprocess` |

### agentd-orchestrator (port 17006)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.orchestrator.port` | `AGENTD_ORCHESTRATOR_PORT` | `17006` | HTTP listen port |
| `services.orchestrator.backend` | `AGENTD_ORCHESTRATOR_BACKEND` | `tmux` | Execution backend: `tmux`, `docker`, `pty`, `subprocess` |
| `services.orchestrator.communicate_url` | `AGENTD_ORCHESTRATOR_COMMUNICATE_URL` | `http://localhost:17010` | Communicate service URL for agent message delivery |
| `services.orchestrator.reconcile_interval_secs` | `AGENTD_RECONCILE_INTERVAL_SECS` | `30` | Agent reconciliation interval in seconds |

### agentd-memory (port 17008)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.memory.port` | `AGENTD_MEMORY_PORT` | `17008` | HTTP listen port |
| `services.memory.embedding_provider` | `AGENTD_MEMORY_EMBEDDING_PROVIDER` | `none` | Embedding provider: `none`, `ollama`, `openai` |
| `services.memory.embedding_model` | `AGENTD_MEMORY_EMBEDDING_MODEL` | `text-embedding-3-small` | Embedding model name |
| `services.memory.lance_path` | `AGENTD_MEMORY_LANCE_PATH` | XDG data dir | LanceDB storage directory |

### agentd-ui (port 17009)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.ui.port` | `AGENTD_UI_PORT` | `17009` | HTTP listen port |
| `services.ui.ui_dir` | `AGENTD_UI_DIR` | `./ui/dist` | Directory containing compiled frontend assets |

### agentd-communicate (port 17010)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.communicate.port` | `AGENTD_COMMUNICATE_PORT` | `17010` | HTTP listen port |

### agentd-index (port 17012)

| TOML Key | Environment Variable | Default | Description |
|----------|---------------------|---------|-------------|
| `services.index.port` | `AGENTD_INDEX_PORT` | `17012` | HTTP listen port |
| `services.index.embedding_provider` | `AGENTD_INDEX_EMBEDDING_PROVIDER` | `ollama` | Embedding provider: `ollama`, `openai` |
| `services.index.embedding_model` | `AGENTD_INDEX_EMBEDDING_MODEL` | `nomic-embed-text` | Embedding model name |
| `services.index.lance_path` | `AGENTD_INDEX_LANCE_PATH` | XDG data dir | LanceDB storage directory |
| `services.index.languages` | `AGENTD_INDEX_LANGUAGES` | `rust,python,javascript,typescript` | Languages to index (env var: comma-separated string) |

### agentd-mcp (stdio transport)

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

## Per-Service Configuration Sections

### Execution Backends

Both `agentd-orchestrator` and `agentd-wrap` support four execution backends. Set `backend` to one of:

| Backend | Description | Use when |
|---------|-------------|----------|
| `tmux` | Launches agents in tmux sessions (default) | Local development, interactive attach |
| `subprocess` | Launches agents as direct child processes with stdio | Lightweight, no tmux required |
| `pty` | Launches agents in a pseudo-terminal | Interactive terminal emulation |
| `docker` | Launches agents in Docker containers | Isolation, reproducible environments |

### Embedding Providers

**agentd-memory** supports:
- `none` - No embeddings; memory search uses exact-match only
- `ollama` - Local Ollama instance (default model: `nomic-embed-text`)
- `openai` - OpenAI API (default model: `text-embedding-3-small`)

**agentd-index** supports:
- `ollama` - Local Ollama instance (default; default model: `nomic-embed-text`)
- `openai` - OpenAI API (default model: `text-embedding-3-large`)

### LanceDB Paths

The `lance_path` for both memory and index services defaults to the platform-specific XDG data directory:

- **Linux:** `~/.local/share/agentd-memory/lancedb` and `~/.local/share/agentd-index/lancedb`
- **macOS:** `~/Library/Application Support/agentd-memory/lancedb` and `~/Library/Application Support/agentd-index/lancedb`

Override with the environment variable or a TOML entry when you want vector data on a specific volume.

### Code Indexing Languages

`services.index.languages` lists the programming languages that `agentd-index` will parse and chunk. Supported values:

| Language name | File extensions indexed |
|---------------|------------------------|
| `rust` | `.rs` |
| `python` | `.py` |
| `javascript` | `.js`, `.jsx`, `.mjs` |
| `typescript` | `.ts`, `.tsx` |

Any other string is treated as a literal file extension (e.g. `"go"` indexes `.go` files).

When setting via environment variable, use a comma-separated list:

```bash
AGENTD_INDEX_LANGUAGES=rust,python,go
```

### Hook Notify Service URL

`services.hook.notify_service_url` is optional (no default). When set, the hook service forwards notable shell events to the notify service at that URL. Leave it unset to disable this integration.

```toml
[services.hook]
notify_service_url = "http://localhost:17004"
```

## Example Configurations

### Minimal Config (override just a few values)

```toml
# ~/.config/agentd/config.toml

[general]
log_level = "debug"

[services.orchestrator]
backend = "subprocess"
```

### Development Config

All services on localhost with verbose logging:

```toml
[general]
log_level = "debug"
log_format = "text"
host = "127.0.0.1"

[services.orchestrator]
backend = "tmux"
reconcile_interval_secs = 10

[services.index]
embedding_provider = "ollama"
embedding_model = "nomic-embed-text"

[services.memory]
embedding_provider = "ollama"
embedding_model = "nomic-embed-text"

[services.hook]
history_size = 1000
notify_service_url = "http://localhost:17004"

[services.monitor]
collection_interval_secs = 5
```

### Production Config (custom ports, external URLs)

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

[services.index]
port = 8012
embedding_provider = "openai"
embedding_model = "text-embedding-3-large"
lance_path = "/data/agentd/index/lancedb"
languages = ["rust", "python", "typescript", "go"]

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

### Docker / Container Config

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

[services.index]
lance_path = "/data/index-lancedb"

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

## Migration Guide: From Env-Var-Only to TOML

If you have been configuring agentd purely through environment variables, migrating to a TOML file is straightforward and non-breaking.

### Step 1 - Generate the default file

```bash
agent config init
```

### Step 2 - Verify current resolved config

```bash
# With your current env vars still set:
agent config show
```

### Step 3 - Move settings into the file

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

### Step 4 - Remove the environment variables

Once the TOML file is in place, unset the corresponding environment variables. Environment variables always win over the file, so leaving them set will shadow the file values.

```bash
unset AGENTD_ORCHESTRATOR_BACKEND
unset AGENTD_MEMORY_EMBEDDING_PROVIDER
unset AGENTD_LOG_LEVEL
```

### Step 5 - Verify again

```bash
agent config show
```

The output should match what you had before.

### Notes

- You do not need to migrate all variables at once. The three-layer system means you can move settings gradually.
- Environment variables are still the right tool for secrets (API keys, tokens) and per-deployment overrides that differ between machines.
- The config file is best for settings that are stable across deployments and should be version-controlled.

## Validation

Every service validates its configuration section at startup. If a setting is invalid (for example, port `0` or an unrecognised backend name), the service exits with a clear error message listing all invalid fields at once.

Common validation rules:

- **port**: Must be non-zero (`1`-`65535`)
- **backend**: Must be one of `tmux`, `docker`, `pty`, `subprocess`
- **embedding_provider (memory)**: Must be one of `none`, `ollama`, `openai`
- **embedding_provider (index)**: Must be one of `ollama`, `openai`
- **URL fields**: Must start with `http://` or `https://`
- **hook.history_size**: Must be greater than `0`
- **monitor.collection_interval_secs**: Must be greater than `0`
- **orchestrator.reconcile_interval_secs**: Must be greater than `0`
- **ui.ui_dir**: Must not be empty
- **index.languages**: Must contain at least one language

Example error output:

```
Error: configuration validation failed:
  [services.orchestrator]: orchestrator.backend must be one of tmux, docker, pty, subprocess; got: invalid
  [services.memory]: memory.embedding_provider must be one of none, ollama, openai; got: huggingface
```
