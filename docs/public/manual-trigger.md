# Manual Triggers

A manual workflow fires only when explicitly told to — there is no polling, no schedule, and no event subscription. You dispatch tasks on demand via the API or CLI, and the agent runs immediately.

```
Operator / CI / script
────────────────────────
POST /workflows/{id}/trigger  ──▶  Scheduler.trigger_workflow()
{ "title": "...", "body": "..." }   Render prompt template
                                    Send to agent via WebSocket
```

Manual workflows are the simplest trigger type: create once, fire whenever you need it.

---

## When to use Manual

| Scenario | Recommended trigger |
|----------|---------------------|
| Testing a workflow before enabling its normal trigger | Manual (bypass any trigger type) |
| Ad-hoc one-off task — "run this now" | `manual` trigger type |
| Operator-initiated task from a script or CI job | `manual` trigger type or bypass |
| Recurring schedule | `cron` |
| Event-driven reaction | `agent_lifecycle` or `dispatch_result` |
| Low-latency external events | `webhook` |

---

## Configuration

### JSON (REST API)

```json
{
  "name": "on-demand-agent",
  "agent_id": "<AGENT_UUID>",
  "trigger_config": {
    "type": "manual"
  },
  "prompt_template": "{{title}}\n\n{{body}}",
  "enabled": true
}
```

The `manual` trigger config has no additional fields.

### CLI

```bash
agent orchestrator create-workflow \
  --name on-demand-agent \
  --agent-name worker \
  --trigger-type manual \
  --prompt-template "{{title}}\n\n{{body}}"
```

### YAML template (`.agentd/`)

```yaml
name: on-demand-agent
agent: worker

source:
  type: manual

prompt_template: |
  {{title}}

  {{body}}

enabled: true
```

---

## Trigger a Workflow

### CLI

```bash
# Minimal — uses default title "Manual trigger"
agent orchestrator trigger-workflow <WORKFLOW_ID>

# With title and body
agent orchestrator trigger-workflow <WORKFLOW_ID> \
  --title "Deploy hotfix" \
  --body "Review and merge PR #123 — customer-impacting regression"

# Output as JSON
agent orchestrator trigger-workflow <WORKFLOW_ID> \
  --title "Run audit" \
  --json
```

`trigger-workflow` works on **any** workflow type, not just `manual` — see [Bypass trigger semantics](#bypass-trigger-semantics).

### API

```http
POST /workflows/{id}/trigger
Content-Type: application/json
```

**Request body** (all fields optional):

```json
{
  "title": "Deploy hotfix",
  "body": "Review and merge PR #123",
  "metadata": {
    "pr_number": "123",
    "environment": "production"
  }
}
```

An empty body (`{}` or no body at all) is valid — defaults are applied.

**Field reference:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `title` | string | `"Manual trigger"` | Task title, rendered as `{{title}}` in the prompt template |
| `body` | string | `""` (empty) | Task body, rendered as `{{body}}` |
| `metadata` | object | `{}` | Arbitrary key-value pairs; each key is available as `{{key}}` in the template |

**Response** (`200 OK`):

```json
{
  "id": "b2c3d4e5-...",
  "workflow_id": "a1b2c3d4-...",
  "source_id": "manual:f47ac10b-...",
  "agent_id": "550e8400-...",
  "prompt_sent": "Deploy hotfix\n\nReview and merge PR #123",
  "status": "pending",
  "dispatched_at": "2026-03-17T12:00:00Z",
  "completed_at": null
}
```

**Response codes:**

| Code | Meaning |
|------|---------|
| `200 OK` | Task queued or dispatched successfully |
| `400 Bad Request` | Workflow is disabled — enable it first |
| `404 Not Found` | Workflow does not exist |
| `409 Conflict` | Agent is currently busy processing another task |
| `503 Service Unavailable` | Agent is not connected |

### Inspect the result

```bash
# Check dispatch status immediately after triggering
agent orchestrator workflow-history <WORKFLOW_ID>

# Stream dispatch history in real time
watch -n 5 'agent orchestrator workflow-history <WORKFLOW_ID>'
```

---

## Bypass Trigger Semantics

`POST /workflows/{id}/trigger` works on **any** workflow type, not just `manual`-type workflows. This is called a *bypass trigger* — it dispatches a task immediately, bypassing whatever the workflow's normal trigger strategy is.

**Manual-type workflow** (runner path):

1. Task pushed to the workflow's internal `mpsc` channel
2. The running `WorkflowRunner` dequeues it and dispatches normally
3. Dispatch record created with `status: "pending"` before dispatch
4. Busy-state tracking applies — returns `409` if agent is already busy

**Any other trigger type** (direct path):

1. Task rendered and sent directly to the agent, bypassing the strategy
2. Dispatch record created with `status: "dispatched"`
3. Agent must be connected — returns `503` if not

The result looks identical to the caller. The difference is internal: Manual workflows use the runner's queue (ordered, tracked), while bypass dispatches go straight to the agent.

!!! tip "Testing a cron workflow before the schedule fires"
    Create your cron workflow with `enabled: true`, then immediately trigger it with:
    ```bash
    agent orchestrator trigger-workflow <WORKFLOW_ID> \
      --title "Test run" \
      --body "Validate before the real cron fires"
    ```
    The agent runs immediately, and dispatch history shows the result alongside future cron-triggered dispatches.

---

## Template Variables

Manual trigger tasks support all standard task template variables:

| Variable | Value | Notes |
|----------|-------|-------|
| `{{title}}` | From request `title` field | Defaults to `"Manual trigger"` |
| `{{body}}` | From request `body` field | Defaults to empty string |
| `{{source_id}}` | `manual:<uuid>` | Random UUID, unique per trigger call |
| `{{url}}` | `""` | Always empty for manual triggers |
| `{{labels}}` | `""` | Always empty for manual triggers |
| `{{assignee}}` | `""` | Always empty for manual triggers |
| `{{metadata}}` | All metadata as `key: value` lines | From request `metadata` object |
| `{{<key>}}` | Metadata value for `<key>` | Any key from the `metadata` object |

**Example — using metadata in a prompt template:**

```yaml
prompt_template: |
  Environment: {{environment}}
  PR: #{{pr_number}}

  {{body}}
```

Triggered with:

```bash
curl -X POST http://127.0.0.1:17006/workflows/<ID>/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Deploy review",
    "body": "Run the deployment checklist.",
    "metadata": { "environment": "staging", "pr_number": "42" }
  }'
```

---

## Usage Patterns

### Pattern 1 — On-demand task dispatch

Create a permanent workflow that you fire manually whenever needed:

```bash
# Create the workflow once
agent orchestrator create-workflow \
  --name release-checklist \
  --agent-name release-bot \
  --trigger-type manual \
  --prompt-template "$(cat <<'TMPL'
Run the release checklist for version {{title}}.

Steps:
1. Run all tests: cargo test --workspace
2. Check for uncommitted changes
3. Update CHANGELOG.md
4. Tag the release: git tag {{title}}
5. Push: git push --tags
TMPL
)"

# Fire it whenever you cut a release
agent orchestrator trigger-workflow <WORKFLOW_ID> --title "v1.4.2"
```

### Pattern 2 — CI/CD integration

Trigger an agent from a CI pipeline:

```bash
#!/usr/bin/env bash
# .github/workflows/deploy.yml step

WORKFLOW_ID="a1b2c3d4-..."  # stored as a CI secret
PR_NUMBER="${{ github.event.pull_request.number }}"
BRANCH="${{ github.head_ref }}"

curl -s -X POST \
  "http://agentd.internal:17006/workflows/${WORKFLOW_ID}/trigger" \
  -H "Content-Type: application/json" \
  -d "{
    \"title\": \"Review PR #${PR_NUMBER}\",
    \"body\": \"Branch: ${BRANCH}\",
    \"metadata\": {
      \"pr_number\": \"${PR_NUMBER}\",
      \"branch\": \"${BRANCH}\"
    }
  }"
```

### Pattern 3 — Scripted batch dispatch

Run the same workflow against a list of inputs:

```bash
WORKFLOW_ID="a1b2c3d4-..."
REPOS=("myorg/alpha" "myorg/beta" "myorg/gamma")

for repo in "${REPOS[@]}"; do
  echo "Triggering audit for $repo..."
  agent orchestrator trigger-workflow "$WORKFLOW_ID" \
    --title "Audit $repo" \
    --body "Run dependency and security audit"
  # Wait for the agent to finish before triggering the next
  sleep 30
done
```

### Pattern 4 — Combining manual and another trigger type

A `manual` workflow and a `cron` workflow can target the same agent. The manual workflow gives you an override path while the cron workflow runs on its normal schedule:

```bash
# Cron workflow — runs at 9 AM on weekdays
agent orchestrator create-workflow \
  --name daily-standup-cron \
  --agent-name standup-bot \
  --trigger-type cron \
  --cron-expression "0 9 * * MON-FRI" \
  --prompt-template "Run the daily standup summary for {{fire_time}}"

# Manual workflow — for immediate on-demand run
agent orchestrator create-workflow \
  --name daily-standup-manual \
  --agent-name standup-bot \
  --trigger-type manual \
  --prompt-template "Run the standup summary now: {{body}}"

# Fire the manual one-off immediately
agent orchestrator trigger-workflow <MANUAL_WORKFLOW_ID> \
  --title "Emergency standup" \
  --body "Triggered by incident response team"
```

!!! note
    Two workflows targeting the same agent will queue behind each other — the agent processes one task at a time. If you trigger a manual workflow while the agent is busy, you get `409 Conflict`.

---

## Operational Notes

### `source_id` format and deduplication

Each manual trigger generates a unique `source_id`:

```
manual:<uuid>
```

The UUID is random and generated at trigger time. Manual triggers are never deduplicated — each `POST /workflows/{id}/trigger` call always produces a new dispatch record. This differs from polling triggers (which track seen issue numbers) and delay triggers (which fire only once).

### Agent busy

The endpoint returns `409 Conflict` if the agent is currently processing another task:

```
Agent is currently busy processing another task.
```

Options:
- Wait and retry after the current task completes
- Check dispatch history for estimated completion: `agent orchestrator workflow-history <WORKFLOW_ID>`
- Use a separate agent for each workflow to allow concurrent tasks

### Agent disconnected

The endpoint returns `503 Service Unavailable` if the agent's WebSocket session is not active:

```
Agent <id> is not connected.
```

The agent must be running and connected before you can trigger a dispatch. Check agent status:

```bash
agent orchestrator get-agent <AGENT_ID>
```

### Workflow disabled

If the workflow has `enabled: false`, the endpoint returns `400 Bad Request`:

```
Workflow <id> is not enabled.
```

Re-enable with:

```bash
agent orchestrator update-workflow <WORKFLOW_ID> --enabled true
```

### Dispatch status lifecycle

| Status | When set |
|--------|----------|
| `pending` | Task queued in the ManualStrategy channel (Manual-type workflows) |
| `dispatched` | Prompt sent to the agent (bypass dispatch or after ManualStrategy dequeues) |
| `completed` | Agent returned a result message |
| `failed` | Agent returned an error, or send failed |

### Log patterns

```
INFO  orchestrator::scheduler::api  workflow_id=... source_id=manual:... title="Deploy hotfix" "Manual workflow trigger requested"
INFO  orchestrator::scheduler::mod  workflow_id=... source_id=manual:... "Manual trigger queued via channel"
INFO  orchestrator::scheduler::mod  workflow_id=... source_id=manual:... "Manual trigger dispatched directly to agent"
```
