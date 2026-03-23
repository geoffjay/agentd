# Agent Contributor Onboarding

This guide helps you understand the agent ecosystem and contribute to it. By the end you'll
know how to deploy the existing agent stack, create new agents and workflows, navigate the
label-driven pipeline, and use git-spice for stacked branch management.

## Prerequisites

- Services running locally (see [Getting Started](../getting-started.md))
- `gh` CLI authenticated: `gh auth login`
- git-spice installed: `brew install abhinav/tap/git-spice`
- git-spice initialized in your clone (one-time, human step — see [git-spice setup](#one-time-setup))

---

## 1. Quick start — deploy the agent stack

The project ships a complete agent stack in `.agentd/`. One command creates every room,
agent, and workflow in the correct order:

```bash
agent apply .agentd/
```

Apply order: **rooms → agents → workflows**. Rooms must exist before agents join them;
agents must be running before workflows reference them by name.

Preview what would be created without making changes:

```bash
agent apply --dry-run .agentd/
```

Tear everything down in reverse order:

```bash
agent teardown .agentd/
```

### What gets created

```
.agentd/
├── rooms/
│   ├── engineering.yml     # General coordination channel
│   ├── announcements.yml   # Broadcast announcements
│   ├── operations.yml      # Pipeline status digests (conductor)
│   └── security.yml        # Audit findings (security agent)
├── agents/
│   ├── conductor.yml       # Pipeline orchestrator
│   ├── planner.yml         # Issue breakdown and planning
│   ├── worker.yml          # Issue implementation
│   ├── reviewer.yml        # PR code review
│   ├── enricher.yml        # Issue enrichment
│   ├── tester.yml          # Test coverage
│   ├── security.yml        # Security auditing
│   └── architect.yml       # Architecture review
└── workflows/
    ├── issue-worker.yml                   # Dispatches on work-agent label
    ├── plan-worker.yml                    # Dispatches on plan-agent label
    ├── pull-request-reviewer.yml          # Polls PRs with review-agent label
    ├── webhook-pull-request-reviewer.yml  # Webhook-triggered PR review
    ├── docs-worker.yml
    ├── research-worker.yml
    ├── enrichment-worker.yml
    ├── test-worker.yml
    └── security-worker.yml
```

!!! note "Idempotent"
    If a room or agent with the same name already exists, `agent apply` reuses it rather
    than creating a duplicate. Re-running `agent apply .agentd/` is safe.

---

## 2. Creating a new agent

Agent definitions live in `.agentd/agents/<name>.yml`.

!!! warning "Human approval required"
    Changes to `.agentd/agents/*.yml` are gated — they alter agent behavior for all future
    runs. A human must review and merge these changes. Do not modify existing agent YAML
    files autonomously.

### Minimal agent

```yaml
name: my-agent
working_dir: "."
system_prompt: |
  You are a specialist agent for the agentd project.
  Describe your role and responsibilities here.
```

### Full field reference

```yaml
# Required
name: my-agent                 # Unique agent name

# Optional — sensible defaults shown
working_dir: "."               # Resolved relative to YAML file at apply time
shell: /bin/zsh                # Shell for the tmux session
worktree: false                # Isolated git worktree per task
model: claude-sonnet-4-6       # Claude model (omit to use default)

# Optional — directories outside working_dir the agent can access
additional_dirs:
  - ../other-repo              # Relative to YAML file
  - /opt/shared/configs        # Absolute path

# Optional — initial prompt sent once after the agent connects
prompt: "Read CLAUDE.md and summarize this project."

# Optional — system prompt for the Claude session
system_prompt: |
  You are a ...

# Optional — tool restrictions (default: allow_all)
tool_policy:
  mode: deny_list
  tools:
    - "Write(crates/**)"       # Block writes to production source
    - "Edit(.agentd/**)"       # Block modification of agent configs

# Optional — rooms the agent auto-joins at startup
rooms:
  - engineering                # Plain name = member role
  - name: announcements
    role: observer             # Read-only
  - name: operations
    role: admin                # Can manage participants
```

### System prompt best practices

The system prompt defines what the agent is and how it behaves. Look at the existing agents
in `.agentd/agents/` for patterns. Key things to include:

1. **Role statement** — one sentence describing what the agent does and for which project
2. **Process section** — numbered steps the agent follows for each task
3. **Scope constraints** — what the agent must NOT do (as important as what it should do)
4. **Branch/PR workflow** — how to use git-spice (copy the pattern from `worker.yml`)
5. **Memory Protocol** — how the agent reads and writes shared memory (see below)

### Memory Protocol

Every agent in the project follows the same memory pattern. Include this block in the
system prompt, substituting your agent's identity for `<name>`:

```markdown
## Memory Protocol

You have access to a shared memory service via the `agent memory` CLI.
Your agent identity for memory operations is "<name>".

### On Startup
Before beginning any task, retrieve relevant context:
1. Search for prior decisions in your area:
   agent memory search "<topic>" --as-actor <name> --limit 5 --json
2. Search for memories tagged with your role:
   agent memory search "<name> guidance" --as-actor <name> --tags <name> --limit 5 --json
3. Review results and incorporate relevant context.

### After Completing Work
Store significant findings, decisions, or gotchas:
  agent memory remember "<what you learned>" \
    --created-by <name> --type information \
    --tags <name>,<topic> --visibility public

### What to Remember
- Decisions and their rationale
- Patterns that worked or failed
- Gotchas not captured in code or docs

### What NOT to Remember
- Information already in code, comments, or git history
- Ephemeral task state
```

### Tool policies

Use tool policies to limit what an agent can do. The tester and security agents
show two common patterns:

=== "Allow list (safest)"
    ```yaml
    # Only these tools are permitted — everything else is denied
    tool_policy:
      mode: allow_list
      tools:
        - Read
        - Bash
        - Grep
        - Glob
        - "Write(crates/*/tests/**)"   # Path-scoped write
        - "Edit(crates/*/tests/**)"
    ```

=== "Deny list (most flexible)"
    ```yaml
    # All tools permitted except these
    tool_policy:
      mode: deny_list
      tools:
        - "Write(crates/**)"      # Block production source writes
        - "Edit(crates/**)"
        - "Write(.agentd/**)"     # Block agent config modification
        - "Edit(.agentd/**)"
    ```

=== "Require approval"
    ```yaml
    # Every tool use requires human approval (5-minute timeout)
    tool_policy:
      mode: require_approval
    ```

See [Tool Policies](../tool-policies.md) for the full reference.

---

## 3. Creating a new workflow

Workflows connect an agent to a task source. They live in `.agentd/workflows/<name>.yml`.

### Minimal workflow

```yaml
name: my-workflow
agent: my-agent               # Agent name (resolved to UUID at apply time)

source:
  type: github_issues
  owner: geoffjay
  repo: agentd
  labels:
    - my-label                # Issues with this label are dispatched
  state: open

poll_interval: 60             # Seconds between polls
enabled: true

prompt_template: |
  You have been assigned a task.

  Issue #{{source_id}}: {{title}}
  URL: {{url}}
  Labels: {{labels}}

  Description:
  {{body}}
```

### Supported trigger types

=== "GitHub Issues"
    ```yaml
    source:
      type: github_issues
      owner: myorg
      repo: myrepo
      labels: [my-label]     # Filter by one or more labels
      state: open
    ```

=== "GitHub Pull Requests"
    ```yaml
    source:
      type: github_pull_requests
      owner: myorg
      repo: myrepo
      labels: [review-agent]
      state: open
    ```

=== "Cron (recurring)"
    ```yaml
    source:
      type: cron
      expression: "0 9 * * MON-FRI"  # 9 AM UTC on weekdays
    ```

=== "Webhook"
    ```yaml
    source:
      type: webhook
      secret: "my-hmac-secret"   # Optional HMAC verification
    ```

=== "Manual"
    ```yaml
    source:
      type: manual
    # Trigger with: agent orchestrator trigger-workflow <id>
    ```

### Prompt template variables

All `{{variable}}` placeholders available in GitHub issue/PR workflows:

| Variable | Description | Example |
|----------|-------------|---------|
| `{{source_id}}` | Issue or PR number | `42` |
| `{{title}}` | Issue/PR title | `"Fix login bug"` |
| `{{body}}` | Full issue/PR body (markdown) | `"## Overview\n..."` |
| `{{url}}` | GitHub URL | `"https://github.com/org/repo/issues/42"` |
| `{{labels}}` | Comma-separated labels | `"bug, agent"` |
| `{{assignee}}` | Assigned user (empty if none) | `"alice"` |

For cron workflows: `{{fire_time}}`, `{{cron_expression}}`. For delay: `{{run_at}}`.

Validate a template before deploying:

```bash
agent orchestrator validate-template "Fix #{{source_id}}: {{title}}\n{{body}}"
```

### Writing effective prompt templates

The prompt template is the entire instruction set the agent receives per task. It should:

1. **Repeat key context** — include issue number, URL, and body so the agent doesn't need
   to re-fetch them
2. **Include a memory step** — have the agent search memory before starting work (see the
   `issue-worker.yml` pattern)
3. **Give explicit instructions** — numbered steps, not vague guidance
4. **Specify the branch naming** — tell the agent exactly how to create the branch
5. **Specify the completion step** — how to remove the dispatch label when done

Look at `.agentd/workflows/issue-worker.yml` and `.agentd/workflows/research-worker.yml`
for complete, production-tested examples.

---

## 4. Label conventions

The pipeline is driven entirely by GitHub labels. Understanding which labels do what is
essential before contributing.

### Pipeline state labels (blue)

These encode the current state of an issue or PR. The conductor agent reads and sets them.

| Label | Set by | Meaning |
|-------|--------|---------|
| `needs-triage` | Human / automation | New issue awaiting triage |
| `triaged` | Planner agent | Issue scoped and ready |
| `merge-ready` | Reviewer agent | PR approved + CI green |
| `merge-queue` | Conductor | Actively queued for merge |

### Status / warning labels (amber + red)

| Label | Set by | Meaning |
|-------|--------|---------|
| `needs-rework` | Reviewer | PR has change requests |
| `needs-restack` | Reviewer / conductor | Branch is behind parent; run `git-spice upstack restack` |
| `needs-refinement` | Human / planner | Issue lacks enough detail |
| `needs-tests` | Reviewer / tester | Missing test coverage |

### Agent dispatch labels (green + specialist)

Applying one of these labels triggers the matching workflow to pick up the issue and
dispatch it to the corresponding agent. **The agent removes the label when it finishes.**

| Label | Workflow | Agent dispatched | Target |
|-------|----------|-----------------|--------|
| `work-agent` | `issue-worker` | worker | Issue |
| `docs-agent` | `docs-worker` | documenter | Issue |
| `plan-agent` | `plan-worker` | planner | Issue |
| `enrich-agent` | `enrichment-worker` | enricher | Issue |
| `test-agent` | `test-worker` | tester | Issue |
| `refactor-agent` | `refactor-worker` | refactor agent | Issue |
| `research-agent` | `research-worker` | research agent | Issue |
| `security-agent` | `security-worker` | security agent | Issue |
| `review-agent` | `pull-request-reviewer` | reviewer | Pull request (polling) |
| `review-agent` | `webhook-pull-request-reviewer` | reviewer | Pull request (webhook) |
| `conductor-sync` | conductor sync | conductor | Manual trigger |

!!! tip "Re-triggering an agent"
    Dispatch is deduplicated — re-applying a label to an already-dispatched issue has no
    effect. To re-trigger, remove the label first, then re-add it. This creates a new event
    that the scheduler treats as a fresh task.

### Adding a new dispatch label

When adding a new agent to the pipeline, you need:

1. A new entry in `.github/labels.yml`
2. A new agent in `.agentd/agents/<name>.yml`
3. A new workflow in `.agentd/workflows/<name>-worker.yml` referencing that label
4. An entry in the dispatch table in `docs/public/pipeline-state-machine.md`

---

## 5. git-spice for agents

All branch management in this project uses git-spice. Raw `git checkout -b` and
`gh pr create` are not used — use the git-spice equivalents.

### One-time setup

```bash
# Install (macOS)
brew install abhinav/tap/git-spice

# Initialize in your clone (sets trunk = main)
git-spice repo init --trunk main
```

!!! warning "Human-only: `git-spice auth login`"
    The OAuth login flow is interactive and cannot be scripted. A human must run this
    once per clone before any `git-spice branch submit` operations:
    ```bash
    git-spice auth login
    ```

Restore project config defaults if they are missing after a fresh clone:

```bash
git config spice.submit.navigationComment true
git config spice.submit.navigationComment.downstack true
git config spice.log.all true
git config spice.log.crStatus true
```

### Session start

Always sync at the start of every working session to pick up merged PRs and
rebase any dependent branches:

```bash
git-spice repo sync
```

### Core workflow

```bash
# 1. Start from the milestone base branch
git checkout feature/autonomous-pipeline

# 2. Create a stacked branch
git-spice branch create issue-NNN -m "feat: brief description"
#   Naming conventions:
#   Implementation:  issue-NNN
#   Documentation:   docs/issue-NNN
#   Tests:           test/issue-NNN
#   Refactoring:     refactor/issue-NNN

# 3. Make changes, then commit
git-spice commit create -m "feat(scope): description (closes #NNN)"

# 4. Submit as a PR — idempotent (creates or updates)
git-spice branch submit --fill --no-prompt --label review-agent
```

### Handling dependencies (stacking)

When issue B depends on issue A being merged first, stack B's branch on top of A's:

```bash
# Check out A's branch, then create B on top of it
git checkout issue-A
git-spice branch create issue-B -m "feat: description"
```

The issue body should declare this with a `## Blocked By` section:

```markdown
## Blocked By
- #A — description of why B needs A first

## Stack Base
Branch off `feature/autonomous-pipeline`. PR back into it.
```

### Addressing review feedback

When a PR receives `needs-rework`:

```bash
# Make fixes, then amend the commit
git-spice commit amend -m "feat(scope): description (address review feedback)"

# Resubmit — updates the existing PR
git-spice branch submit --fill --no-prompt --label review-agent
```

### Merge conflict / needs-restack

When a PR receives `needs-restack`, the branch has fallen behind its parent:

```bash
git-spice repo sync                  # Sync parent branches first
git-spice upstack restack            # Rebase this branch on its parent
git-spice branch submit --fill --no-prompt --label review-agent
```

### Useful inspection commands

```bash
# See the full branch stack and PR status
git-spice log short

# Detailed stack with parent/child relationships
git-spice log long

# Check spice data is intact
git log --oneline refs/spice/data | head -5
```

### Non-interactive flags

Agents must never use interactive git-spice commands. Always pass these flags:

| Command | Non-interactive form |
|---------|---------------------|
| `git-spice branch submit` | `git-spice branch submit --fill --no-prompt` |
| `git-spice stack submit` | `git-spice stack submit --fill --no-prompt` |

`--fill` populates the PR title/body from commit messages. `--no-prompt` suppresses
all interactive confirmation prompts.

---

## 6. Room configuration

Rooms are communication channels that agents and humans share. They are defined in
`.agentd/rooms/<name>.yml` and created as part of `agent apply .agentd/`.

### Room template

```yaml
name: engineering
topic: "Engineering team coordination"
description: "General channel for engineering agents and humans."
type: group          # direct | group | broadcast

participants:
  - identifier: conductor
    kind: agent
    role: member
  - identifier: geoff
    kind: human
    role: admin
    display_name: "Geoff"
```

### Room types

| Type | Who can post | Use case |
|------|-------------|----------|
| `group` | All members | Agent/human coordination |
| `direct` | Both participants | One-to-one |
| `broadcast` | Admins only | Announcements, status feeds |

### Participant roles

| Role | Post messages | Manage participants |
|------|--------------|-------------------|
| `member` | Yes | No |
| `admin` | Yes | Yes |
| `observer` | No | No |

### Existing rooms

| Room | Type | Purpose |
|------|------|---------|
| `engineering` | group | General agent/human coordination |
| `announcements` | group | Project-wide announcements |
| `operations` | group | Pipeline status (conductor posts digests here) |
| `security` | group | Audit findings from the security agent |

### Adding an agent to a room

**In the agent YAML** (preferred — the agent joins automatically at startup):

```yaml
# .agentd/agents/my-agent.yml
rooms:
  - engineering                # member role
  - name: announcements
    role: observer
```

**Via CLI** (for a running agent, without redeploying):

```bash
agent communicate add-participant \
  --room engineering \
  --identifier my-agent \
  --kind agent \
  --role member
```

**Adding a room to the project** requires:

1. A new file `.agentd/rooms/<name>.yml`
2. Adding each agent that should join to their `rooms:` list
3. Running `agent apply .agentd/rooms/<name>.yml` (or `agent apply .agentd/`)

!!! note "Idempotent creation"
    If a room already exists, `agent apply` skips creation — it does not update the
    topic, description, or participants. To modify an existing room, use the CLI directly.

---

## 7. Testing agents before deploying

### Validate templates without creating anything

```bash
agent apply --dry-run .agentd/
agent apply --dry-run .agentd/agents/my-agent.yml
agent apply --dry-run .agentd/workflows/my-workflow.yml
```

### Validate prompt templates

```bash
agent orchestrator validate-template "Fix #{{source_id}}: {{title}}\n{{body}}"
agent orchestrator validate-template --file .agentd/workflows/my-workflow.yml
```

### Deploy a single agent for testing

```bash
# Create just the agent (not its workflow)
agent apply .agentd/agents/my-agent.yml

# Check it started
agent orchestrator list-agents --status running
```

### Send a one-off task

```bash
# Find the agent ID
AGENT_ID=$(agent orchestrator list-agents --json | jq -r '.[] | select(.name=="my-agent") | .id')

# Send a test prompt directly (bypasses the workflow)
agent orchestrator send-message "$AGENT_ID" "This is a test task. Describe what you would do for issue #1."
```

### Watch the agent work in real time

```bash
# Stream structured output (tool calls, text, permission requests)
agent orchestrator stream "$AGENT_ID"

# Attach to the raw tmux session for full terminal view
agent orchestrator attach --name my-agent
# Detach without killing: Ctrl-b d
```

### Approve tool requests interactively

For agents with `require_approval` policy, approve tool calls as they come in:

```bash
# In one terminal — watch for permission requests
agent orchestrator stream "$AGENT_ID"

# In another — approve or deny
agent orchestrator list-approvals
agent orchestrator approve <APPROVAL_ID>
agent orchestrator deny <APPROVAL_ID>
```

### Test a workflow dispatch manually

Trigger a manual-type workflow without waiting for a label event:

```bash
# Find the workflow ID
WORKFLOW_ID=$(agent orchestrator list-workflows --json | jq -r '.[] | select(.name=="my-workflow") | .id')

# Trigger it
agent orchestrator trigger-workflow "$WORKFLOW_ID"
```

For label-driven workflows, the quickest way to test end-to-end is to apply the
`research-agent` (or other) label to a real issue and watch the stream:

```bash
gh issue edit <number> --repo geoffjay/agentd --add-label "research-agent"
agent orchestrator stream --all
```

### Tear down after testing

```bash
agent teardown .agentd/agents/my-agent.yml
# or remove everything:
agent teardown .agentd/
```

---

## Human approval gates

These operations must never be performed by an agent without explicit human instruction.
Full reference: `docs/planning/autonomous-pipeline-gates.md`.

| Operation | Why |
|-----------|-----|
| `git-spice auth login` | Interactive OAuth — cannot be scripted |
| Modifying `.agentd/agents/*.yml` | Changes agent behavior for all future runs |
| Modifying `crates/orchestrator/src/` core | Risk of breaking the pipeline |
| `git push --force` to trunk branches | Destructive, irreversible |
| Deleting branches, issues, or milestones | Destructive |
| Production deployments | Human sign-off required |
| Adding new external service integrations | Architectural decision |

---

## Related

- [Pipeline State Machine](../pipeline-state-machine.md) — full label reference and lifecycle
- [Research Agent](research.md) — example of a specialist agent
- [Templates Reference](../templates.md) — complete YAML schema with all fields
- [Tool Policies](../tool-policies.md) — tool restriction modes and approval flow
- [Communication Guide](../communication-guide.md) — rooms, messages, and participants
- `CLAUDE.md` — authoritative project conventions for all contributors
