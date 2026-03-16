# Schedule Triggers

Schedule triggers let a workflow run on a time-based schedule rather than in response to an external event like a GitHub issue. Two schedule trigger types are available as of Phase 2:

| Type | When it fires | Repeats? |
|------|--------------|----------|
| `cron` | On a recurring schedule defined by a cron expression | Yes — indefinitely |
| `delay` | Once at a specific datetime | No — auto-disables after firing |

Both types synthesise a `Task` internally rather than fetching tasks from an external source. The runner dispatches this synthetic task to the connected agent using the workflow's prompt template.

---

## Cron Trigger

### Configuration format

=== "JSON (API)"

    ```json
    {
      "type": "cron",
      "expression": "0 9 * * MON-FRI"
    }
    ```

=== "YAML (.agentd/)"

    ```yaml
    source:
      type: cron
      expression: "0 9 * * MON-FRI"
    ```

The `expression` field is required and must be a valid cron expression (see [syntax reference](#cron-expression-syntax) below).

### Cron expression syntax

agentd uses the [`croner`](https://crates.io/crates/croner) Rust crate to parse cron expressions. Both the standard 5-field format and an extended 6-field format with a leading seconds field are supported.

**5-field format (standard):**

```
┌───────────── minute (0–59)
│ ┌───────────── hour (0–23)
│ │ ┌───────────── day of month (1–31)
│ │ │ ┌───────────── month (1–12 or JAN–DEC)
│ │ │ │ ┌───────────── day of week (0–7 or SUN–SAT, 0 and 7 = Sunday)
│ │ │ │ │
* * * * *
```

**6-field format (with seconds):**

```
┌───────────── second (0–59)
│ ┌───────────── minute (0–59)
│ │ ┌───────────── hour (0–23)
│ │ │ ┌───────────── day of month (1–31)
│ │ │ │ ┌───────────── month (1–12 or JAN–DEC)
│ │ │ │ │ ┌───────────── day of week (0–7 or SUN–SAT)
│ │ │ │ │ │
* * * * * *
```

Supported special characters:

| Character | Meaning | Example |
|-----------|---------|---------|
| `*` | Any value | `* * * * *` — every minute |
| `,` | List of values | `0,30 * * * *` — at :00 and :30 |
| `-` | Range | `MON-FRI` — Monday through Friday |
| `/` | Step | `*/5 * * * *` — every 5 minutes |

### Common expressions

| Expression | Schedule |
|------------|----------|
| `* * * * *` | Every minute |
| `*/5 * * * *` | Every 5 minutes |
| `0 * * * *` | Every hour (on the hour) |
| `0 9 * * *` | Daily at 9:00 AM UTC |
| `0 9 * * MON-FRI` | Weekday mornings at 9:00 AM UTC |
| `0 0 * * *` | Daily at midnight UTC |
| `0 12 1 * *` | First of the month at noon UTC |
| `30 4 * * SUN` | Sundays at 4:30 AM UTC |
| `0 */6 * * *` | Every 6 hours |
| `0 0 1 1 *` | New Year's Day (once per year) |
| `* * * * * *` | Every second (6-field format) |

!!! tip "Timezone"
    Cron expressions are evaluated in **UTC**. If your schedule should align with a local timezone, convert the desired local time to UTC. For example, "9:00 AM EST (UTC-5)" becomes `0 14 * * *`.

### Template variables

When a cron workflow fires, the synthetic task includes these metadata variables for use in `{{placeholders}}`:

| Variable | Description | Example |
|----------|-------------|---------|
| `{{fire_time}}` | RFC 3339 timestamp when the cron fired | `2026-04-01T09:00:00Z` |
| `{{cron_expression}}` | The cron expression that fired | `0 9 * * MON-FRI` |
| `{{source_id}}` | Deduplication key (`cron:<fire_time>`) | `cron:2026-04-01T09:00:00Z` |
| `{{title}}` | Auto-generated title | `Cron trigger: 0 9 * * MON-FRI` |

The standard task variables (`{{body}}`, `{{url}}`, `{{labels}}`, `{{assignee}}`) are present but empty for cron triggers.

### Deduplication

The `source_id` for each cron firing is `cron:<fire_time_rfc3339>`, which is unique per firing. The scheduler's dedup check prevents the same fire time from dispatching twice even if the orchestrator restarts.

### How `poll_interval_secs` is handled

The `poll_interval_secs` field is stored in the database for all workflows but is **ignored** by the cron trigger. Instead of polling at a fixed interval, the `CronStrategy` calculates the next cron tick from the current time and sleeps precisely until that instant.

---

## Delay Trigger

### Configuration format

=== "JSON (API)"

    ```json
    {
      "type": "delay",
      "run_at": "2026-04-01T09:00:00Z"
    }
    ```

=== "YAML (.agentd/)"

    ```yaml
    source:
      type: delay
      run_at: "2026-04-01T09:00:00Z"
    ```

The `run_at` field must be a valid **RFC 3339 / ISO 8601** datetime string.

### Datetime format

`run_at` is parsed by `chrono::DateTime::parse_from_rfc3339`. Accepted formats:

```
2026-04-01T09:00:00Z          # UTC (recommended)
2026-04-01T09:00:00+00:00     # UTC with explicit offset
2026-04-01T05:00:00-04:00     # With timezone offset (EDT)
```

The orchestrator validates the format at workflow creation time and returns `400 Invalid Input` if the string cannot be parsed.

### One-shot semantics

A delay workflow fires **exactly once**, then auto-disables:

1. The runner sleeps until `run_at`.
2. It dispatches the synthetic task to the agent.
3. After dispatch, the runner sets `enabled = false` in the database and stops.

The workflow remains in storage with `enabled: false`. You can inspect it with `agent orchestrator get-workflow <ID>` and view its dispatch history.

### Behaviour when `run_at` is in the past

If `run_at` is already in the past when the runner starts, the delay trigger fires **immediately** (zero wait). This applies if:

- You create the workflow with a past `run_at` (intentional or not).
- The orchestrator was restarted after the scheduled time had passed.

!!! warning "Past `run_at` fires immediately"
    A delay workflow with `run_at` in the past will dispatch to the agent as soon as the runner loop starts. If this is not the intended behaviour, delete or disable the workflow before restarting the orchestrator.

### Template variables

| Variable | Description | Example |
|----------|-------------|---------|
| `{{run_at}}` | RFC 3339 datetime the trigger was scheduled for | `2026-04-01T09:00:00Z` |
| `{{workflow_id}}` | UUID of the workflow | `550e8400-e29b-41d4-a716-446655440001` |
| `{{source_id}}` | Deduplication key (`delay:<workflow_id>`) | `delay:550e8400-...` |
| `{{title}}` | Auto-generated title | `Delay trigger: 2026-04-01T09:00:00Z` |

### Deduplication

The `source_id` for a delay trigger is `delay:<workflow_uuid>`, which is stable across orchestrator restarts. This means: even if the orchestrator restarts after the trigger fires but before the dispatch record is written, the dedup check prevents a double-dispatch.

---

## CLI Usage

### Create a cron workflow

```bash
agent orchestrator create-workflow \
  --name daily-report \
  --agent-name worker \
  --trigger-type cron \
  --cron-expression "0 9 * * MON-FRI" \
  --prompt-template "Generate the daily status report for {{fire_time}}."
```

The `--cron-expression` argument is **required** when `--trigger-type cron` is used. Supplying an invalid expression returns an error immediately — the expression is validated by the API at creation time.

### Create a delay workflow

```bash
agent orchestrator create-workflow \
  --name april-fools-task \
  --agent-name worker \
  --trigger-type delay \
  --run-at "2026-04-01T09:00:00Z" \
  --prompt-template "It is now {{run_at}}. Run the April Fools deployment."
```

The `--run-at` argument is **required** when `--trigger-type delay` is used.

### Observe a workflow firing

After creating a workflow, watch its status:

```bash
# Show workflow details (enabled status, trigger config)
agent orchestrator get-workflow <WORKFLOW_ID>

# Watch dispatch history — new entries appear after each firing
agent orchestrator dispatch-history <WORKFLOW_ID>
```

Example dispatch history entry after a cron firing:

```
ID:           a1b2c3d4-...
Workflow:     daily-report
Source ID:    cron:2026-04-01T09:00:00Z
Status:       completed
Dispatched:   2026-04-01T09:00:01Z
Completed:    2026-04-01T09:04:23Z
```

After a delay workflow fires, `get-workflow` shows `enabled: false`:

```bash
agent orchestrator get-workflow <WORKFLOW_ID>
# Status: disabled   ← auto-disabled after firing
```

---

## YAML Template Examples

### Cron workflow — daily report

`.agentd/workflows/daily-report.yml`:

```yaml
name: daily-report
agent: reporter

source:
  type: cron
  expression: "0 9 * * MON-FRI"

prompt_template: |
  It is {{fire_time}} (schedule: {{cron_expression}}).

  Generate the daily engineering status report:
  1. Summarise open pull requests in geoffjay/agentd
  2. List any failing CI runs
  3. Post a summary to the team channel

enabled: true
```

### Cron workflow — hourly health check

`.agentd/workflows/health-check.yml`:

```yaml
name: health-check
agent: monitor

source:
  type: cron
  expression: "0 * * * *"

prompt_template: |
  Hourly health check at {{fire_time}}.
  Check all service endpoints and report any anomalies.

tool_policy:
  mode: allow_list
  tools:
    - Bash
    - WebFetch

enabled: true
```

### Delay workflow — one-shot deployment

`.agentd/workflows/scheduled-deploy.yml`:

```yaml
name: scheduled-deploy
agent: deployer

source:
  type: delay
  run_at: "2026-04-01T02:00:00Z"

prompt_template: |
  Scheduled maintenance window has started ({{run_at}}).
  Deploy the v2.0 release:
  1. Pull the latest release tag
  2. Run database migrations
  3. Restart services
  4. Verify health checks pass

tool_policy:
  mode: allow_list
  tools:
    - Bash

enabled: true
```

Apply a single workflow template:

```bash
agent apply .agentd/workflows/daily-report.yml
```

Or apply the whole project (creates agents first, then workflows):

```bash
agent apply .agentd/
```

---

## End-to-End Example

The following walkthrough creates a cron workflow, watches it fire, and inspects the dispatch history.

**1. Start an agent:**

```bash
agent orchestrator create-agent --name reporter
# Note the agent ID from the output
```

**2. Create the cron workflow:**

```bash
agent orchestrator create-workflow \
  --name weekday-standup \
  --agent-name reporter \
  --trigger-type cron \
  --cron-expression "0 9 * * MON-FRI" \
  --prompt-template "Daily standup at {{fire_time}}. Summarise yesterday's commits."
```

**3. Confirm the workflow was created:**

```bash
agent orchestrator list-workflows
agent orchestrator get-workflow <WORKFLOW_ID>
```

**4. Wait for the next fire time, then check history:**

```bash
agent orchestrator dispatch-history <WORKFLOW_ID>
```

**5. When done, delete the workflow:**

```bash
agent orchestrator delete-workflow <WORKFLOW_ID>
```

---

## Operational Notes

### `poll_interval_secs` is ignored

For `cron` and `delay` triggers, `poll_interval_secs` is stored (defaults to `60`) but has no effect. The value is only used by poll-based triggers (`github_issues`, `github_pull_requests`). Schedule triggers sleep precisely until their next event.

### Timezone handling

All cron expressions and `run_at` datetimes are interpreted in **UTC**. The orchestrator makes no attempt to adjust for the system timezone or any per-workflow timezone setting.

### Deduplication and restart safety

Both trigger types are designed to be safe across orchestrator restarts:

- **Cron** uses `cron:<fire_time_rfc3339>` as the `source_id`. If the orchestrator restarts mid-cycle, the dedup check prevents the same fire time from dispatching twice.
- **Delay** uses `delay:<workflow_id>`. Because the workflow UUID is stable, the same one-shot task is never dispatched more than once.

### Auto-disable after delay fires

When a `delay` workflow fires, the orchestrator sets `enabled = false` in the database automatically (via `RunOutcome::AutoDisable`). The workflow is not deleted — it stays in storage so you can inspect its dispatch history. To re-run a one-shot task, either create a new workflow or re-enable the existing one with:

```bash
agent orchestrator update-workflow <WORKFLOW_ID> --enabled true
```

!!! note
    Re-enabling a delay workflow whose `run_at` is in the past will cause it to fire immediately on the next runner start.
