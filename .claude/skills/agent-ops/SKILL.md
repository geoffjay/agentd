---
name: agent-ops
description: Create, monitor, message, and debug agents via the CLI. Use when creating agents, checking agent status, sending messages to running agents, attaching to tmux sessions, streaming agent output, or managing tool policies and approvals.
---

# Agent Operations

Operational skill for managing agents through the agentd CLI.

## Creating Agents

```bash
# Basic agent creation
agent orchestrator create-agent \
  --name my-agent \
  --working-dir /path/to/project \
  --model claude-sonnet-4-6

# With system prompt
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --model claude-sonnet-4-6 \
  --system-prompt "You are a code review agent."

# With initial prompt (starts working immediately)
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --model opus \
  --prompt "Review the latest PR"

# With worktree isolation
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --worktree \
  --model sonnet

# JSON output for scripting
agent orchestrator create-agent --name test --working-dir . --model sonnet --json
```

Model aliases: `sonnet` → `claude-sonnet-4-6`, `opus` → `claude-opus-4-6`, `haiku` → `claude-haiku-4-5-20251001`

## Checking Agent Status

```bash
# List all agents
agent orchestrator list-agents

# Filter by status
agent orchestrator list-agents --status running
agent orchestrator list-agents --status pending
agent orchestrator list-agents --status stopped
agent orchestrator list-agents --status failed

# Get specific agent details
agent orchestrator get-agent <agent-id>

# JSON output
agent orchestrator list-agents --json
```

## Interacting with Running Agents

```bash
# Send a message/prompt to an agent
agent orchestrator send-message <agent-id> "Please review this file"

# Attach to agent's tmux session (interactive)
agent orchestrator attach <agent-id>

# Stream agent output (real-time monitoring)
agent orchestrator stream <agent-id>

# Stream all agents
agent orchestrator stream --all
```

## Tool Policy Management

```bash
# View current policy
agent orchestrator get-policy <agent-id>

# Set policy: allow all tools (default)
agent orchestrator set-policy <agent-id> '{"type": "AllowAll"}'

# Set policy: deny all tools (text-only)
agent orchestrator set-policy <agent-id> '{"type": "DenyAll"}'

# Set policy: only allow specific tools
agent orchestrator set-policy <agent-id> '{"type": "AllowList", "tools": ["Read", "Grep", "Glob"]}'

# Set policy: allow all except specific tools
agent orchestrator set-policy <agent-id> '{"type": "DenyList", "tools": ["Bash", "Write"]}'

# Set policy: require human approval for every tool
agent orchestrator set-policy <agent-id> '{"type": "RequireApproval"}'
```

## Approval Management

When an agent has `RequireApproval` policy:

```bash
# List pending approvals
agent orchestrator list-approvals
agent orchestrator list-approvals --agent-id <agent-id>

# Approve a tool request
agent orchestrator approve <approval-id>

# Deny a tool request
agent orchestrator deny <approval-id>
```

Approvals timeout after 5 minutes by default.

## Changing Model at Runtime

```bash
agent orchestrator model <agent-id> sonnet
agent orchestrator model <agent-id> opus
agent orchestrator model <agent-id> claude-sonnet-4-6
```

## Deleting Agents

```bash
# Delete a single agent
agent orchestrator delete-agent <agent-id>

# List then delete stopped agents
agent orchestrator list-agents --status stopped --json | jq -r '.[].id' | xargs -I{} agent orchestrator delete-agent {}
```

## Troubleshooting

### Agent stuck in Pending
1. Check orchestrator logs for errors
2. Verify tmux is installed: `tmux -V`
3. Check if tmux sessions are being created: `tmux list-sessions`

### Agent not responding to messages
1. Verify agent is Running: `agent orchestrator get-agent <id>`
2. Check WebSocket connection: stream the agent output
3. Attach to tmux session to see the agent's terminal directly

### Agent failed
1. Get agent details: `agent orchestrator get-agent <id>`
2. Check tmux session: `tmux list-sessions`
3. Check orchestrator logs for error details
4. Capture tmux pane output: `tmux capture-pane -t <session-name> -p`
