# Event-Driven Triggers

Event-driven triggers fire workflows in response to internal orchestrator events rather than on a schedule or by polling an external source. Three event trigger types are available:

| Type | When it fires |
|------|--------------|
| `agent_lifecycle` | When a specific agent connects, disconnects, or clears its context |
| `dispatch_result` | When a workflow dispatch completes, enabling workflow chaining |
| `ask_response` | When a human answers or dismisses an agent's question (the ask service) |

These types are backed by the internal [Event Bus](#event-bus-architecture) and implemented via `EventStrategy`.

!!! tip "YAML template support"
    `dispatch_result` and `agent_lifecycle` triggers can be configured in `.agentd/` workflow YAML files (see [Pipeline chaining with YAML templates](#pipeline-chaining-with-yaml-templates)) as well as via the REST API.

---

## Event Bus Architecture

The orchestrator maintains a shared in-process event bus that connects internal components. All event-driven workflows subscribe to this bus through `EventStrategy`.

```mermaid
graph LR
    subgraph "Event Publishers"
        WS["ConnectionRegistry\n(WebSocket)"]
        MGR["AgentManager"]
        SCHED["Scheduler"]
    end

    BUS["EventBus\n(broadcast, capacity 256)"]

    WS -->|AgentConnected\nAgentDisconnected| BUS
    MGR -->|ContextCleared| BUS
    SCHED -->|DispatchCompleted| BUS

    subgraph "Event Consumers"
        ES1["EventStrategy\n(lifecycle workflow)"]
        ES2["EventStrategy\n(dispatch-result workflow)"]
    end

    BUS -->|broadcast| ES1
    BUS -->|broadcast| ES2

    ES1 -->|Vec<Task>| Runner1["WorkflowRunner A"]
    ES2 -->|Vec<Task>| Runner2["WorkflowRunner B"]

    Runner1 -->|prompt| Agent1["Agent"]
    Runner2 -->|prompt| Agent2["Agent"]
```

**Source code:** `crates/orchestrator/src/scheduler/events.rs`

### `SystemEvent` variants

| Variant | Published by | When |
|---------|-------------|------|
| `AgentConnected { agent_id }` | `ConnectionRegistry` | An agent establishes a WebSocket connection |
| `AgentDisconnected { agent_id }` | `ConnectionRegistry` | An agent's WebSocket connection is closed |
| `ContextCleared { agent_id }` | `AgentManager` | An agent's conversation context is cleared (`/clear`) |
| `DispatchCompleted { workflow_id, dispatch_id, status, source_id }` | `Scheduler` | A workflow dispatch finishes (success or failure). `source_id` is the original task identifier (e.g. GitHub issue or PR number). |

### Broadcast channel model

The bus uses `tokio::sync::broadcast` with a **capacity of 256**. Key behaviours:

- **Fan-out:** Every active `EventStrategy` subscriber receives every event.
- **Lag:** A subscriber that falls behind loses events older than the channel capacity. `EventStrategy` logs a warning and continues - it will not miss future events. See [Handling broadcast lag](#handling-broadcast-lag).
- **No persistence:** Events are not stored. A workflow that is not running when an event fires will not receive it.
- **No-op publish:** Publishing when no workflows are subscribed is safe and silent.

### Event ordering

Events are delivered to each subscriber in the order they were published. There are **no ordering guarantees** between different subscribers - two workflows may react to the same event at different times depending on their processing speed.

---

## `agent_lifecycle` Trigger

### Configuration format

```json
{
  "type": "agent_lifecycle",
  "event": "session_start"
}
```

**Field reference:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Must be `"agent_lifecycle"` |
| `event` | string | Yes | Lifecycle event to listen for (see table below) |

**Supported event values:**

| `event` value | Fires when | System event |
|--------------|-----------|-------------|
| `session_start` | The workflow's agent establishes a WebSocket connection | `AgentConnected` |
| `session_end` | The workflow's agent's WebSocket connection is closed | `AgentDisconnected` |
| `context_clear` | The workflow's agent's conversation context is cleared | `ContextCleared` |

The API validates `event` at creation time and returns `400 Invalid Input` for any other value.

### Agent ID matching

An `agent_lifecycle` workflow only fires for its **own assigned agent**. If multiple agents are running and one of them connects, only the workflow whose `agent_id` matches the connecting agent produces a task. Events for other agents are silently ignored.

This makes `agent_lifecycle` safe to use in multi-agent deployments - each workflow responds only to its own agent.

### Synthetic task structure

When the trigger fires, a synthetic `Task` is produced with these fields:

| Field | Value |
|-------|-------|
| `source_id` | `event:{event_type}:{agent_id}:{timestamp}` |
| `title` | `Agent lifecycle: {event_type}` |
| `body` | *(empty)* |
| `url` | *(empty)* |
| `labels` | *(empty)* |
| `assignee` | *(empty)* |

**Metadata map** (accessible as template variables):

| Key | Description | Example |
|-----|-------------|---------|
| `event_type` | The event name that fired | `session_start` |
| `agent_id` | UUID of the agent | `550e8400-...` |
| `timestamp` | RFC 3339 timestamp of the event | `2026-04-01T09:00:00Z` |

Because `source_id` includes both the agent UUID and the timestamp, each firing produces a unique identifier - the dedup check will never suppress a legitimate re-connection.

### Template variables

Use `{{event_type}}`, `{{agent_id}}`, and `{{timestamp}}` in prompt templates:

```
Agent {{agent_id}} fired a {{event_type}} event at {{timestamp}}.

Perform the necessary setup or cleanup tasks.
```

### Use cases

**Bootstrap on agent connect (`session_start`):**
Set up the agent's working environment, pull the latest code, or send an initial greeting every time the agent reconnects.

```json
{
  "name": "bootstrap-on-connect",
  "agent_id": "<AGENT_UUID>",
  "trigger_config": {
    "type": "agent_lifecycle",
    "event": "session_start"
  },
  "prompt_template": "Agent connected at {{timestamp}}. Pull latest changes and confirm the repo is clean.",
  "enabled": true
}
```

**Cleanup on agent disconnect (`session_end`):**
Log the disconnection, archive state, or notify a monitoring system when the agent goes offline.

**Re-initialise on context clear (`context_clear`):**
Re-inject important context after a `/clear` command resets the conversation history.

---

## `dispatch_result` Trigger

### Configuration format

```json
{
  "type": "dispatch_result",
  "source_workflow_id": "550e8400-e29b-41d4-a716-446655440001",
  "status": "completed"
}
```

**Field reference:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Must be `"dispatch_result"` |
| `source_workflow_id` | UUID string | No | Only match dispatches from this workflow. `null` or omitted = match any workflow |
| `status` | string | No | Only match dispatches with this status. `null` or omitted = match any status |

**Valid `status` values:** `pending`, `dispatched`, `completed`, `failed`, `skipped`

In practice, `dispatch_result` workflows are most useful filtering on `"completed"` or `"failed"`.

### Filtering behaviour

| `source_workflow_id` | `status` | Triggers when |
|---------------------|---------|---------------|
| set | set | That specific workflow completes with that specific status |
| set | `null` | That specific workflow completes with any status |
| `null` | set | Any workflow completes with that specific status |
| `null` | `null` | Any workflow dispatch completes |

### Synthetic task structure

| Field | Value |
|-------|-------|
| `source_id` | `event:dispatch:{dispatch_id}:{timestamp}` |
| `title` | `Dispatch completed: {dispatch_id} ({status})` |
| `body` | *(empty)* |
| `url` | *(empty)* |

**Metadata map:**

| Key | Description | Example |
|-----|-------------|---------|
| `source_workflow_id` | UUID of the workflow that completed | `550e8400-...` |
| `dispatch_id` | UUID of the specific dispatch record | `a1b2c3d4-...` |
| `status` | Completion status | `completed` |
| `timestamp` | RFC 3339 timestamp | `2026-04-01T09:05:00Z` |
| `original_source_id` | Source ID from the parent dispatch (e.g. GitHub issue or PR number). Present only when the parent workflow had a task-level source identifier. | `42` |

`source_id` includes both the dispatch UUID and the timestamp, so it is unique per event.

Use `{{original_source_id}}` in a chained workflow's prompt template to reference the GitHub issue or PR number that triggered the parent workflow. This is the key variable that makes triage→enrich and review→merge pipeline chains practical.

### Workflow chaining

`dispatch_result` enables building multi-stage pipelines where each stage triggers the next:

```
Workflow A (lint)
    │ completes
    ▼
Workflow B (test) ← dispatch_result trigger, source=A, status=completed
    │ completes
    ▼
Workflow C (deploy) ← dispatch_result trigger, source=B, status=completed
```

Each downstream workflow uses `{{source_workflow_id}}` and `{{dispatch_id}}` in its prompt to reference upstream context.

### Template variables

```
Workflow {{source_workflow_id}} completed with status {{status}} at {{timestamp}}.
Dispatch ID: {{dispatch_id}}.
Original issue/PR: {{original_source_id}}.

Run the next pipeline stage.
```

All `dispatch_result` variables are also listed in the [template variable reference](templates.md).

---

## `ask_response` Trigger

The `ask_response` trigger fires when a human **answers or dismisses** a question raised through the [ask service](services/ask.md). It enables human-in-the-loop pipelines: an agent asks a question, and a separate workflow reacts to the response.

!!! note "API-only"
    `ask_response` is created via the REST API only — it is not available in `.agentd/` YAML templates or through the CLI.

### Configuration format

```json
{
  "type": "ask_response",
  "agent_id": "dietician",
  "category": "health",
  "response_pattern": ".*"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Must be `"ask_response"` |
| `agent_id` | string | No | Only react to questions asked by this agent |
| `category` | string | No | Only react to questions in this category |
| `response_pattern` | string | No | Regex matched against the answer text |

All three filters are optional; omit them to react to every answered or dismissed question. When multiple filters are set, all must match.

### Template variables

| Variable | Description |
|----------|-------------|
| `{{question_id}}` | UUID of the answered question |
| `{{agent_id}}` | Which agent asked the question |
| `{{category}}` | Question category |
| `{{question}}` | The question text |
| `{{answer}}` | The human's answer (empty string if dismissed) |
| `{{event_type}}` | `question_answered` or `question_dismissed` |
| `{{workflow_id}}` | Originating workflow, if the question came from one |

---

## Pipeline Chaining with YAML Templates

Workflow chaining can be configured in `.agentd/` YAML files using `type: dispatch_result` in the `source` block. The orchestrator serialises the trigger configuration from YAML using the same `TriggerConfig` enum as the REST API.

### triage → enrich example

When the triage workflow completes, the conductor applies the `enrich-agent` label so the enrichment-worker picks up the issue next.

```yaml
name: triage-enrich-chain
agent: conductor

source:
  type: dispatch_result
  # source_workflow_id: "<TRIAGE_WORKER_UUID>"  # fill in after deployment
  status: completed

poll_interval: 60
enabled: true

prompt_template: |
  Workflow {{source_workflow_id}} completed. Advance issue #{{original_source_id}}
  to the enrichment stage by applying the enrich-agent label:

    gh issue edit {{original_source_id}} --repo geoffjay/agentd --add-label "enrich-agent"
```

The ready-to-deploy version is at `.agentd/workflows/triage-enrich-chain.yml`.

### review → merge example

When the reviewer workflow completes, the conductor checks whether the PR is approved and CI is passing, then applies `merge-ready` if so.

```yaml
name: review-merge-chain
agent: conductor

source:
  type: dispatch_result
  # source_workflow_id: "<REVIEWER_WORKFLOW_UUID>"  # fill in after deployment
  status: completed

poll_interval: 60
enabled: true

prompt_template: |
  Workflow {{source_workflow_id}} completed. Check PR #{{original_source_id}}
  for merge readiness and apply merge-ready if all criteria are met.
```

The ready-to-deploy version is at `.agentd/workflows/review-merge-chain.yml`.

### Setting `source_workflow_id` after deployment

YAML templates cannot know workflow UUIDs before deployment. After running `agent apply`, retrieve the UUID and patch the workflow:

```bash
# 1. Find the triage-worker UUID
agent orchestrator list-workflows | grep triage-worker
# Example output: triage-worker  550e8400-e29b-41d4-a716-446655440001  enabled

# 2. Find the chain workflow UUID
agent orchestrator list-workflows | grep triage-enrich-chain
# Example output: triage-enrich-chain  a1b2c3d4-e29b-41d4-a716-446655440002  enabled

# 3. Update the chain workflow via the API
curl -s -X PATCH http://127.0.0.1:17006/workflows/a1b2c3d4-e29b-41d4-a716-446655440002 \
  -H "Content-Type: application/json" \
  -d '{"source": {"type": "dispatch_result", "source_workflow_id": "550e8400-e29b-41d4-a716-446655440001", "status": "completed"}}'
```

---

## Creating Event-Driven Workflows via REST API

Event-driven triggers are created via the orchestrator REST API. Use `curl` or any HTTP client.

### Create an `agent_lifecycle` workflow

```bash
curl -s -X POST http://127.0.0.1:17006/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "bootstrap-on-connect",
    "agent_id": "<AGENT_UUID>",
    "trigger_config": {
      "type": "agent_lifecycle",
      "event": "session_start"
    },
    "prompt_template": "Agent connected at {{timestamp}}. Pull latest changes and confirm the working tree is clean.",
    "enabled": true
  }'
```

### Create a `dispatch_result` workflow (chained pipeline)

**Step 1 - Create the upstream workflow (lint):**

```bash
curl -s -X POST http://127.0.0.1:17006/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "lint",
    "agent_id": "<LINT_AGENT_UUID>",
    "trigger_config": {
      "type": "github_issues",
      "owner": "myorg",
      "repo": "myrepo",
      "labels": ["run-pipeline"]
    },
    "prompt_template": "Run cargo clippy on the codebase. Report any warnings.",
    "enabled": true
  }'
# → note the workflow ID from the response: LINT_WF_ID
```

**Step 2 - Create the downstream workflow (test), triggered when lint completes:**

```bash
curl -s -X POST http://127.0.0.1:17006/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test",
    "agent_id": "<TEST_AGENT_UUID>",
    "trigger_config": {
      "type": "dispatch_result",
      "source_workflow_id": "<LINT_WF_ID>",
      "status": "completed"
    },
    "prompt_template": "Lint (dispatch {{dispatch_id}}) completed at {{timestamp}}. Run cargo test and report results.",
    "enabled": true
  }'
# → note the workflow ID: TEST_WF_ID
```

**Step 3 - Create the deploy workflow, triggered when test completes:**

```bash
curl -s -X POST http://127.0.0.1:17006/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "deploy",
    "agent_id": "<DEPLOY_AGENT_UUID>",
    "trigger_config": {
      "type": "dispatch_result",
      "source_workflow_id": "<TEST_WF_ID>",
      "status": "completed"
    },
    "prompt_template": "Tests passed (dispatch {{dispatch_id}}). Deploy the release build.",
    "enabled": true
  }'
```

### Observe dispatch history

After the pipeline runs, inspect each workflow's dispatch history:

```bash
# List history for the lint workflow
curl -s http://127.0.0.1:17006/workflows/<LINT_WF_ID>/history | jq .

# Or via the CLI
agent orchestrator dispatch-history <LINT_WF_ID>
agent orchestrator dispatch-history <TEST_WF_ID>
agent orchestrator dispatch-history <DEPLOY_WF_ID>
```

---

## Operational Notes

### Handling broadcast lag

The event bus channel has a fixed capacity of **256 events**. If an `EventStrategy` subscriber falls behind (e.g. the agent is busy and dispatch is delayed), the channel may overflow and the subscriber will receive a `RecvError::Lagged` error.

When lag occurs:
- The `EventStrategy` logs a warning: `EventStrategy: subscriber lagged, some events may have been missed`
- The subscriber **resumes** from the oldest available event - it does **not** stop or crash
- Events that fell off the end of the channel are **permanently lost**

In a typical deployment (events fire at human timescales), the 256-event buffer is more than sufficient. High-frequency automation that fires hundreds of dispatches per second may need to reduce event-driven workflow complexity to avoid lag.

### Deduplication

Both event trigger types include a timestamp in `source_id`, making each firing unique:

- Lifecycle: `event:{event_type}:{agent_id}:{timestamp}` - unique per connect/disconnect
- Dispatch result: `event:dispatch:{dispatch_id}:{timestamp}` - unique per dispatch completion

The dedup check in the scheduler prevents re-dispatching the same event, which provides safety across orchestrator restarts if the same event happens to fire again quickly.

### No persistence

Events are not stored. If a workflow is disabled or its runner is not started when an event fires, the event is missed. Workflows should be created and enabled **before** the events they need to catch.

For `session_start` workflows, this means the workflow must be created and enabled before the agent connects. If the agent is already connected when the workflow is created, the `session_start` event has already fired and will not be re-delivered.

### Observing events in logs

The orchestrator logs an `info`-level message whenever it publishes each event type. Enable structured logging to correlate events with workflow dispatches:

```
INFO orchestrator::websocket agent_id=550e8400-... "Agent WebSocket registered"
INFO orchestrator::scheduler::runner workflow_id=... source_id=event:session_start:... "Dispatched task to agent"
```

For event lag warnings:

```
WARN orchestrator::scheduler::strategy lagged=3 "EventStrategy: subscriber lagged, some events may have been missed"
```

### Circular pipeline prevention

`dispatch_result` workflows can inadvertently create cycles if a downstream workflow's agent also completes a dispatch that is observed by an upstream filter. Design pipelines with unique agents or add `source_workflow_id` filters to ensure each stage only responds to its intended predecessor.
