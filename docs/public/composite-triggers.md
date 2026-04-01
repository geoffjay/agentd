# Composite Triggers

Composite triggers combine multiple trigger strategies using boolean logic
(**OR** or **AND**), enabling powerful multi-condition workflows.

## Overview

| Mode | Fires when… |
|------|-------------|
| `or` | **Any** sub-trigger produces tasks |
| `and` | **All** sub-triggers produce tasks within the correlation window |

Composite triggers are fully nestable (up to 3 levels deep) and support any
combination of existing trigger types as sub-triggers.

> **Note:** `manual` and `webhook` trigger types cannot be used as
> sub-triggers inside a composite because they require a dedicated channel
> that is managed by the scheduler. Use `cron`, `agent_lifecycle`,
> `dispatch_result`, `agent_idle`, `github_issues`, or `linear_issues` as
> sub-triggers.

---

## OR Combinator

The OR composite fires as soon as **any** one of its sub-triggers produces
tasks. It is the simplest combinator and is useful for "either/or" scenarios.

### Example: cron backup OR manual trigger

```json
{
  "name": "backup-or-manual",
  "agent_id": "<agent-uuid>",
  "trigger_config": {
    "type": "composite",
    "mode": "or",
    "triggers": [
      {
        "type": "cron",
        "expression": "0 2 * * *"
      },
      {
        "type": "agent_lifecycle",
        "event": "session_start"
      }
    ]
  },
  "prompt_template": "Run the backup procedure. Trigger: {{trigger_type}}"
}
```

### How OR works

1. Each sub-trigger runs in its own background task.
2. The first sub-trigger to produce tasks sends them over an internal channel.
3. `next_tasks()` returns immediately when it receives from the channel.
4. Other sub-triggers continue running - their results are queued for future
   `next_tasks()` calls.

---

## AND Combinator

The AND composite fires only when **all** sub-triggers have produced tasks
within a configurable **correlation window**.

### Example: GitHub issue AND cron window

```json
{
  "name": "issue-during-business-hours",
  "agent_id": "<agent-uuid>",
  "trigger_config": {
    "type": "composite",
    "mode": "and",
    "correlation_window_secs": 300,
    "triggers": [
      {
        "type": "github_issues",
        "owner": "my-org",
        "repo": "my-repo",
        "labels": ["needs-review"]
      },
      {
        "type": "cron",
        "expression": "0 9-17 * * MON-FRI"
      }
    ]
  },
  "prompt_template": "Review issue {{title}} during business hours."
}
```

### Correlation window semantics

The correlation window is a sliding timer:

1. The window starts when the **first** sub-trigger fires.
2. If **all** remaining sub-triggers fire before the window expires, the
   composite produces a merged task.
3. If the window expires before all sub-triggers fire, all partial state is
   discarded and the process restarts.

The default window is **60 seconds**. Use `correlation_window_secs` to
customise it.

### Merged task format

When an AND composite fires, all sub-tasks are merged into a single task:

| Field | Value |
|-------|-------|
| `source_id` | `composite:and:<sub-source-id-1>,<sub-source-id-2>,...` |
| `title` | Title from the first sub-task |
| `body` | Newline-joined bodies of all sub-tasks |
| `metadata.composite_sub_source_ids` | Comma-joined list of all sub-source IDs |
| `metadata.sub_<key>` | Each metadata key from each sub-task, prefixed with `sub_` |

Template variables available in AND composite prompts:

```
{{title}}                        - from first sub-task
{{body}}                         - merged bodies
{{source_id}}                    - composite:and:...
{{composite_sub_source_ids}}     - all sub-source IDs
```

---

## Nested composites

Composites can be nested up to **3 levels deep**:

```json
{
  "type": "composite",
  "mode": "or",
  "triggers": [
    {
      "type": "composite",
      "mode": "and",
      "correlation_window_secs": 120,
      "triggers": [
        { "type": "github_issues", "owner": "org", "repo": "repo", "labels": ["urgent"] },
        { "type": "agent_lifecycle", "event": "session_start" }
      ]
    },
    {
      "type": "cron",
      "expression": "0 */6 * * *"
    }
  ]
}
```

This example fires when either:
- A GitHub issue labelled `urgent` arrives **and** the agent is starting a new
  session (within 2 minutes), **or**
- The 6-hourly cron fires.

---

## Configuration reference

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `mode` | `"or"` \| `"and"` | ✓ | - | Combinator logic |
| `triggers` | array of trigger configs | ✓ | - | Sub-triggers (minimum 2) |
| `correlation_window_secs` | integer | AND only | `60` | How long all sub-triggers must fire within |

### Validation rules

- `mode` must be `"or"` or `"and"`.
- At least **2** sub-triggers are required.
- Nesting depth must not exceed **3 levels**.
- `manual` and `webhook` cannot be used as sub-triggers.

---

## Limitations and performance considerations

- **AND with long-lived sub-triggers**: If one sub-trigger fires very
  infrequently (e.g., a cron running once a week), the correlation window
  may need to be very large, or the AND combinator may never fire in practice.
- **Resource usage**: Each sub-trigger spawns its own background tokio task.
  Deeply nested or wide composites with many sub-triggers will each consume
  a task handle. Keep compositions reasonably bounded.
- **Deduplication**: The composite `source_id` is derived from the sub-tasks
  at dispatch time. If the same sub-task fires in two different windows, the
  resulting composite `source_id` will be different, so both dispatches will
  proceed.

---

## Troubleshooting

### AND composite never fires

- Check that all sub-triggers can actually produce tasks.
- Verify `correlation_window_secs` is long enough for all sub-triggers to fire.
- Check orchestrator logs for `"AND: correlation window expired"` messages -
  this means one or more sub-triggers fired but others did not within the window.

### Unexpected duplicate dispatches from OR composite

- Each fire from each sub-trigger is queued independently.
- If two sub-triggers fire in quick succession, both results will appear on
  sequential `next_tasks()` calls.
- The runner's deduplication check (`is_dispatched`) will prevent the same
  `source_id` from being dispatched twice, but tasks with different `source_id`
  values are treated as distinct.

### Composite workflow not starting

- Verify `is_implemented()` returns `true` for all sub-triggers.
- Check that event-bus-dependent sub-triggers (`agent_lifecycle`,
  `dispatch_result`, `agent_idle`) are not used when the event bus is
  unavailable.
