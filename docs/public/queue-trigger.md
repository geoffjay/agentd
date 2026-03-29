# Queue-Based Trigger

The `queue` trigger type allows workflows to consume tasks from a named internal
queue. Producers push work via the REST API; the workflow's agent processes items
one at a time with visibility timeout and automatic retry semantics.

## Overview

```
Producer                 Orchestrator                  Agent
   │                          │                          │
   │  POST /queues/q/push      │                          │
   │─────────────────────────>│                          │
   │  { "title": "...", ... }  │                          │
   │                          │ dequeue (atomic)          │
   │                          │ visibility timeout starts │
   │                          │─────────────────────────>│
   │                          │  dispatch prompt          │
   │                          │<─────────────────────────│
   │                          │  task complete            │
```

Tasks are dequeued in **priority-descending, FIFO** order:

- Tasks with higher `priority` values are processed first.
- Among tasks with equal priority, the oldest is taken first.

While a task is processing, it is invisible to other consumers for
`visibility_timeout_secs` seconds (default 300 / 5 minutes).

---

## Configuration

### `TriggerConfig` variant

```json
{
  "type": "queue",
  "queue_name": "work-items",
  "poll_interval_secs": 5,
  "visibility_timeout_secs": 300
}
```

| Field                    | Type     | Required | Default | Description                                         |
|--------------------------|----------|----------|---------|-----------------------------------------------------|
| `queue_name`             | `string` | yes      | —       | Name of the queue to consume from                   |
| `poll_interval_secs`     | `u64`    | no       | `5`     | How often to poll when the queue is empty (seconds) |
| `visibility_timeout_secs`| `u64`    | no       | `300`   | Visibility timeout for dequeued tasks (seconds)     |

**Queue name rules:** alphanumeric characters and hyphens only, 1–64 characters.

---

## Creating a Queue-Triggered Workflow

### Via REST API

```bash
curl -X POST http://localhost:7006/workflows \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "report-processor",
    "agent_id": "<agent-uuid>",
    "trigger_config": {
      "type": "queue",
      "queue_name": "reports",
      "poll_interval_secs": 5,
      "visibility_timeout_secs": 300
    },
    "prompt_template": "Process report: {{title}}\n\nDetails: {{body}}\n\nQueue: {{queue_name}}, Task ID: {{queue_task_id}}"
  }'
```

### Via CLI

```bash
agent orchestrator create-workflow \
  --name report-processor \
  --agent-name reporter \
  --trigger-type queue \
  --queue-name reports \
  --queue-poll-interval 5 \
  --queue-visibility-timeout 300 \
  --prompt-template "Process report: {{title}}\n\nDetails: {{body}}"
```

---

## Pushing Tasks

### REST API

```bash
curl -X POST http://localhost:7006/queues/reports/push \
  -H 'Content-Type: application/json' \
  -d '{
    "title": "Process Q4 report",
    "body": "{\"report_id\": 42, \"format\": \"pdf\"}",
    "priority": 5
  }'
```

**Response (201 Created):**

```json
{
  "id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
  "queue_name": "reports",
  "status": "pending",
  "created_at": "2026-03-28T12:00:00Z"
}
```

| Field      | Type     | Required | Default | Description                          |
|------------|----------|----------|---------|--------------------------------------|
| `title`    | `string` | yes      | —       | Short description of the task        |
| `body`     | `string` | no       | `""`    | Task payload (JSON, text, etc.)      |
| `priority` | `i32`    | no       | `0`     | Higher values processed first        |

---

## Queue Management

### Statistics

```bash
# REST
GET /queues/{name}/stats

# CLI
agent orchestrator queue-stats reports
```

**Response:**

```json
{
  "pending":    3,
  "processing": 1,
  "completed":  42,
  "failed":     2,
  "dead":       1
}
```

### Peek (non-destructive view)

```bash
# REST
GET /queues/{name}/peek?limit=10

# CLI
agent orchestrator queue-peek reports --limit 5
```

Returns up to `limit` pending tasks without claiming them. Default limit: 10, max: 100.

### Purge

```bash
# REST
DELETE /queues/{name}

# CLI
agent orchestrator queue-purge reports
```

**Response:**

```json
{ "deleted": 7 }
```

Removes **all** tasks from the queue regardless of status. Use with care.

---

## Template Variables

Queue workflows have access to these `{{placeholder}}` variables in addition to
standard task fields:

| Variable          | Description                             |
|-------------------|-----------------------------------------|
| `{{queue_name}}`  | Name of the queue the task came from    |
| `{{queue_task_id}}`| Internal UUID of the queue task        |
| `{{queue_priority}}`| Priority value assigned at push time  |
| `{{title}}`       | Task title (from push request)          |
| `{{body}}`        | Task body / payload (from push request) |

---

## Retry and Dead-Letter Semantics

When a task fails to dispatch (agent unavailable, network error, etc.):

1. The task's `retry_count` is incremented.
2. If `retry_count < max_retries` (default 3), the task is moved back to `pending`.
3. If `retry_count >= max_retries`, the task is moved to `dead` (dead letter).

Dead-letter tasks are not retried automatically. They remain in the queue for
inspection via `queue-stats` and `queue-peek` (visible in stats but not in peek,
which only shows pending tasks). Purge the queue to remove them.

---

## Architecture Patterns

### Simple background work queue

```
Agent A (producer) → POST /queues/bg-tasks/push
Agent B (worker)   → queue trigger workflow consuming bg-tasks
```

### Priority queue for tiered processing

```bash
# High priority bug fix
curl -X POST .../queues/work/push -d '{"title":"Critical bug","priority":100}'

# Normal feature request
curl -X POST .../queues/work/push -d '{"title":"New feature","priority":10}'
```

### Multi-producer pattern with webhooks feeding the queue

```
GitHub webhook → webhook-workflow → POST /queues/gh-issues/push
Linear webhook → webhook-workflow → POST /queues/gh-issues/push
Queue workflow → consumes gh-issues → agent processes unified stream
```

---

## Troubleshooting

**Tasks are not being picked up:**
- Verify the workflow is enabled: `agent orchestrator get-workflow <id>`
- Check that `queue_name` in the trigger config matches the name used in `POST /queues/{name}/push`
- Confirm the agent is running: `agent orchestrator list-agents`

**Tasks stay in `processing` state:**
- The agent may be busy or disconnected.
- After `visibility_timeout_secs` have elapsed, the task will become visible again
  and be retried automatically (up to `max_retries`).

**Queue grows unboundedly:**
- The consumer workflow may be disabled or the agent may be unavailable.
- Monitor with `queue-stats` and adjust producer rate.

**Dead-letter tasks:**
- Check agent logs for error details.
- Use `queue-purge` to clear dead-letter tasks after investigation.
