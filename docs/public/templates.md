# YAML Template Reference

agentd supports declarative YAML templates for defining agents and workflows. Templates live in a `.agentd/` directory in your project root and can be applied with a single command.

## Directory Convention

```
.agentd/
├── rooms/
│   └── engineering.yml   # room (created before agents)
├── agents/
│   ├── planner.yml       # planning agent (can reference rooms by name)
│   └── worker.yml        # worker agent
└── workflows/
    └── issue-worker.yml  # GitHub issue workflow (references worker agent)
```

- **`rooms/`** — Room definitions. Each file creates one room and its initial participants.
- **`agents/`** — Agent definitions. Each file creates one agent. Agents can list `rooms` to auto-join.
- **`workflows/`** — Workflow definitions. Each file creates one workflow that references an agent by name.

## Commands

### Apply

Create rooms, agents, and workflows from templates:

```bash
# Apply entire project (rooms first, then agents, then workflows)
agent apply .agentd/

# Apply a single file
agent apply .agentd/rooms/engineering.yml
agent apply .agentd/agents/worker.yml
agent apply .agentd/workflows/issue-worker.yml

# Validate without creating anything
agent apply --dry-run .agentd/

# Custom timeout for agent startup (default: 60s)
agent apply --wait-timeout 120 .agentd/
```

**Apply order for directories:**

1. Parse and validate all templates (fail fast — no partial creates on error)
2. Create rooms from `rooms/*.yml` (and add their initial participants)
3. Create agents from `agents/*.yml`, joining them to any listed rooms
4. Wait for all agents to reach `running` status
5. Create workflows from `workflows/*.yml`, resolving agent name references
6. Print summary

If a room or agent with the same name already exists, it is reused (not duplicated).

### Teardown

Delete resources in reverse order (workflows → agents → rooms):

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
| `rooms` | list | `[]` | Rooms the agent automatically joins at startup (see [Room Membership](#room-membership)) |

### Room Membership

The `rooms` field lists rooms the agent should automatically join when it starts. Each entry is either a plain room name (defaults to `member` role) or a structured object with an explicit role:

```yaml
rooms:
  - engineering                # plain string — member role
  - name: announcements
    role: observer             # read-only access
  - name: ops-channel
    role: admin
```

Rooms referenced here must exist before the agent starts. When using `agent apply .agentd/`, rooms listed in `.agentd/rooms/` are created first. If you apply an agent template independently, create the room first:

```bash
agent communicate create-room --name engineering --created-by cli
agent apply .agentd/agents/worker.yml
```

Available roles:

| Role | Can post | Can manage participants |
|------|----------|-------------------------|
| `member` | Yes | No |
| `admin` | Yes | Yes |
| `observer` | No (read-only) | No |

### Working Directory Resolution

- `"."` → resolves to the current working directory (`$PWD`) at apply time
- Relative paths → resolved relative to the YAML file's directory
- Absolute paths → used as-is

The same resolution rules apply to each entry in `additional_dirs`. See [Additional Directories](additional-dirs.md) for full details.

---

---

## Room Template Schema

File: `.agentd/rooms/<name>.yml`

```yaml
# Required
name: engineering               # Room name (must be unique)

# Optional
topic: "Engineering coordination"   # Short label shown in listings
description: |                      # Longer description
  General channel for engineering agents and humans.
type: group                         # direct | group (default) | broadcast

# Optional — participants added when the room is first created
participants:
  - identifier: alice               # Human username or agent UUID/name
    kind: human                     # agent (default) | human
    role: admin                     # member (default) | admin | observer
    display_name: "Alice"           # Optional; defaults to identifier
  - identifier: worker
    kind: agent
    role: member
```

### Field Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | **required** | Unique room name |
| `topic` | string | none | Short label |
| `description` | string | none | Longer description |
| `type` | string | `"group"` | `"direct"`, `"group"`, or `"broadcast"` |
| `participants` | list | `[]` | Initial participants added at creation time |

### Participant fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `identifier` | string | **required** | Agent UUID/name or human username |
| `kind` | string | `"agent"` | `"agent"` or `"human"` |
| `role` | string | `"member"` | `"member"`, `"admin"`, or `"observer"` |
| `display_name` | string | `identifier` | Display name shown in messages |

!!! note "Idempotent creation"
    If a room with the given name already exists, `agent apply` skips creation entirely — it does not add participants or update the topic/description. To modify an existing room, use the CLI or REST API directly.

### Room types

| Type | Who can post | Use case |
|------|-------------|----------|
| `group` | All members | Collaborative agent teams, human-agent coordination |
| `direct` | Both participants | One-to-one agent ↔ human or agent ↔ agent conversation |
| `broadcast` | Admins only | Status feeds, announcement channels |

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

**Webhook trigger variables** — populated by the `webhook` trigger:

| Variable | Trigger | Description | Example |
|----------|---------|-------------|---------|
| `{{delivery_id}}` | `webhook` | Delivery ID from `X-GitHub-Delivery` header or auto-generated UUID | `abc-123` |
| `{{timestamp}}` | `webhook` | RFC 3339 time the webhook was received | `2026-04-01T09:00:00Z` |
| `{{github_event}}` | `webhook` (GitHub) | GitHub event type | `issues` |
| `{{action}}` | `webhook` (GitHub) | GitHub action | `opened` |
| `{{issue_number}}` | `webhook` (GitHub issues) | Issue number | `42` |
| `{{pr_number}}` | `webhook` (GitHub PRs) | Pull request number | `99` |

**Event trigger variables** — populated by `agent_lifecycle` and `dispatch_result` triggers:

| Variable | Trigger | Description | Example |
|----------|---------|-------------|---------|
| `{{event_type}}` | `agent_lifecycle` | Lifecycle event name | `session_start` |
| `{{agent_id}}` | `agent_lifecycle` | UUID of the agent that fired the event | `550e8400-...` |
| `{{timestamp}}` | `agent_lifecycle`, `dispatch_result` | RFC 3339 timestamp of the event | `2026-04-01T09:00:00Z` |
| `{{source_workflow_id}}` | `dispatch_result` | UUID of the workflow that completed | `a1b2c3d4-...` |
| `{{dispatch_id}}` | `dispatch_result` | UUID of the dispatch record | `b2c3d4e5-...` |
| `{{status}}` | `dispatch_result` | Completion status | `completed` |

!!! note
    Schedule and event trigger variables are stored in the task's `metadata` map and resolved during template rendering. If a variable is referenced but not present for the trigger type (e.g. `{{fire_time}}` in a delay workflow), the placeholder is left as-is in the rendered prompt.

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

**GitHub Pull Requests:**
```yaml
source:
  type: github_pull_requests
  owner: myorg           # GitHub user or organization
  repo: myrepo           # Repository name
  labels: [needs-review] # Filter by labels (optional)
  state: open            # PR state filter (default: open)
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

**Webhook:**
```yaml
source:
  type: webhook
  secret: "my-hmac-secret"   # optional — omit to disable HMAC verification
```

See [Webhook Triggers](webhook-triggers.md) for endpoint details, HMAC verification, payload parsing, and GitHub setup.

**Manual:**
```yaml
source:
  type: manual
```

See [Manual Triggers](manual-trigger.md) for the `trigger-workflow` CLI command and the `POST /workflows/{id}/trigger` API endpoint.

!!! note "Event-driven triggers (API only)"
    The `agent_lifecycle` and `dispatch_result` trigger types are configured via the REST API only. They are not supported in `.agentd/` YAML templates. See [Event-Driven Triggers](event-triggers.md).

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
