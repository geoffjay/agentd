# Getting Started with agentd

This guide walks you through a complete workflow from first run to managing autonomous agents. By the end, you'll understand how all the agentd services work together.

## Prerequisites

Before starting, make sure you have:

- **macOS 14+** or **Linux** with systemd
- **Rust 1.75+** — install from [rustup.rs](https://rustup.rs/)
- **tmux** — `brew install tmux` (macOS) or `apt install tmux` (Linux)
- **curl** and **jq** — for testing API endpoints
- **Claude Code** — install from [claude.ai/download](https://claude.ai/download) (required for agent orchestration)

## 1. First Run (~5 minutes)

### Clone and build

```bash
git clone https://github.com/geoffjay/agentd.git
cd agentd
cargo build --workspace
```

### Start the core services

Open three terminal windows (or tmux panes) and start the services:

```bash
# Terminal 1 — Notification service (port 17004)
cargo run -p agentd-notify
```

```bash
# Terminal 2 — Ask service (port 17001)
cargo run -p agentd-ask
```

```bash
# Terminal 3 — Orchestrator (port 17006)
cargo run -p agentd-orchestrator
```

!!! tip "Alternative: start all at once"
    If you've already installed with `cargo xtask install-user`, you can start all services with:
    ```bash
    cargo xtask start-services
    cargo xtask service-status
    ```

### Verify health endpoints

In a new terminal, check that each service is running:

```bash
curl -s http://localhost:17004/health | jq
```

Expected output:
```json
{
  "status": "ok",
  "service": "agentd-notify",
  "version": "0.2.0"
}
```

```bash
curl -s http://localhost:17001/health | jq
```

Expected output:
```json
{
  "status": "ok",
  "service": "agentd-ask",
  "version": "0.2.0",
  "notification_service_url": "http://localhost:17004"
}
```

```bash
curl -s http://localhost:17006/health | jq
```

Expected output:
```json
{
  "status": "ok",
  "agents_active": 0
}
```

If all three respond, you're ready to go!

---

## 2. Notifications — Your First Workflow

The notification system is the simplest starting point. It stores and manages messages between services and users.

### Create a notification

Using the CLI (build it first if you haven't installed):

```bash
cargo run -p cli -- notify create \
  --title "Welcome" \
  --message "agentd is running!" \
  --priority normal
```

Expected output:
```
Notification created successfully!

================================================================================
ID: 550e8400-e29b-41d4-a716-446655440000
Title: Welcome
Message: agentd is running!
Priority: normal
Status: pending
...
================================================================================
```

Or using curl directly:

```bash
curl -s -X POST http://localhost:17004/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "source": {"type": "system"},
    "lifetime": {"type": "persistent"},
    "priority": "normal",
    "title": "Build Complete",
    "message": "All tests passed on main branch",
    "requires_response": false
  }' | jq
```

### List notifications

```bash
cargo run -p cli -- notify list
```

You'll see a table of all notifications with their IDs, titles, priorities, and statuses.

### Filter by status

```bash
# Only show pending notifications
cargo run -p cli -- notify list --status pending

# Only show actionable notifications (pending or viewed, not expired)
cargo run -p cli -- notify list --actionable
```

### Respond to a notification

First, create a notification that requires a response:

```bash
cargo run -p cli -- notify create \
  --title "Deploy to production?" \
  --message "Main branch has 5 new commits ready for release" \
  --priority high \
  --requires-response
```

Copy the notification ID from the output, then respond:

```bash
cargo run -p cli -- notify respond <NOTIFICATION_ID> "Approved, ship it!"
```

The notification status changes from `pending` → `responded`.

### Clean up

```bash
cargo run -p cli -- notify delete <NOTIFICATION_ID>
```

---

## 3. Spawning an Agent

The orchestrator manages AI agents running in tmux sessions. This is the core of agentd.

### Create an agent

```bash
curl -s -X POST http://localhost:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "explorer",
    "working_dir": "'$(pwd)'",
    "system_prompt": "You are a helpful coding assistant.",
    "prompt": "List the top-level files and give a one-sentence summary of this project."
  }' | jq
```

Expected output:
```json
{
  "id": "a1b2c3d4-...",
  "name": "explorer",
  "status": "running",
  "config": {
    "working_dir": "/path/to/agentd",
    "shell": "zsh",
    "interactive": false,
    "worktree": false
  },
  "tmux_session": "agentd-orch-a1b2c3d4-...",
  "created_at": "...",
  "updated_at": "..."
}
```

Using the CLI:

```bash
agent orchestrator create-agent \
  --name explorer \
  --prompt "List the top-level files and give a one-sentence summary of this project."
```

Or use a YAML template (recommended for reproducible setups):

```bash
# Apply a template file
agent apply .agentd/agents/worker.yml

# Apply an entire project directory (agents + workflows)
agent apply .agentd/
```

See the [Template Reference](templates.md) for the full YAML schema.

### What happens under the hood

When you create an agent, the orchestrator:

1. Creates a record in its SQLite database
2. Starts a new **tmux session** named `agentd-orch-<agent-id>`
3. Launches `claude` inside that session with `--sdk-url` pointing back to the orchestrator's WebSocket endpoint
4. The Claude Code process connects to `ws://127.0.0.1:17006/ws/<agent-id>`
5. The orchestrator sends the initial prompt via WebSocket
6. Agent output is broadcast to monitoring streams at `/stream/<agent-id>`

### See the agent running

List the tmux sessions:

```bash
tmux list-sessions | grep agentd-orch
```

Attach to the agent's session to see it working:

```bash
# Use the tmux_session value from the create response
tmux attach -t agentd-orch-<agent-id>
```

Press `Ctrl-b d` to detach without killing the agent.

### Monitor agent output

Stream real-time output with colored formatting:

```bash
# Watch a specific agent
agent orchestrator stream <agent-id>

# Watch all agents
agent orchestrator stream --all

# Raw JSON output for piping
agent orchestrator stream --all --json
```

Press Ctrl+C to disconnect. Messages are formatted by type: assistant text, tool usage, results, and permission requests.

### Attach to an agent

Connect directly to the agent's tmux session for interactive debugging:

```bash
agent orchestrator attach --name my-agent
# or by ID:
agent orchestrator attach <agent-id>
```

Press `Ctrl-b d` to detach without killing the agent.

### Send follow-up messages

After the agent completes its first task, send it more work:

```bash
agent orchestrator send-message <agent-id> "Now count the lines of Rust code across all crates."

# Or pipe multi-line prompts from stdin:
echo "Review all files in src/ for security issues" | \
  agent orchestrator send-message <agent-id> --stdin
```

SDK-mode agents stay alive between tasks — you can keep sending messages.

### Check agent status

```bash
cargo run -p cli -- orchestrator list-agents
```

Or filter by status:

```bash
cargo run -p cli -- orchestrator list-agents --status running
```

### Terminate the agent

```bash
cargo run -p cli -- orchestrator delete-agent <agent-id>
```

This kills the tmux session and marks the agent as `stopped`.

### Restrict tool access

Control which tools an agent can use with tool policies:

```bash
# Set a read-only policy on a running agent
agent orchestrator set-policy <agent-id> '{"mode":"allow_list","tools":["Read","Grep","Glob"]}'

# Or set it at creation time
agent orchestrator create-agent --name safe --tool-policy '{"mode":"deny_list","tools":["Bash"]}'

# Require human approval for every tool use
agent orchestrator set-policy <agent-id> '{"mode":"require_approval"}'

# Then manage approvals as they come in
agent orchestrator list-approvals
agent orchestrator approve <approval-id>
```

See the [Tool Policies Guide](tool-policies.md) for the full reference.

---

## 4. Automated Workflows

Workflows connect an agent to a task source (like GitHub Issues) so the agent automatically picks up and works on new tasks.

### Prerequisites

- A running agent (created in step 3)
- A GitHub repository you have access to
- The `gh` CLI authenticated (`gh auth login`)

### Create a worker agent

Create an agent without an initial prompt — the workflow will send tasks:

```bash
AGENT=$(curl -s -X POST http://localhost:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "issue-worker",
    "working_dir": "'$(pwd)'",
    "system_prompt": "You are a development agent. Implement the task described in each prompt."
  }')
AGENT_ID=$(echo "$AGENT" | jq -r '.id')
echo "Agent created: $AGENT_ID"
```

Wait about 10 seconds for the agent to connect via WebSocket.

### Create a workflow

```bash
curl -s -X POST http://localhost:17006/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "auto-issues",
    "agent_id": "'$AGENT_ID'",
    "trigger_config": {
      "type": "github_issues",
      "owner": "YOUR_ORG",
      "repo": "YOUR_REPO",
      "labels": ["agent"],
      "state": "open"
    },
    "prompt_template": "Work on GitHub issue #{{source_id}}: {{title}}\n\n{{body}}\n\nURL: {{url}}",
    "poll_interval_secs": 120,
    "enabled": true
  }' | jq
```

Replace `YOUR_ORG` and `YOUR_REPO` with your actual GitHub organization and repository.

### How it works

1. The scheduler polls the GitHub API every 120 seconds
2. When it finds a new issue matching the labels, it renders the prompt template with the issue data
3. It sends the rendered prompt to the agent via WebSocket
4. After the agent completes the task, it picks up the next unprocessed issue

### Monitor workflow activity

View dispatch history to see which issues have been processed:

```bash
cargo run -p cli -- orchestrator workflow-history <WORKFLOW_ID>
```

Each dispatch shows: source ID (issue number), status (dispatched/completed/failed), prompt sent, and timestamps.

### Pause or stop a workflow

```bash
# Pause (keeps the configuration, stops polling)
curl -s -X PUT http://localhost:17006/workflows/<WORKFLOW_ID> \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}'

# Delete entirely
cargo run -p cli -- orchestrator delete-workflow <WORKFLOW_ID>
```

---

## 5. The Ask Service

The ask service monitors your environment and creates interactive notifications when it detects something worth asking about.

### Trigger a check

```bash
cargo run -p cli -- ask trigger
```

Or via curl:

```bash
curl -s -X POST http://localhost:17001/trigger | jq
```

Expected output:
```json
{
  "checks_run": ["tmux_sessions"],
  "notifications_sent": [],
  "check_results": {
    "tmux_sessions": {
      "sessions_running": true,
      "session_count": 2
    }
  }
}
```

If no tmux sessions are running, the ask service creates a notification asking if you'd like to start one. You'll see it in `notifications_sent`.

### Answer a question

If the ask service created a question notification:

```bash
cargo run -p cli -- ask answer <QUESTION_ID> "yes"
```

---

## 6. Where to Go Next

### API Documentation

- [Orchestrator API](services/orchestrator.md) — Full REST and WebSocket endpoint reference
- [Notify API](services/notify.md) — Notification CRUD endpoints

### Production Deployment

For running agentd as persistent background services:

```bash
# Install binaries and service definitions
cargo xtask install-user

# Start all services (uses launchd on macOS, systemd on Linux)
cargo xtask start-services

# Check status
cargo xtask service-status
```

Production services use ports 7001-7006 (configured in plist/unit files), while development defaults to ports 17001-17006.

### Shell completions

Enable tab completion for the `agent` CLI:

```bash
# Bash
agent completions bash > ~/.local/share/bash-completion/completions/agent

# Zsh (add ~/.zfunc to fpath in .zshrc)
agent completions zsh > ~/.zfunc/_agent

# Fish
agent completions fish > ~/.config/fish/completions/agent.fish
```

Or install all completions at once: `cargo xtask install-completions`

See [Installation Guide](install.md) for detailed setup instructions.

### Port Reference

agentd uses a dual-port scheme: **dev ports (17xxx)** when running with `cargo run`, and **production ports (7xxx)** when installed as a LaunchAgent (macOS) or systemd unit (Linux).

| Service | Dev Port | Prod Port |
|---------|----------|-----------|
| agentd-ask | 17001 | 7001 |
| agentd-hook | 17002 | 7002 |
| agentd-monitor | 17003 | 7003 |
| agentd-notify | 17004 | 7004 |
| agentd-wrap | 17005 | 7005 |
| agentd-orchestrator | 17006 | 7006 |
| agentd-memory | — | 7008 |
| agentd-communicate | 17010 | 7010 |

The `agent` CLI defaults to **production ports** (7xxx). If your services are running on dev ports, set the URL overrides:

```bash
source .env   # sets all AGENTD_*_SERVICE_URL vars to dev ports
```

Or override a single service:

```bash
AGENTD_COMMUNICATE_SERVICE_URL=http://localhost:17010 agent communicate health
```

See [Configuration Reference](configuration.md) for the full list of environment variables.

---

## Troubleshooting

### `agent status` shows services as down when they are running

The `agent status` command checks **production ports (7xxx)** by default. If you're running services with `cargo run` (dev ports 17xxx), status checks will fail even though services are healthy.

Check which ports your services are actually on:

```bash
# Test dev ports directly
curl -s http://localhost:17004/health   # notify (dev)
curl -s http://localhost:17006/health   # orchestrator (dev)

# Test production ports
curl -s http://localhost:7004/health    # notify (prod)
curl -s http://localhost:7006/health    # orchestrator (prod)
```

If dev services are healthy but `agent status` shows them down, source the `.env` file to point the CLI at dev ports:

```bash
source .env
agent status
```

See [issue #536](https://github.com/geoffjay/agentd/issues/536) — a code fix is in progress to make `agent status` port-scheme-aware.

### "Connection refused" when hitting health endpoints

The service isn't running, or you're checking the wrong port. Check:

```bash
# Is the process running?
ps aux | grep agentd

# Are services on dev ports (cargo run) or prod ports (installed)?
curl -s http://localhost:17004/health   # dev
curl -s http://localhost:7004/health    # prod
```

If another process holds the port, either stop it or override with `AGENTD_PORT=18004 cargo run -p agentd-notify`.

### "tmux not found" when creating agents

Install tmux:

```bash
# macOS
brew install tmux

# Linux (Debian/Ubuntu)
sudo apt install tmux
```

### Agent shows "pending" but never starts running

The Claude Code CLI may not be installed or not on your PATH:

```bash
which claude
```

If not found, install it from [claude.ai/download](https://claude.ai/download).

### Notification service URL mismatch

If the ask service can't reach the notify service, set the URL explicitly:

```bash
AGENTD_NOTIFY_SERVICE_URL=http://localhost:17004 cargo run -p agentd-ask
```

### Database errors on first run

The SQLite databases are created automatically. If you see permission errors:

```bash
# Check the data directory exists and is writable
ls -la ~/Library/Application\ Support/agentd-notify/   # macOS
ls -la ~/.local/share/agentd-notify/                     # Linux
```

### Agent WebSocket connection fails

Make sure the orchestrator is running *before* creating agents. The agent process needs to connect back to `ws://127.0.0.1:17006/ws/<id>`.

Check the orchestrator logs:
```bash
# If running via xtask
tail -f /usr/local/var/log/agentd-orchestrator.err

# If running in terminal, check the terminal output
```
