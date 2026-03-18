# Workflow Trigger Reference

This page is a consolidated quick-reference for all workflow trigger types. For full documentation on each type, follow the links in each section.

---

## Trigger Type Comparison

| Trigger | When it fires | Repeats? | Requires public endpoint? | CLI support? | YAML support? |
|---------|--------------|----------|--------------------------|-------------|--------------|
| `github_issues` | New or updated GitHub issues matching filters | Yes — polls continuously | No | Yes | Yes |
| `github_pull_requests` | New or updated GitHub PRs matching filters | Yes — polls continuously | No | Yes | Yes |
| `cron` | On a recurring cron schedule | Yes — indefinitely | No | Yes | Yes |
| `delay` | Once at a specific datetime | No — auto-disables | No | Yes | Yes |
| `agent_lifecycle` | When the agent connects, disconnects, or clears context | Yes | No | No — API only | No — API only |
| `dispatch_result` | When another workflow dispatch completes | Yes | No | No — API only | No — API only |
| `webhook` | On inbound HTTP POST to a dedicated endpoint | Yes | Yes (or tunnel) | Yes | Yes |
| `manual` | Only when explicitly triggered via CLI or API | On demand | No | Yes | Yes |

---

## When to Use Each Trigger

### `github_issues` / `github_pull_requests`

Use when:

- You want an agent to react to GitHub issues or PRs automatically
- You are behind a firewall or don't want to expose a public endpoint
- Latency of up to `poll_interval_secs` is acceptable (default: 60 s)

Do not use when:

- You need sub-second latency (use `webhook` instead)
- The trigger source is not GitHub

### `cron`

Use when:

- You need a recurring scheduled task (daily report, hourly health check, weekly audit)
- The schedule is fixed and known in advance

Do not use when:

- The task should run exactly once (use `delay` instead)
- The task is triggered by an external event

### `delay`

Use when:

- You need a one-shot task at a specific future time (scheduled deployment, maintenance window)
- You want a "run this exactly once at time X" guarantee

Do not use when:

- You need a recurring schedule (use `cron` instead)

### `agent_lifecycle`

Use when:

- You need to bootstrap an agent every time it connects (`session_start`)
- You need to clean up or archive state when an agent disconnects (`session_end`)
- You need to re-inject context after a `/clear` command (`context_clear`)

### `dispatch_result`

Use when:

- You want to chain workflows into a pipeline (lint → test → deploy)
- A downstream step should only start after an upstream step completes

Do not use when:

- You want parallel execution (each workflow must have its own trigger)

### `webhook`

Use when:

- You need near-real-time reaction to external events (GitHub, CI, monitoring)
- You can expose a public HTTPS endpoint (or use a tunnel like ngrok)

Do not use when:

- You are behind a firewall with no public endpoint (use polling instead)

### `manual`

Use when:

- You want an on-demand workflow that fires only when you tell it to
- You need an override or emergency path for any workflow type
- You are integrating with a CI pipeline (`curl POST /workflows/{id}/trigger`)

---

## Configuration Quick Reference

### JSON (REST API)

All trigger types use the `trigger_config` field with a `"type"` discriminant:

```json
{ "type": "github_issues",        "owner": "myorg", "repo": "myrepo", "labels": ["agent"], "state": "open" }
{ "type": "github_pull_requests", "owner": "myorg", "repo": "myrepo", "labels": [],        "state": "open" }
{ "type": "cron",                 "expression": "0 9 * * MON-FRI" }
{ "type": "delay",                "run_at": "2026-04-01T09:00:00Z" }
{ "type": "agent_lifecycle",      "event": "session_start" }
{ "type": "dispatch_result",      "source_workflow_id": "<UUID>", "status": "completed" }
{ "type": "webhook",              "secret": "my-hmac-secret" }
{ "type": "manual" }
```

### YAML (`.agentd/` templates)

```yaml
# GitHub Issues
source:
  type: github_issues
  owner: myorg
  repo: myrepo
  labels: [agent]
  state: open

# GitHub Pull Requests
source:
  type: github_pull_requests
  owner: myorg
  repo: myrepo
  state: open

# Cron
source:
  type: cron
  expression: "0 9 * * MON-FRI"

# Delay (one-shot)
source:
  type: delay
  run_at: "2026-04-01T09:00:00Z"

# Webhook
source:
  type: webhook
  secret: "my-hmac-secret"   # optional

# Manual
source:
  type: manual
```

!!! note
    `agent_lifecycle` and `dispatch_result` are not available in YAML templates. Create them via the REST API.

### CLI (`create-workflow`)

```bash
# GitHub Issues (default trigger type)
agent orchestrator create-workflow --name wf --agent-name agent \
  --owner myorg --repo myrepo --labels "bug,agent"

# GitHub Pull Requests
agent orchestrator create-workflow --name wf --agent-name agent \
  --trigger-type github-pull-requests \
  --owner myorg --repo myrepo

# Cron
agent orchestrator create-workflow --name wf --agent-name agent \
  --trigger-type cron \
  --cron-expression "0 9 * * MON-FRI" \
  --prompt-template "Daily task at {{fire_time}}"

# Delay
agent orchestrator create-workflow --name wf --agent-name agent \
  --trigger-type delay \
  --run-at "2026-04-01T09:00:00Z" \
  --prompt-template "Scheduled task: {{run_at}}"

# Webhook
agent orchestrator create-workflow --name wf --agent-name agent \
  --trigger-type webhook \
  --webhook-secret "$(openssl rand -hex 32)" \
  --prompt-template "Webhook: {{title}}\n{{body}}"

# Manual
agent orchestrator create-workflow --name wf --agent-name agent \
  --trigger-type manual \
  --prompt-template "{{title}}\n\n{{body}}"
```

---

## Template Variables by Trigger Type

### Universal variables (all trigger types)

| Variable | Description |
|----------|-------------|
| `{{title}}` | Task title |
| `{{body}}` | Task body / description |
| `{{url}}` | Source URL (empty for non-GitHub triggers) |
| `{{labels}}` | Comma-separated label names (empty for non-GitHub triggers) |
| `{{assignee}}` | Assignee login (empty for non-GitHub triggers) |
| `{{source_id}}` | Deduplication identifier (format varies by trigger type) |
| `{{metadata}}` | All metadata as `key: value` lines |

### `github_issues` / `github_pull_requests`

| Variable | Description |
|----------|-------------|
| `{{title}}` | Issue or PR title |
| `{{body}}` | Issue or PR body (markdown) |
| `{{url}}` | GitHub HTML URL |
| `{{labels}}` | Comma-separated label names |
| `{{assignee}}` | Assignee login |
| `{{source_id}}` | Issue or PR number (string) |

### `cron`

| Variable | Description | Example |
|----------|-------------|---------|
| `{{fire_time}}` | RFC 3339 timestamp when the cron fired | `2026-04-01T09:00:00Z` |
| `{{cron_expression}}` | The expression that fired | `0 9 * * MON-FRI` |
| `{{source_id}}` | `cron:<fire_time>` | `cron:2026-04-01T09:00:00Z` |

### `delay`

| Variable | Description | Example |
|----------|-------------|---------|
| `{{run_at}}` | RFC 3339 scheduled datetime | `2026-04-01T09:00:00Z` |
| `{{workflow_id}}` | UUID of the workflow | `550e8400-...` |
| `{{source_id}}` | `delay:<workflow_id>` | `delay:550e8400-...` |

### `agent_lifecycle`

| Variable | Description | Example |
|----------|-------------|---------|
| `{{event_type}}` | Lifecycle event name | `session_start` |
| `{{agent_id}}` | UUID of the agent | `550e8400-...` |
| `{{timestamp}}` | RFC 3339 event timestamp | `2026-04-01T09:00:00Z` |
| `{{source_id}}` | `event:<event_type>:<agent_id>:<timestamp>` | |

### `dispatch_result`

| Variable | Description | Example |
|----------|-------------|---------|
| `{{source_workflow_id}}` | UUID of the workflow that completed | `a1b2c3d4-...` |
| `{{dispatch_id}}` | UUID of the dispatch record | `b2c3d4e5-...` |
| `{{status}}` | Completion status | `completed` |
| `{{timestamp}}` | RFC 3339 completion timestamp | `2026-04-01T09:05:00Z` |
| `{{source_id}}` | `event:dispatch:<dispatch_id>:<timestamp>` | |

### `webhook`

| Variable | Description | Example |
|----------|-------------|---------|
| `{{title}}` | Parsed title or `"Webhook payload"` | `"Fix login bug"` |
| `{{body}}` | Issue/PR body or raw payload | |
| `{{url}}` | GitHub HTML URL (empty for generic payloads) | |
| `{{delivery_id}}` | From `X-GitHub-Delivery` or auto-generated UUID | `abc-123` |
| `{{timestamp}}` | RFC 3339 receive time | `2026-04-01T09:00:00Z` |
| `{{github_event}}` | GitHub event type (GitHub requests only) | `issues` |
| `{{action}}` | GitHub action (issues/pull_request only) | `opened` |
| `{{issue_number}}` | Issue number (issues events only) | `42` |
| `{{pr_number}}` | PR number (pull_request events only) | `99` |
| `{{source_id}}` | `webhook:<delivery_id>:<timestamp>` | |

### `manual`

| Variable | Description |
|----------|-------------|
| `{{title}}` | From request `title` field (default: `"Manual trigger"`) |
| `{{body}}` | From request `body` field (default: empty) |
| `{{source_id}}` | `manual:<uuid>` — random per trigger call |
| `{{<key>}}` | Any key from the request `metadata` object |

---

## Performance Considerations

| Aspect | Polling (`github_issues`, `github_pull_requests`) | Schedule (`cron`, `delay`) | Event (`agent_lifecycle`, `dispatch_result`) | Webhook | Manual |
|--------|--------------------------------------------------|---------------------------|----------------------------------------------|---------|--------|
| Latency | Up to `poll_interval_secs` (default 60 s) | Zero — wakes exactly at fire time | Near-zero — in-process event bus | Sub-second | Immediate |
| External API calls | Yes — GitHub API per poll | None | None | One inbound HTTP request per event | None |
| Missed events on restart | No — deduplication by issue/PR number | No — dedup by fire time / workflow ID | Yes — events not stored | Depends on sender retry policy | N/A |
| Network requirements | Outbound to GitHub | None | None | Inbound HTTP (public endpoint or tunnel) | None |
| Rate limits | GitHub API rate limits apply | None | None | Depends on sender volume | None |

### Polling vs. webhooks for GitHub

| Scenario | Recommendation |
|----------|----------------|
| Behind a firewall, no public endpoint | Use `github_issues` / `github_pull_requests` polling |
| Need sub-second latency | Use `webhook` |
| Network unreliable | Use polling — catches up automatically on next poll |
| High-volume GitHub events | Use `webhook` — polling may miss events between polls |

---

## Triggering Any Workflow On Demand

Any workflow, regardless of trigger type, can be dispatched immediately via:

```bash
# CLI
agent orchestrator trigger-workflow <WORKFLOW_ID> \
  --title "Ad-hoc run" \
  --body "Description of what to do"

# API
curl -X POST http://127.0.0.1:17006/workflows/<ID>/trigger \
  -H "Content-Type: application/json" \
  -d '{"title": "Ad-hoc run", "body": "Description", "metadata": {"key": "value"}}'
```

This bypasses the workflow's normal trigger strategy and dispatches immediately. See [Manual Triggers](manual-trigger.md) for full details on bypass trigger semantics and response codes.

---

## Troubleshooting

### Workflow is enabled but not firing

1. **Check the trigger config** — `agent orchestrator get-workflow <ID>` shows the active `trigger_config`.
2. **Check dispatch history** — `agent orchestrator workflow-history <ID>` shows recent dispatches. A new entry should appear after each firing.
3. **Check agent connectivity** — `agent orchestrator get-agent <AGENT_ID>` must show `status: running`. Workflows skip dispatch if the agent is not connected.
4. **Check logs** — the orchestrator logs at `INFO` level for each trigger event and dispatch attempt.

### Cron workflow not firing at the expected time

- All cron expressions are evaluated in **UTC**. Convert your local time to UTC.
- Validate your expression with an external tool like [crontab.guru](https://crontab.guru/).
- Check that `enabled: true` — a disabled workflow's runner is not started and the cron strategy is never executed.

### Delay workflow fired immediately (not at scheduled time)

- `run_at` is in the past. If `run_at` has already passed when the runner starts, the delay trigger fires immediately. Verify the datetime and recreate the workflow with a future `run_at`.

### Webhook returning 404

- The workflow UUID in the URL must match exactly.
- The workflow must be `enabled: true` — a disabled workflow's runner is not started, so the webhook channel does not exist.
- Verify with `agent orchestrator get-workflow <ID>`.

### Webhook returning 401

- HMAC signature mismatch. Verify that the secret in the trigger config matches the secret configured in your external system (e.g., GitHub webhook settings).
- Ensure the request body is not modified in transit (e.g., by a proxy that re-formats JSON).

### `dispatch_result` workflow not chaining

- The `source_workflow_id` must exactly match the upstream workflow's UUID.
- The upstream workflow must complete (not just dispatch) — `dispatch_result` fires on `DispatchCompleted`, which is published after the agent finishes.
- Check that `status` in the trigger config matches the upstream's actual completion status (`completed` vs `failed`).

### Agent lifecycle workflow not firing on `session_start`

- The workflow must be created and enabled **before** the agent connects. If the agent was already connected when the workflow was created, the `session_start` event has already fired and will not be re-delivered.
- Events are not stored — a missed event cannot be replayed.

---

## Full Documentation

| Topic | Page |
|-------|------|
| TriggerStrategy trait and architecture | [Trigger Strategies](trigger-strategies.md) |
| Cron and delay triggers | [Schedule Triggers](schedule-triggers.md) |
| Agent lifecycle and dispatch result triggers | [Event-Driven Triggers](event-triggers.md) |
| Webhook trigger and GitHub setup | [Webhook Triggers](webhook-triggers.md) |
| Manual trigger and on-demand dispatch | [Manual Triggers](manual-trigger.md) |
| YAML template schema | [Templates](templates.md) |
| Migration from `source_config` | [Migration Guide](migration-trigger.md) |
