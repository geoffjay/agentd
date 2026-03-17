# Trigger Strategies

Trigger strategies define *when* a workflow runs and *what tasks* it produces. They decouple the scheduling mechanism from the dispatch logic, making it straightforward to add new trigger types without touching the runner loop.

## Architecture

```mermaid
graph LR
    Runner -->|calls next_tasks()| Strategy
    Strategy -->|returns Vec<Task>| Runner
    Runner -->|renders prompt + dispatches| Agent

    subgraph "Trigger Layer"
        Strategy["TriggerStrategy"]
        Polling["PollingStrategy"]
        CronS["CronStrategy"]
        DelayS["DelayStrategy"]
        EventS["EventStrategy"]
        TaskSrc["TaskSource (e.g. GithubIssueSource)"]
        EventBus["EventBus"]
        Polling -->|delegates to| TaskSrc
        EventS -->|subscribes to| EventBus
        Strategy -.->|implemented by| Polling
        Strategy -.->|implemented by| CronS
        Strategy -.->|implemented by| DelayS
        Strategy -.->|implemented by| EventS
    end
```

The **trigger layer** answers: *"Is there work to do right now?"*
The **dispatch layer** answers: *"How do I send that work to an agent?"*

Source code lives in `crates/orchestrator/src/scheduler/`.

---

## `TriggerStrategy` Trait

**File:** `crates/orchestrator/src/scheduler/strategy.rs`

```rust
#[async_trait]
pub trait TriggerStrategy: Send + Sync {
    /// Wait for the next trigger event and return tasks to dispatch.
    ///
    /// Implementations should respect the `shutdown` receiver and return
    /// promptly (with an empty vec or an error) when the signal fires.
    ///
    /// Returning an empty `Vec<Task>` is valid and indicates that no work
    /// is available at this time — the runner may call `next_tasks` again.
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>>;
}
```

### Contract

| Aspect | Behaviour |
|--------|-----------|
| **Return: tasks** | A non-empty `Vec<Task>` — the runner dispatches each task to the agent in sequence |
| **Return: empty vec** | No work available; the runner calls `next_tasks()` again on the next iteration |
| **Return: `Err`** | Transient failure; the runner logs the error, applies backoff, and retries |
| **Shutdown** | When `*shutdown.borrow() == true` the implementation must return promptly — `Ok(vec![])` is the correct response |
| **Thread safety** | Implementors must be `Send + Sync` so they can be boxed and moved across task boundaries |

### Runner loop integration

The runner holds a `Box<dyn TriggerStrategy>` and drives it in a loop:

```rust
loop {
    let tasks = strategy.next_tasks(&shutdown_rx).await?;
    for task in tasks {
        dispatch(task, &config, &registry).await?;
    }
}
```

The loop exits when the shutdown channel fires or `next_tasks` returns an unrecoverable error.

---

## `PollingStrategy` — Reference Implementation

`PollingStrategy` is the built-in implementation used by all poll-based workflows (currently GitHub Issues and GitHub Pull Requests).

**File:** `crates/orchestrator/src/scheduler/strategy.rs`

### How it works

1. Sleep for `poll_interval_secs` (interruptible by shutdown signal).
2. Call `TaskSource::fetch_tasks()` on the underlying source.
3. Return the tasks, or apply backoff and propagate the error on failure.

### Exponential backoff

When `fetch_tasks()` fails, `PollingStrategy` adds a linear backoff on top of the regular interval:

```
sleep_duration = poll_interval_secs + min(consecutive_errors × 2, 30)
```

`consecutive_errors` resets to `0` on the first successful fetch. The maximum additional delay is **30 seconds**.

### Shutdown handling

`PollingStrategy` uses `tokio::select!` to make the sleep interruptible:

```rust
tokio::select! {
    _ = tokio::time::sleep(sleep_dur) => {}
    _ = shutdown.changed() => {
        if *shutdown.borrow() { return Ok(vec![]); }
    }
}
```

A workflow that is disabled or deleted responds to shutdown within milliseconds regardless of how long the configured poll interval is.

---

## `TriggerConfig` Enum

`TriggerConfig` is the serialisable description of which strategy to use for a workflow. It is stored as JSON in the `trigger_config` column of the `workflows` table.

**File:** `crates/orchestrator/src/scheduler/types.rs`

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerConfig {
    GithubIssues {
        owner: String,
        repo: String,
        labels: Vec<String>,            // default: []
        state: String,                  // default: "open"
    },
    GithubPullRequests {
        owner: String,
        repo: String,
        labels: Vec<String>,            // default: []
        state: String,                  // default: "open"
    },
    Cron { expression: String },        // Phase 2
    Delay { run_at: String },           // Phase 2 — ISO 8601
    AgentLifecycle { event: String },   // Phase 3
    DispatchResult {                    // Phase 3
        source_workflow_id: Option<Uuid>,
        status: Option<DispatchStatus>,
    },
    Webhook { secret: Option<String> }, // Phase 4
    Manual {},
}
```

!!! info "Implementation status"
    `github_issues`, `github_pull_requests`, `cron`, `delay`, `agent_lifecycle`, and `dispatch_result` are fully implemented. `webhook` and `manual` are defined but not yet runnable — attempting to create a workflow with either type returns `400 Invalid Input`.

    - See [Schedule Triggers](schedule-triggers.md) for `cron` and `delay` documentation.
    - See [Event-Driven Triggers](event-triggers.md) for `agent_lifecycle` and `dispatch_result` documentation.

### JSON tagged-union format

`TriggerConfig` uses `#[serde(tag = "type")]` — the discriminant is the `type` key:

```json
{ "type": "github_issues", "owner": "myorg", "repo": "myrepo", "labels": ["agent"] }
{ "type": "github_pull_requests", "owner": "myorg", "repo": "myrepo", "state": "open" }
{ "type": "cron", "expression": "0 9 * * MON-FRI" }
{ "type": "delay", "run_at": "2026-04-01T09:00:00Z" }
{ "type": "agent_lifecycle", "event": "session_start" }
{ "type": "dispatch_result", "source_workflow_id": "<UUID>", "status": "completed" }
```

---

## `TaskSource` Trait

`TriggerStrategy` and `TaskSource` are two distinct abstractions:

| Trait | Responsibility |
|-------|---------------|
| `TriggerStrategy` | *When* to run — owns timing, backoff, shutdown |
| `TaskSource` | *What* to fetch — owns the external API call |

```rust
#[async_trait]
pub trait TaskSource: Send + Sync {
    async fn fetch_tasks(&self) -> anyhow::Result<Vec<Task>>;
    fn source_type(&self) -> &'static str;
}
```

Implemented by `GithubIssueSource` and `GithubPullRequestSource` in `crates/orchestrator/src/scheduler/github.rs`. Both call the `gh` CLI to list issues/PRs and map them to `Task` structs.

---

## Adding a New Trigger Type

To add a new trigger type (e.g. `Cron`):

1. **Add a variant** to `TriggerConfig` in `scheduler/types.rs`.
2. **Implement `TriggerStrategy`** (or a new `TaskSource` if it's poll-based) in `scheduler/strategy.rs` or a new file.
3. **Wire it** in `create_strategy()` in `scheduler/runner.rs`.
4. **Add CLI support** in the `TriggerType` enum in `crates/cli/src/commands/orchestrator.rs`.
5. **Remove the "not yet implemented" guard** in `scheduler/api.rs` for the new variant.

See [Trigger Migration Guide](../migration-trigger.md) for the history of the `source_*` → `trigger_*` rename.
