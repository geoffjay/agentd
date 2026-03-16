# agentd

agentd is a service platform for managing autonomous AI agents. It provides infrastructure for spawning, orchestrating, and monitoring Claude Code instances as long-running background processes, with support for automated workflows that dispatch work from external task sources like GitHub Issues.

## Services

### [Orchestrator](public/services/orchestrator.md)

The core service. Manages AI agent lifecycle through tmux sessions and exposes a REST + WebSocket API for:

- Creating and terminating agents
- Sending messages to running agents
- Monitoring agent output via WebSocket streams
- Configuring autonomous workflows that poll GitHub issues and dispatch them to agents

### [Notify](public/services/notify.md)

Notification service with a REST API for creating, reading, and managing notifications. Used for agent-to-user communication and status updates.

## Getting Started

### [Getting Started Guide](public/getting-started.md)

End-to-end walkthrough from first run to managing autonomous agents — notifications, agent spawning, automated workflows, and more.

### [Installation](public/install.md)

Install agentd from source, configure services, and verify everything is running.

### Quick Start

Start the orchestrator and create your first agent:

```bash
# Start the orchestrator
cargo run -p orchestrator

# Create an agent
curl -s -X POST http://127.0.0.1:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-agent",
    "working_dir": "/path/to/project",
    "system_prompt": "You are a helpful coding assistant."
  }' | jq

# Send it a task (use the agent id from the response above)
curl -s -X POST http://127.0.0.1:17006/agents/{id}/message \
  -H 'Content-Type: application/json' \
  -d '{"content": "List the files in this project and summarize what it does"}'

# Watch the output
websocat ws://127.0.0.1:17006/stream/{id}
```

### Autonomous Workflows

Set up an agent that automatically processes GitHub issues:

```bash
# 1. Create a worker agent (stays alive, waiting for tasks)
curl -s -X POST http://127.0.0.1:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "worker",
    "working_dir": "/path/to/project",
    "system_prompt": "You are a worker agent. Implement the issue described in each task."
  }' | jq

# 2. Create a workflow that polls for issues labeled "agent"
curl -s -X POST http://127.0.0.1:17006/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "issue-worker",
    "agent_id": "<agent-id-from-step-1>",
    "trigger_config": {
      "type": "github_issues",
      "owner": "your-org",
      "repo": "your-repo",
      "labels": ["agent"]
    },
    "prompt_template": "Work on issue #{{source_id}}: {{title}}\n\n{{body}}",
    "poll_interval_secs": 60
  }' | jq
```

The workflow polls every 60 seconds, picks up new issues with the specified label, and dispatches them to the agent one at a time.

## Architecture

agentd is structured as a Rust workspace with the following crates:

| Crate | Binary | Description |
|-------|--------|-------------|
| `orchestrator` | `agentd-orchestrator` | Agent lifecycle, WebSocket protocol, workflow scheduler |
| `notify` | `agentd-notify` | Notification REST API |
| `ask` | `agentd-ask` | Interactive question service |
| `cli` | `agent` | Command-line interface |
| `hook` | `agentd-hook` | Shell integration hooks (planned) |
| `monitor` | `agentd-monitor` | Service monitoring (planned) |
| `wrap` | `agentd-wrap` | Tmux session management REST API |

## Requirements

- macOS (tested on macOS 14+)
- Rust 1.75+
- [tmux](https://github.com/tmux/tmux) (for agent sessions)
- [gh CLI](https://cli.github.com/) (for GitHub workflow sources)
- [Claude Code](https://claude.ai/download) (the AI agent runtime)

## License

MIT OR Apache-2.0
