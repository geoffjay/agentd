# Ask Service (agentd-ask) - Detailed Plan

## Overview

The Ask Service (agentd-ask) is a daemon that orchestrates AI agent workflows through human-in-the-loop interactions. It monitors registered projects, proactively asks users questions via notifications, and manages agent sessions in tmux environments.

## Scope

- **Single-user**: Designed for one user per machine
- **Hands-off monitoring**: Launches agents in tmux sessions; user attaches manually
- **Cross-platform**: Linux, macOS, Windows support

## Target Agent Integrations

Initial support for:

- **Claude Code**: Anthropic's official CLI agent
- **opencode**: Open-source coding agent
- **Gemini CLI**: Google's Gemini command-line interface

## Core Concept

agentd-ask acts as an "agent agent" that has some special abilities:

- Maintains a registry of projects and their locations
- Periodically polls project states
- Sends notifications to request user input
- Manages tmux sessions for running agents
- Tracks active agent sessions and their lifecycle

## Technology Stack

### Language & Core

- **Language**: Rust
- **Runtime**: Tokio async runtime
- **Database**: SQLite (via sqlx)
- **Configuration**: TOML format
- **Platform Support**: Linux, macOS (Windows support is not planned)

### Key Crates

Note that in a lot of cases these should be taken from the root using, for example `tokio = { workspace = true }`.

```toml
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
clap = { version = "4", features = ["derive"] }
axum = "0.7"
reqwest = { version = "0.11", features = ["json"] }
tmux_interface = "0.3"
directories = "5.0"
walkdir = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1.0"
thiserror = "1.0"
```

## Architecture

### Project Structure

```
crates/ask/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, CLI parsing
│   ├── lib.rs               # Library exports
│   ├── config.rs            # TOML configuration handling
│   ├── db/
│   │   ├── mod.rs
│   │   ├── schema.rs        # SQL schema definitions
│   │   └── models.rs        # Data models (Project, AgentSession, etc.)
│   ├── api/
│   │   ├── mod.rs
│   │   ├── server.rs        # HTTP API server (axum)
│   │   ├── client.rs        # HTTP client for CLI
│   │   └── commands.rs      # CLI command handlers
│   ├── daemon/
│   │   ├── mod.rs
│   │   ├── poller.rs        # Poll cycle logic
│   │   ├── scanner.rs       # Project discovery/scanning
│   │   ├── session.rs       # tmux session management
│   │   └── service.rs       # systemd/launchd service management
│   └── agent/
│       ├── mod.rs
│       └── launcher.rs      # Agent launching (agentd-wrap integration)
```

## Data Models

### Database Schema

#### projects

Stores registered projects (manually registered or auto-discovered).

```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL UNIQUE,
    location_id INTEGER,           -- NULL if manually registered
    type TEXT,                     -- coding, writing, research, etc.
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_active TIMESTAMP,
    is_active BOOLEAN DEFAULT FALSE,
    metadata TEXT,                 -- JSON string for flexible data
    FOREIGN KEY (location_id) REFERENCES project_locations(id)
);
```

#### project_locations

Parent directories that can be scanned for projects containing `.agentd.toml`.

```sql
CREATE TABLE project_locations (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    scan_enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

#### agent_sessions

Tracks running and historical agent sessions.

```sql
CREATE TABLE agent_sessions (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL,
    agent_type TEXT NOT NULL,
    tmux_session_name TEXT NOT NULL,
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    ended_at TIMESTAMP,
    model_provider TEXT NOT NULL,    -- ollama, openai, anthropic, etc.
    model_name TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);
```

#### user_preferences

Future expansion: store learned user preferences.

```sql
CREATE TABLE user_preferences (
    id INTEGER PRIMARY KEY,
    project_id INTEGER,
    preference_key TEXT NOT NULL,
    preference_value TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id),
    UNIQUE(project_id, preference_key)
);
```

### Rust Data Models

```rust
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub project_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProjectLocation {
    pub id: i64,
    pub path: String,
    pub scan_enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: i64,
    pub project_id: i64,
    pub agent_type: String,
    pub tmux_session_name: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub model_provider: String,
    pub model_name: String,
}
```

## Configuration

### File Locations (via `directories` crate)

All paths are platform-specific:

**Linux:**

- Config: `~/.config/agentd/ask.toml`
- Data: `~/.local/share/agentd/ask.db`
- Cache: `~/.cache/agentd/ask/`
- Logs: `~/.local/share/agentd/ask.log`
- PID: `~/.local/share/agentd/ask.pid`

**macOS:**

- Config: `~/Library/Application Support/Agent/ask.toml`
- Data: `~/Library/Application Support/Agent/ask.db`
- Cache: `~/Library/Caches/Agent/ask/`

### config.toml Structure

```toml
[daemon]
poll_interval_minutes = 1
enable_filesystem_watch = false  # Future feature

[notifications]
# agentd-notify service endpoint
service_endpoint = "http://localhost:7004"

[tmux]
session_prefix = "ask"
default_shell = "/bin/bash"

[agents]
default_provider = "ollama"
default_model = "gpt-oss:120b-cloud"
# Available agent types: claude-code, opencode, gemini
available_types = ["claude-code", "opencode", "gemini", "general"]

[database]
# Automatically set to platform-specific data directory
# path = "~/.local/share/agentd/ask.db"
```

## CLI Interface

### Commands

```bash
# Project management
agent ask register <name> <path>           # Register individual project
agent ask register-location <path>         # Register location to scan
agent ask list [--active-only]             # List projects
agent ask locations                        # List registered locations

# Session management
agent ask start <project-name> [--agent-type <type>]
agent ask stop <project-name>
agent ask active                           # Show active sessions
agent ask status                           # Daemon status
```

### HTTP API (IPC)

The daemon runs an HTTP server on `127.0.0.1:7001` for CLI commands to communicate:

```
GET  /health                          # Health check
GET  /projects                        # List projects
POST /projects                        # Register project
POST /projects/:name/start            # Start agent session
POST /projects/:name/stop             # Stop agent session
GET  /locations                       # List locations
POST /locations                       # Register location
GET  /sessions                        # List sessions
GET  /status                          # Daemon status
```

## Daemon Management

### Linux (systemd)

Service file: `~/.config/systemd/user/agentd-ask.service`

```ini
[Unit]
Description=Agentd Ask Service - Agent orchestration daemon
After=network.target

[Service]
Type=simple
ExecStart=/path/to/agentd-ask
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user daemon-reload
systemctl --user enable agentd-ask
systemctl --user start agentd-ask
```

### macOS (launchd)

macOS installation has already been added, see contrib/plists for existing plist files. Nothing is
required to do for this section.

## Project Discovery

### Manual Registration

Users can explicitly register projects:

```bash
agent ask register my-project ~/code/my-project
```

### Location Scanning

Register a parent directory to scan:

```bash
agent ask register-location ~/projects/
```

The daemon scans for projects containing `.agentd.toml`:

```toml
# .agentd.toml
name = "my-awesome-project"
agent_type = "coding"
model_provider = "ollama"
model_name = "codellama"
```

If `name` is not specified, the directory name is used.

## Workflow

### Daemon Poll Cycle

1. **Check for registered projects**

   - If none → Send notification: "No projects registered. Would you like to register one?"
   - If exists → Continue

2. **Check for active projects**

   - Query `agent_sessions` for sessions without `ended_at`
   - Verify tmux sessions still exist
   - If no active sessions → Continue

3. **Send notification if no active sessions**

   - "No active projects. Would you like to start one?"
   - List all registered projects as options

4. **Sleep until next poll interval**

### Starting a Project

When user selects a project to start:

1. **Ask for agent type**

   - Present list from `config.agents.available_types`
   - Or use project's `.agentd.toml` default

2. **Ask for model provider**

   - Options: Ollama (local), OpenAI, Anthropic, etc.
   - Use config default or project preference

3. **Ask for model name**

   - Based on provider selection
   - Use config default or project preference

4. **Create tmux session**

   - Name: `askd-{project-name}-{timestamp}`
   - Working directory: project path
   - Detached mode

5. **Launch agent**

   - Via `agentd-wrap` (details TBD)
   - Pass configuration to agent

6. **Record session**

   - Insert into `agent_sessions` table
   - Set `project.is_active = true`
   - Set `project.last_active = now()`

7. **Monitor session**
   - Background task checks if tmux session exists
   - When session ends, set `ended_at` timestamp

### Stopping a Project

When user runs `agent ask stop <project>`:

1. **Kill tmux session**

   - Find session by name in database
   - Execute `tmux kill-session -t {name}`

2. **Update database**
   - Set `agent_sessions.ended_at = now()`
   - Set `project.is_active = false`

## Tmux Session Management

### Session Naming

Format: `{prefix}-{project-name}-{timestamp}`

Example: `agentd-ask-my-project-20241102-143022`

### Session Operations

```rust
pub struct SessionManager {
    prefix: String,
}

impl SessionManager {
    pub fn create_session(&self, name: &str, working_dir: &str) -> Result<()>
    pub fn session_exists(&self, name: &str) -> Result<bool>
    pub fn kill_session(&self, name: &str) -> Result<()>
    pub async fn monitor_session(&self, name: &str) -> Result<()>
}
```

### Session Lifecycle

1. **Created**: tmux session starts, entry in `agent_sessions`
2. **Running**: `ended_at` is NULL, tmux session exists
3. **Ended**: Either:
   - User exits tmux session naturally
   - User runs `agent ask stop`
   - Daemon detects session no longer exists
4. **Cleaned up**: `ended_at` set, `project.is_active = false`

## Notifications

### Integration Point

agentd-ask sends notifications to agentd-notify service:

```rust
pub struct NotificationClient {
    endpoint: String,        // agentd-notify base URL
    client: reqwest::Client,
}

impl NotificationClient {
    pub async fn send(&self, notification: Notification) -> Result<String> {
        // POST to agentd-notify /notifications endpoint
        // Returns notification ID
    }

    pub async fn wait_for_response(&self, notification_id: &str, timeout_secs: u64) -> Result<NotificationResponse> {
        // GET /notifications/:id/response with timeout
    }
}
```

Note that the agentd-notify service is already implemented in crates/notify. There is already a
client implementation for this purpose and a new one DOES NOT need to be implemented.

### Notification Structure

agentd-ask uses the notification API defined by agentd-notify. See [agentd-notify-plan.md](./agentd-notify-plan.md) for full API specification.

Example notification payload:

```json
{
  "source_service": "agentd-ask",
  "title": "Start a Project?",
  "message": "No active projects. Would you like to start one?",
  "notification_type": "question",
  "urgency": "normal",
  "actions": [
    {
      "action_id": "select-project",
      "label": "Select project",
      "action_type": "select",
      "action_data": {
        "options": [
          { "value": "project-a", "label": "Project A" },
          { "value": "project-b", "label": "Project B" }
        ]
      }
    }
  ],
  "metadata": {
    "context": "poll-cycle",
    "projects": [
      { "name": "project-a", "path": "/home/user/projects/a" },
      { "name": "project-b", "path": "/home/user/projects/b" }
    ]
  }
}
```

## Agent Integration

### agentd-wrap Integration

agentd-ask uses agentd-wrap to launch agent CLIs. Initial support for:

- **Claude Code**: Anthropic's official CLI
- **opencode**: Open-source coding agent
- **Gemini CLI**: Google's Gemini CLI

Expected interface:

```bash
# agentd-ask will invoke:
agent wrap launch \
  --agent-type claude-code \
  --model-provider anthropic \
  --model-name claude-sonnet-4.5 \
  --project-path /path/to/project
```

agentd-wrap responsibilities:

- Tmux session management
- Agent startup success/failure reporting
- Agent health monitoring
- Exit code reporting
- Agent-specific configuration handling

### Session Interaction Model

**Current scope**: Hands-off monitoring

- agentd-ask launches agent in tmux session
- User attaches to tmux session manually to interact
- agentd-ask monitors session lifecycle only

**Future enhancements** (documented for future consideration):

- Programmatic agent I/O (send/receive messages via API)
- Agent-to-agent communication
- Multi-agent coordination within projects

### Multi-Agent Support (Future)

Initial version: **single agent per project**

Future expansion:

- Multiple agents per project
- Git worktrees for each agent
- Separate tmux sessions or windows
- Agent coordination/communication
- Hierarchical agent structures

Design must not prevent this expansion.

## Future Enhancements

### Learning from User Preferences

Track user choices in `user_preferences` table:

- Preferred agent type per project
- Preferred model provider/name per project type
- Time-of-day patterns
- Frequently paired projects

Use this data to:

- Reduce repetitive questions
- Auto-start common workflows
- Suggest optimal configurations

### Filesystem Watching

Currently: polling only

Future: React to filesystem events:

- New commits → "Would you like to review changes?"
- Build failures → "Build failed. Start debugging agent?"
- New files → "New file detected. Need assistance?"

Implementation via `notify` crate (already in dependencies).

### Context Awareness

Enhance decision-making with:

- Git branch/status awareness
- Project dependencies (multi-project workflows)
- Time-of-day heuristics (morning = resume work)
- Priority levels
- Deadline awareness

### Advanced Session Management

- Session templates per project type
- Multi-pane layouts
- Session state snapshots
- Session migration between machines

## Implementation Phases

### Phase 1: Core Foundation (MVP)

- [ ] Database schema and migrations
- [ ] Configuration loading with `directories` crate
- [ ] Basic CLI (register, list, start, stop)
- [ ] HTTP API server
- [ ] tmux session management
- [ ] Simple polling loop
- [ ] Basic notification sending (stub)

### Phase 2: Project Discovery

- [ ] `.agentd.toml` parsing
- [ ] Location scanning
- [ ] Auto-registration of discovered projects
- [ ] Project metadata handling

### Phase 3: Daemon Management

- [ ] systemd service generation (Linux)
- [ ] launchd plist generation (macOS)
- [ ] Install/uninstall commands
- [ ] PID file management
- [ ] Proper daemonization

### Phase 4: Agent Integration

- [ ] agentd-wrap interface definition
- [ ] Agent launching
- [ ] Session monitoring
- [ ] Health checks
- [ ] Error handling

### Phase 5: Enhanced Decision Making

- [ ] User preference tracking
- [ ] Smart defaults based on history
- [ ] Project type detection
- [ ] Context-aware suggestions

### Phase 6: Advanced Features

- [ ] Filesystem watching
- [ ] Multi-agent support
- [ ] Git worktree integration
- [ ] Session templates

## Testing Strategy

### Unit Tests

- Database operations (CRUD)
- Configuration parsing
- tmux command generation
- Session name formatting

### Integration Tests

- Full workflow: register → start → stop
- Database migrations
- HTTP API endpoints
- Notification sending

### System Tests

- Daemon startup/shutdown
- Service installation
- Cross-platform compatibility
- Long-running stability

## Security Considerations

1. **Database**: SQLite file permissions (user-only)
2. **API Server**: Localhost only (127.0.0.1)
3. **PID File**: Prevent multiple daemon instances
4. **Project Paths**: Validate paths, prevent traversal
5. **Notification Endpoint**: Validate URL, HTTPS recommended

## Performance Considerations

1. **Polling Interval**: Configurable (default 15 minutes)
2. **Database Queries**: Indexed on frequently queried columns
3. **Session Monitoring**: Efficient tmux existence checks
4. **Scanning**: Rate-limited, skip hidden directories
5. **HTTP Timeouts**: Reasonable defaults for notification service

## Dependencies on Other Services

### Notification Service (agentd-notify)

- **Status**: Design complete (see [agentd-notify-plan.md](./agentd-notify-plan.md))
- **Interface**: HTTP JSON API
- **Required capabilities**:
  - Display notifications with actions
  - Handle user responses
  - Queue management
  - Response polling with timeout

### Agent Wrapper (agentd-wrap)

- **Status**: To be designed
- **Interface**: CLI tool
- **Required capabilities**:
  - Launch agents with configuration (claude-code, opencode, gemini)
  - Monitor agent health
  - Report exit codes
  - Agent-specific configuration handling

## Success Metrics

1. **Reduces friction**: Starting an agent session takes < 30 seconds
2. **Increases engagement**: Users actively work on more projects
3. **Improves awareness**: Users notified of inactive projects
4. **Enables automation**: Common workflows can be one-click
5. **Maintains context**: Project state preserved across sessions

## Open Questions

1. Should agentd-ask be aware of agent capabilities, or is that agentd-wrap's job?
   - **Leaning toward**: agentd-wrap handles agent-specific details
2. ~~How should multi-user systems be handled (if at all)?~~
   - **Resolved**: Single-user only for initial scope
3. Should there be a web UI in addition to CLI?
   - **Future consideration**: Focus on CLI + notifications first
4. What level of agent monitoring is agentd-ask responsible for?
   - **Resolved**: Lifecycle only (start/stop/exists); agentd-wrap handles health
5. Should agentd-ask have plugin/extension support?
   - **Future consideration**: Not in initial scope

## Conclusion

The Ask Service (agentd-ask) provides a foundational layer for human-agent interaction by:

- Maintaining project awareness
- Proactively engaging users via agentd-notify
- Managing agent lifecycles in tmux sessions
- Tracking usage patterns and session history
- Supporting multiple agent types (Claude Code, opencode, Gemini CLI)

It's designed to be extensible, allowing for future enhancements (multi-agent coordination, programmatic I/O, agent-to-agent communication) while maintaining a clean, focused initial implementation.
