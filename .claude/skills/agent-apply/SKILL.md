---
name: agent-apply
description: Apply and teardown declarative YAML templates for agents and workflows. Use for deploying agent stacks from .agentd/ directories, validating templates, and cleaning up resources.
---

# Agent Apply

Skill for deploying and tearing down agentd resources from declarative YAML templates.

## Applying Templates

```bash
# Apply an entire .agentd/ directory (agents first, then workflows)
agent apply .agentd/

# Apply a single template file
agent apply .agentd/agents/worker.yml
agent apply .agentd/workflows/issue-worker.yml

# Dry run — validate without creating anything
agent apply .agentd/ --dry-run

# Custom timeout for waiting on agents to become Running
agent apply .agentd/ --wait-timeout 120
```

### Application Order
1. Agent templates are created first
2. System waits for each agent to reach `Running` status (default 60s timeout)
3. Workflow templates are created, referencing agents by name
4. Workflows are automatically enabled on creation

## Tearing Down Templates

```bash
# Teardown entire directory (workflows first, then agents)
agent teardown .agentd/

# Teardown a single resource
agent teardown .agentd/agents/worker.yml

# Dry run — show what would be deleted
agent teardown .agentd/ --dry-run
```

### Teardown Order (reverse of apply)
1. Workflows are deleted first
2. Agents are deleted (stops tmux sessions)

## Template Directory Structure

```
.agentd/
├── agents/
│   ├── worker.yml
│   ├── reviewer.yml
│   └── planner.yml
└── workflows/
    ├── issue-worker.yml
    └── pr-reviewer.yml
```

## Agent Template Format

```yaml
# .agentd/agents/worker.yml
name: worker
working_dir: "."
shell: /bin/zsh
worktree: false
model: claude-sonnet-4-6

system_prompt: |
  You are a worker agent for the project.
  Focus on implementing features and fixing bugs.

env:
  RUST_LOG: debug
  GITHUB_TOKEN: "${GITHUB_TOKEN}"
```

Required: `name`, `working_dir`, `model`
Optional: `shell`, `worktree`, `system_prompt`, `env`

## Workflow Template Format

```yaml
# .agentd/workflows/issue-worker.yml
name: issue-worker
agent: worker

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
  URL: {{url}}
  Labels: {{labels}}
  {{body}}
```

Required: `name`, `agent`, `source`, `prompt_template`
Optional: `poll_interval` (default 60), `enabled` (default true), `tool_policy`

### Template Variables
- `{{source_id}}` — GitHub issue/PR number
- `{{title}}` — issue/PR title
- `{{body}}` — issue/PR body text
- `{{url}}` — GitHub URL
- `{{labels}}` — comma-separated label list
