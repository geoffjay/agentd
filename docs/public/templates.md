# YAML Template Reference

agentd supports declarative YAML templates for defining agents and workflows. Templates live in a `.agentd/` directory in your project root and can be applied with a single command.

## Directory Convention

```
.agentd/
├── agents/
│   ├── planner.yml       # planning agent
│   └── worker.yml        # worker agent
└── workflows/
    └── issue-worker.yml  # GitHub issue workflow (references worker agent)
```

- **`agents/`** — Agent definitions. Each file creates one agent.
- **`workflows/`** — Workflow definitions. Each file creates one workflow that references an agent by name.

## Commands

### Apply

Create agents and workflows from templates:

```bash
# Apply entire project (agents first, wait for running, then workflows)
agent apply .agentd/

# Apply a single file
agent apply .agentd/agents/worker.yml
agent apply .agentd/workflows/issue-worker.yml

# Validate without creating anything
agent apply --dry-run .agentd/

# Custom timeout for agent startup (default: 60s)
agent apply --wait-timeout 120 .agentd/
```

**Apply order for directories:**

1. Parse and validate all templates (fail fast — no partial creates on error)
2. Create agents from `agents/*.yml`
3. Wait for all agents to reach `running` status
4. Create workflows from `workflows/*.yml`, resolving agent name references
5. Print summary

If an agent with the same name is already running, it is reused (not duplicated).

### Teardown

Delete resources in reverse order (workflows first, then agents):

```bash
agent teardown .agentd/
agent teardown --dry-run .agentd/   # preview what would be deleted
```

---

## Agent Template Schema

File: `.agentd/agents/<name>.yml`

```yaml
# Required
name: worker                    # Agent name (must be unique)

# Optional — all have sensible defaults
working_dir: "."                # Resolved relative to YAML file location
shell: zsh                      # Shell to use (default: zsh)
interactive: false              # Interactive mode (default: false)
worktree: false                 # Use git worktree (default: false)

# Optional — grant access to directories outside working_dir
additional_dirs:
  - ../shared-libraries         # relative: resolved relative to this YAML file
  - /opt/company/configs        # absolute: used as-is
  - ~/other-project             # tilde: expanded to home directory

# Optional — no default
prompt: "Analyze the codebase"  # Initial prompt sent after agent connects
system_prompt: |                # System prompt for the agent session
  You are a code review agent.
  Focus on security and performance.

# Optional — defaults to allow_all
tool_policy:
  mode: allow_list              # allow_all | deny_all | allow_list | deny_list | require_approval
  tools:                        # Only for allow_list / deny_list modes
    - Read
    - Grep
    - Glob
```

### Field Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | **required** | Unique agent name |
| `working_dir` | string | `"."` | Working directory. `"."` resolves to `$PWD` |
| `additional_dirs` | list | `[]` | Extra directories the agent can access (see [Additional Directories](additional-dirs.md)) |
| `shell` | string | `"zsh"` | Shell for the tmux session |
| `interactive` | bool | `false` | Run in interactive mode (no WebSocket) |
| `worktree` | bool | `false` | Start with `--worktree` for isolated git worktree |
| `prompt` | string | none | Initial prompt sent via WebSocket after connection |
| `system_prompt` | string | none | System prompt for the Claude session |
| `tool_policy` | object | `allow_all` | Tool use restrictions (see [Tool Policies](#tool-policies)) |

### Working Directory Resolution

- `"."` → resolves to the current working directory (`$PWD`) at apply time
- Relative paths → resolved relative to the YAML file's directory
- Absolute paths → used as-is

The same resolution rules apply to each entry in `additional_dirs`. See [Additional Directories](additional-dirs.md) for full details.

---

## Workflow Template Schema

File: `.agentd/workflows/<name>.yml`

```yaml
# Required
name: issue-worker              # Unique workflow name
agent: worker                   # Agent NAME (resolved to UUID at apply time)
source:
  type: github_issues
  owner: myorg
  repo: myrepo
  labels:                       # Optional — filter issues by label
    - agent
  state: open                   # Optional — default: open

# Required (one of these)
prompt_template: |              # Inline prompt with {{variables}}
  Fix issue #{{source_id}}: {{title}}
  {{body}}
# OR
prompt_template_file: ../prompts/worker.txt   # Relative to YAML file

# Optional
poll_interval: 60               # Seconds between polls (default: 60)
enabled: true                   # Start polling immediately (default: true)

# Optional — defaults to allow_all
tool_policy:
  mode: allow_all
```

### Field Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | **required** | Unique workflow name |
| `agent` | string | **required** | Agent name (resolved to UUID via API) |
| `source` | object | **required** | Task source configuration |
| `prompt_template` | string | see below | Inline prompt template |
| `prompt_template_file` | string | see below | Path to external template file |
| `poll_interval` | integer | `60` | Seconds between source polls |
| `enabled` | bool | `true` | Start polling on creation |
| `tool_policy` | object | `allow_all` | Tool policy applied before each dispatch |

One of `prompt_template` or `prompt_template_file` is required.

### Template Variables

Available `{{placeholders}}` in prompt templates:

**Task fields** — present for all trigger types:

| Variable | Description | Example |
|----------|-------------|---------|
| `{{title}}` | Task title | `"Fix login bug"` |
| `{{body}}` | Task body | Full markdown content |
| `{{url}}` | Source URL | `"https://github.com/org/repo/issues/42"` |
| `{{labels}}` | Comma-separated labels | `"bug, auth"` |
| `{{assignee}}` | Assigned user (or empty) | `"alice"` |
| `{{source_id}}` | Source identifier | `"42"` |
| `{{metadata}}` | All metadata as `key: value` lines | `"fire_time: 2026-04-01T09:00:00Z"` |

**Schedule trigger variables** — populated by `cron` and `delay` triggers:

| Variable | Trigger | Description | Example |
|----------|---------|-------------|---------|
| `{{fire_time}}` | `cron` | RFC 3339 timestamp when the cron fired | `2026-04-01T09:00:00Z` |
| `{{cron_expression}}` | `cron` | The cron expression that fired | `0 9 * * MON-FRI` |
| `{{run_at}}` | `delay` | Scheduled datetime from the trigger config | `2026-04-01T09:00:00Z` |
| `{{workflow_id}}` | `delay` | UUID of the workflow | `550e8400-...` |

!!! note
    Schedule trigger variables are stored in the task's `metadata` map and resolved during template rendering. If a variable is referenced but not present for the trigger type (e.g. `{{fire_time}}` in a delay workflow), the placeholder is left as-is in the rendered prompt.

Validate templates before creating workflows:

```bash
agent orchestrator validate-template "Fix: {{title}}\n{{body}}"
agent orchestrator validate-template --file ./my-template.txt
```

### Source Configuration

Supported sources:

**GitHub Issues:**
```yaml
source:
  type: github_issues
  owner: myorg           # GitHub user or organization
  repo: myrepo           # Repository name
  labels: [bug, agent]   # Filter by labels (optional)
  state: open            # Issue state filter (default: open)
```

**Cron (recurring schedule):**
```yaml
source:
  type: cron
  expression: "0 9 * * MON-FRI"   # 9 AM UTC on weekdays
```

**Delay (one-shot):**
```yaml
source:
  type: delay
  run_at: "2026-04-01T09:00:00Z"  # RFC 3339 datetime
```

See [Schedule Triggers](schedule-triggers.md) for full syntax reference, common expression examples, and operational notes.

---

## Tool Policies

Control which tools an agent can use. Set on agents at creation or on workflows for dispatch-time enforcement.

| Mode | YAML | Effect |
|------|------|--------|
| Allow all | `mode: allow_all` | No restrictions (default) |
| Deny all | `mode: deny_all` | Block all tool usage |
| Allow list | `mode: allow_list` | Only listed tools permitted |
| Deny list | `mode: deny_list` | All tools except listed ones |
| Require approval | `mode: require_approval` | Human must approve each tool use |

**Example — read-only agent:**
```yaml
tool_policy:
  mode: allow_list
  tools:
    - Read
    - Grep
    - Glob
    - WebFetch
```

**Example — block dangerous tools:**
```yaml
tool_policy:
  mode: deny_list
  tools:
    - Bash
    - Write
    - Edit
```

**Example — human oversight:**
```yaml
tool_policy:
  mode: require_approval
```

With `require_approval`, every tool request is held pending until a human approves or denies it:

```bash
agent orchestrator list-approvals
agent orchestrator approve <APPROVAL_ID>
agent orchestrator deny <APPROVAL_ID>
```

Unanswered approvals auto-deny after 5 minutes.

---

## Complete Example

The agentd project itself uses templates in `.agentd/`:

### `.agentd/agents/worker.yml`

```yaml
name: worker
working_dir: "."
shell: /bin/zsh
worktree: false

system_prompt: |
  You are a worker agent for the agentd project. You will receive
  GitHub issues as tasks. For each issue:
  1. Read the issue carefully
  2. Plan your approach
  3. Implement the change
  4. Run tests
  5. Create a branch, commit, and push
  6. Create a PR using the gh CLI
```

### `.agentd/workflows/issue-worker.yml`

```yaml
name: issue-worker
agent: worker

source:
  type: github_issues
  owner: geoffjay
  repo: agentd
  labels: [agent]
  state: open

poll_interval: 60
enabled: true

prompt_template: |
  Work on the following GitHub issue:

  Issue #{{source_id}}: {{title}}
  URL: {{url}}
  Labels: {{labels}}

  Description:
  {{body}}

  Instructions:
  1. Create a feature branch: git checkout -b issue-{{source_id}}
  2. Implement the changes
  3. Run tests: cargo test
  4. Commit and push
  5. Create a PR: gh pr create --title "{{title}}" --body "Closes #{{source_id}}"
```

### Launch everything

```bash
agent apply .agentd/
```

This creates the worker agent, waits for it to connect, then creates the workflow that starts polling for GitHub issues.
