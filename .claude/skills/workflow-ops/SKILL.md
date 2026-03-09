---
name: workflow-ops
description: Create, test, and debug workflows and dispatches. Use when creating workflows, checking dispatch history, validating prompt templates, enabling/disabling workflows, or debugging task source polling.
---

# Workflow Operations

Operational skill for managing workflows and task dispatching.

## Creating Workflows

```bash
# Create a workflow that polls GitHub issues
agent orchestrator create-workflow \
  --name issue-worker \
  --agent-name worker \
  --source-type github_issues \
  --owner geoffjay \
  --repo agentd \
  --labels agent \
  --state open \
  --poll-interval 60 \
  --prompt-template 'Work on issue #{{source_id}}: {{title}}\n\n{{body}}'

# Create a PR review workflow
agent orchestrator create-workflow \
  --name pr-reviewer \
  --agent-name reviewer \
  --source-type github_pull_requests \
  --owner geoffjay \
  --repo agentd \
  --labels review \
  --state open \
  --poll-interval 120

# With tool policy override
agent orchestrator create-workflow \
  --name safe-worker \
  --agent-name worker \
  --source-type github_issues \
  --owner geoffjay \
  --repo agentd \
  --tool-policy '{"type": "AllowList", "tools": ["Read", "Grep", "Glob"]}'
```

## Managing Workflows

```bash
# List all workflows
agent orchestrator list-workflows

# Get workflow details
agent orchestrator get-workflow <workflow-id>

# Enable/disable a workflow
agent orchestrator update-workflow <workflow-id> --enabled true
agent orchestrator update-workflow <workflow-id> --enabled false

# Delete a workflow
agent orchestrator delete-workflow <workflow-id>
```

## Dispatch History

```bash
# View dispatch history for a workflow
agent orchestrator workflow-history <workflow-id>

# JSON output for analysis
agent orchestrator workflow-history <workflow-id> --json
```

Dispatch statuses: `dispatched`, `completed`, `failed`

## Validating Prompt Templates

```bash
# Check template syntax
agent orchestrator validate-template '{{source_id}} - {{title}}: {{body}}'
```

Available template variables:
- `{{source_id}}` — GitHub issue/PR number
- `{{title}}` — Issue/PR title
- `{{body}}` — Issue/PR body text
- `{{url}}` — GitHub URL
- `{{labels}}` — Comma-separated label list

## Troubleshooting

### Workflow not dispatching tasks
1. Check workflow is enabled: `agent orchestrator get-workflow <id>`
2. Verify the referenced agent is Running: `agent orchestrator list-agents --status running`
3. Check GitHub source has matching items:
   ```bash
   gh issue list --repo geoffjay/agentd --label agent --state open
   gh pr list --repo geoffjay/agentd --label review --state open
   ```
4. Check dispatch history for duplicates — same source_id won't be dispatched twice
5. Check orchestrator logs: `RUST_LOG=agentd_orchestrator::scheduler=debug`

### Dispatch stuck in "dispatched" state
1. The agent may still be working — check with `agent orchestrator stream <agent-id>`
2. If the agent has disconnected, the dispatch won't complete
3. Check the agent's tmux session: `agent orchestrator attach <agent-id>`

### Wrong prompt sent to agent
1. Validate the template: `agent orchestrator validate-template '...'`
2. Check dispatch history to see the actual prompt sent
3. Verify the GitHub issue/PR has the expected content

## Declarative Workflow Templates

Workflows can be defined as YAML files in `.agentd/workflows/`:

```yaml
name: issue-worker
agent: worker                    # References agent by name
source:
  type: github_issues
  owner: geoffjay
  repo: agentd
  labels:
    - agent
  state: open
poll_interval: 60
enabled: true
prompt_template: |
  Work on issue #{{source_id}}: {{title}}
  {{body}}
```

Apply with: `agent apply .agentd/workflows/issue-worker.yml`
