---
name: agent-orchestrator
description: Manage the full agent lifecycle, workflows, approvals, tool policies, and runtime configuration through the orchestrator service.
---

# Agent Orchestrator

Skill for interacting with the agentd orchestrator service — the central coordinator for AI agents and autonomous workflows.

## Agent Lifecycle

### Creating Agents

```bash
# Basic agent
agent orchestrator create-agent \
  --name my-agent \
  --working-dir /path/to/project \
  --model claude-sonnet-4-6

# With an initial prompt (starts working immediately)
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --model opus \
  --prompt "Review the latest PR"

# With system prompt for persistent instructions
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --model sonnet \
  --system-prompt "You are a code review agent. Focus on security issues."

# With worktree isolation (creates a git worktree)
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --worktree \
  --model sonnet

# With environment variables
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --model sonnet \
  --env GITHUB_TOKEN=xxx \
  --env RUST_LOG=debug

# Docker-backed agent with resource limits
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --model sonnet \
  --docker-image agentd-runner:latest \
  --cpu-limit 2.0 \
  --memory-limit 4096

# Read prompt from stdin
echo "Fix the failing tests" | agent orchestrator create-agent \
  --name fixer \
  --working-dir . \
  --model sonnet \
  --stdin

# Auto-attach to the agent's tmux session after creation
agent orchestrator create-agent \
  --name my-agent \
  --working-dir . \
  --model sonnet \
  --attach
```

Model aliases: `sonnet` → `claude-sonnet-4-6`, `opus` → `claude-opus-4-6`, `haiku` → `claude-haiku-4-5-20251001`

### Listing and Inspecting Agents

```bash
# List all agents
agent orchestrator list-agents

# Filter by status (pending, running, stopped, failed)
agent orchestrator list-agents --status running

# Get detailed info for a specific agent
agent orchestrator get-agent <agent-id>

# JSON output for scripting
agent orchestrator list-agents --json
```

### Interacting with Running Agents

```bash
# Send a message/prompt to a running agent
agent orchestrator send-message <agent-id> "Please review this file"

# Send a message from stdin
echo "Fix the bug in auth.rs" | agent orchestrator send-message <agent-id> --stdin

# Attach to agent's tmux session (interactive terminal)
agent orchestrator attach <agent-id>
agent orchestrator attach --name my-agent

# Stream real-time output via WebSocket
agent orchestrator stream <agent-id>
agent orchestrator stream <agent-id> --verbose
agent orchestrator stream --all

# View Docker agent logs
agent orchestrator logs <agent-id>
agent orchestrator logs <agent-id> --follow
agent orchestrator logs --name my-agent --tail 200

# Get usage statistics (tokens, cost, turns, duration)
agent orchestrator usage <agent-id>
agent orchestrator usage --name my-agent

# Clear context and start fresh
agent orchestrator clear-context <agent-id>
agent orchestrator clear-context --name my-agent
```

### Changing Model at Runtime

```bash
agent orchestrator set-model <agent-id> --model sonnet
agent orchestrator set-model --name my-agent --model opus
agent orchestrator set-model <agent-id> --model opus --restart
agent orchestrator set-model <agent-id> --clear  # Remove model override
```

### Directory Management

```bash
# Add an additional directory to an agent's accessible paths
agent orchestrator add-dir <agent-id> /path/to/extra/dir

# Remove an additional directory
agent orchestrator remove-dir <agent-id> /path/to/extra/dir
```

### Deleting Agents

```bash
agent orchestrator delete-agent <agent-id>

# Batch delete stopped agents
agent orchestrator list-agents --status stopped --json | jq -r '.[].id' | xargs -I{} agent orchestrator delete-agent {}
```

## Tool Policies

Control what tools an agent can use.

```bash
# View current policy
agent orchestrator get-policy <agent-id>

# Allow all tools (default)
agent orchestrator set-policy <agent-id> '{"type": "AllowAll"}'

# Deny all tools (text-only mode)
agent orchestrator set-policy <agent-id> '{"type": "DenyAll"}'

# Only allow specific tools
agent orchestrator set-policy <agent-id> '{"type": "AllowList", "tools": ["Read", "Grep", "Glob"]}'

# Allow all except specific tools
agent orchestrator set-policy <agent-id> '{"type": "DenyList", "tools": ["Bash", "Write"]}'

# Require human approval for every tool call
agent orchestrator set-policy <agent-id> '{"type": "RequireApproval"}'
```

## Approvals

When an agent has `RequireApproval` policy, tool calls queue for human review.

```bash
# List pending approvals
agent orchestrator list-approvals
agent orchestrator list-approvals --agent-id <agent-id>
agent orchestrator list-approvals --status pending

# Approve or deny
agent orchestrator approve <approval-id>
agent orchestrator deny <approval-id>
```

Approvals time out after 5 minutes by default.

## Workflows

Workflows automate task dispatch to agents from external sources.

### Creating Workflows

```bash
# GitHub issues workflow
agent orchestrator create-workflow \
  --name issue-worker \
  --agent-name worker \
  --trigger-type github-issues \
  --owner geoffjay \
  --repo agentd \
  --labels agent \
  --state open \
  --poll-interval 60 \
  --prompt-template 'Work on issue #{{source_id}}: {{title}}\n\n{{body}}'

# GitHub pull requests workflow
agent orchestrator create-workflow \
  --name pr-reviewer \
  --agent-name reviewer \
  --trigger-type github-pull-requests \
  --owner geoffjay \
  --repo agentd \
  --labels review \
  --state open

# Cron-based workflow
agent orchestrator create-workflow \
  --name nightly-check \
  --agent-name checker \
  --trigger-type cron \
  --cron-expression "0 2 * * *"

# Webhook-triggered workflow
agent orchestrator create-workflow \
  --name deploy-hook \
  --agent-name deployer \
  --trigger-type webhook \
  --webhook-secret mysecret

# Manual trigger only
agent orchestrator create-workflow \
  --name on-demand \
  --agent-name worker \
  --trigger-type manual
```

Template variables: `{{source_id}}`, `{{title}}`, `{{body}}`, `{{url}}`, `{{labels}}`

### Managing Workflows

```bash
agent orchestrator list-workflows
agent orchestrator get-workflow <workflow-id>
agent orchestrator update-workflow <workflow-id> --enabled true
agent orchestrator update-workflow <workflow-id> --enabled false
agent orchestrator update-workflow <workflow-id> --poll-interval 120
agent orchestrator delete-workflow <workflow-id>
```

### Dispatch History and Manual Triggers

```bash
# View dispatch history
agent orchestrator workflow-history <workflow-id>

# Manually trigger a workflow
agent orchestrator trigger-workflow <workflow-id> --title "Manual task" --body "Do this thing"

# Validate a prompt template
agent orchestrator validate-template '{{source_id}} - {{title}}: {{body}}'
agent orchestrator validate-template --file template.txt
```

## Health Check

```bash
agent orchestrator health
```
