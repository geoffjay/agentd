---
name: template-ops
description: Apply, teardown, and validate declarative YAML templates for agents and workflows. Use when working with .agentd/ template files, deploying agent+workflow stacks, or debugging template application issues.
---

# Template Operations

Operational skill for working with agentd declarative YAML templates.

## Template Directory Structure

```
.agentd/
├── agents/
│   ├── worker.yml        # Agent definitions
│   ├── reviewer.yml
│   └── planner.yml
├── workflows/
│   ├── issue-worker.yml  # Workflow definitions referencing agents by name
│   └── pull-request-reviewer.yml
└── templates/
    └── *.yml             # Reusable prompt/config templates
```

## Applying Templates

```bash
# Apply entire directory (agents first, then workflows)
agent apply .agentd/

# Apply a single agent
agent apply .agentd/agents/worker.yml

# Apply a single workflow (agent must already exist)
agent apply .agentd/workflows/issue-worker.yml

# Dry run — validate without creating
agent apply .agentd/ --dry-run

# Custom timeout for agent startup
agent apply .agentd/ --wait-timeout 120
```

### Application Order
1. Agent templates are created first
2. System waits for each agent to reach Running status (default 60s timeout)
3. Workflow templates are created, referencing agents by name
4. Workflows are automatically enabled on creation

## Tearing Down Templates

```bash
# Teardown entire directory (workflows first, then agents)
agent teardown .agentd/

# Teardown a single resource
agent teardown .agentd/agents/worker.yml
```

### Teardown Order (reverse of apply)
1. Workflows are deleted first
2. Then agents are deleted (stops tmux sessions)

## Agent Template Format

```yaml
# .agentd/agents/worker.yml
name: worker                    # Unique name (used by workflow references)
working_dir: "."                # Working directory (. = project root)
shell: /bin/zsh                 # Shell for tmux session
worktree: false                 # Use git worktree isolation
model: claude-sonnet-4-6        # Model (or alias: sonnet, opus, haiku)

system_prompt: |
  You are a worker agent for the project.
  ...instructions...
```

Required fields: `name`, `working_dir`, `model`
Optional fields: `shell`, `worktree`, `system_prompt`, `env` (map of env vars)

## Workflow Template Format

```yaml
# .agentd/workflows/issue-worker.yml
name: issue-worker
agent: worker                   # References agent by name (not ID)

source:
  type: github_issues           # or github_pull_requests
  owner: geoffjay
  repo: agentd
  labels:
    - agent                     # Filter by labels
  state: open                   # Filter by state

poll_interval: 60               # Seconds between polls
enabled: true

prompt_template: |
  Work on issue #{{source_id}}: {{title}}
  URL: {{url}}
  Labels: {{labels}}
  {{body}}
```

Required fields: `name`, `agent`, `source`, `prompt_template`
Optional fields: `poll_interval` (default 60), `enabled` (default true), `tool_policy`

## Troubleshooting

### Apply times out waiting for agent
1. Increase timeout: `agent apply .agentd/ --wait-timeout 120`
2. Check orchestrator logs for agent startup errors
3. Verify tmux is available: `tmux -V`
4. Check if agent was partially created: `agent orchestrator list-agents`

### Workflow references unknown agent
- The agent name in `agent:` field must match a running agent's name
- If applying a directory, agents are created first — this should work automatically
- If applying a single workflow file, ensure the agent exists and is Running

### Template validation errors
- Use `--dry-run` to check templates before applying
- Verify YAML syntax with a linter
- Check that all required fields are present
- Ensure model names are valid (sonnet, opus, haiku, or full model IDs)

### Teardown doesn't clean up everything
- Teardown only removes resources defined in the template files
- Manually check: `agent orchestrator list-agents` and `agent orchestrator list-workflows`
- Tmux sessions should be killed when agents are deleted

## Examples

The project includes example templates:

```bash
# Apply the full agentd project example
agent apply examples/agentd-project/

# Check what it created
agent orchestrator list-agents
agent orchestrator list-workflows

# Tear it down
agent teardown examples/agentd-project/
```
