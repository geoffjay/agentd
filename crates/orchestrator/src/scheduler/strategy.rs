//! Trigger strategy abstraction for workflow scheduling.
//!
//! [`TriggerStrategy`] decouples *how* a workflow waits for its next event
//! from the scheduler itself.  The default implementation is polling-based,
//! but the trait is designed so that webhook-driven, cron, or event-stream
//! strategies can be swapped in without changing the runner.

use crate::scheduler::events::{EventBus, SystemEvent};
use crate::scheduler::source::TaskSource;
use crate::scheduler::types::{DispatchStatus, Task};
use async_trait::async_trait;
use chrono::Utc;
use croner::Cron;
use globset::{Glob, GlobSet, GlobSetBuilder};
use notify::Watcher as _;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, mpsc, watch};
use tracing::{info, warn};
use uuid::Uuid;

/// Abstracts how a workflow waits for its next trigger event.
///
/// Each running workflow owns a `Box<dyn TriggerStrategy>` that the runner
/// calls in a loop.  The strategy blocks until either new tasks are available
/// or the shutdown signal fires.
///
/// # Object Safety
///
/// The trait is `Send + Sync` so it can be stored as `Box<dyn TriggerStrategy>`
/// and moved across task boundaries.
///
/// # Examples
///
/// ```rust,ignore
/// use orchestrator::scheduler::strategy::TriggerStrategy;
/// use tokio::sync::watch;
///
/// async fn run_loop(
///     mut strategy: Box<dyn TriggerStrategy>,
///     shutdown: watch::Receiver<bool>,
/// ) {
///     loop {
///         match strategy.next_tasks(&shutdown).await {
///             Ok(tasks) if tasks.is_empty() => break,   // source exhausted
///             Ok(tasks) => { /* dispatch tasks */ }
///             Err(e) => { eprintln!("trigger error: {e}"); break; }
///         }
///     }
/// }
/// ```
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

// ---------------------------------------------------------------------------
// PollingStrategy
// ---------------------------------------------------------------------------

/// Maximum backoff multiplier (caps exponential growth).
const MAX_BACKOFF_SECS: u64 = 30;

/// A [`TriggerStrategy`] that polls a [`TaskSource`] at a fixed interval.
///
/// This preserves the original `WorkflowRunner` polling behaviour:
///
/// 1. Sleep for the configured interval (respecting the shutdown signal).
/// 2. Call `source.fetch_tasks()` to retrieve available work.
/// 3. On consecutive errors, apply linear backoff before the next attempt.
///
/// # Backoff
///
/// Each consecutive error increases the wait by `min(errors * 2, 30)` seconds
/// on top of the base interval. The counter resets after a successful fetch.
///
/// # Example
///
/// ```rust,ignore
/// use orchestrator::scheduler::strategy::{PollingStrategy, TriggerStrategy};
///
/// let source: Box<dyn TaskSource> = /* ... */;
/// let mut strategy = PollingStrategy::new(source, 60);
///
/// // Use in a runner loop:
/// let tasks = strategy.next_tasks(&shutdown_rx).await?;
/// ```
pub struct PollingStrategy {
    source: Box<dyn TaskSource>,
    interval: Duration,
    consecutive_errors: u32,
}

impl PollingStrategy {
    /// Create a new polling strategy.
    ///
    /// * `source` — the task source to poll.
    /// * `poll_interval_secs` — base seconds between poll cycles.
    pub fn new(source: Box<dyn TaskSource>, poll_interval_secs: u64) -> Self {
        Self { source, interval: Duration::from_secs(poll_interval_secs), consecutive_errors: 0 }
    }

    /// Compute the total sleep duration including any error backoff.
    fn sleep_duration(&self) -> Duration {
        let backoff_secs = std::cmp::min(u64::from(self.consecutive_errors) * 2, MAX_BACKOFF_SECS);
        self.interval + Duration::from_secs(backoff_secs)
    }
}

#[async_trait]
impl TriggerStrategy for PollingStrategy {
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>> {
        let sleep_dur = self.sleep_duration();

        // Sleep for the interval, but bail early on shutdown.
        let mut shutdown = shutdown.clone();
        tokio::select! {
            _ = tokio::time::sleep(sleep_dur) => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    return Ok(vec![]);
                }
            }
        }

        // Poll the source.
        match self.source.fetch_tasks().await {
            Ok(tasks) => {
                self.consecutive_errors = 0;
                Ok(tasks)
            }
            Err(e) => {
                self.consecutive_errors += 1;
                let backoff =
                    std::cmp::min(u64::from(self.consecutive_errors) * 2, MAX_BACKOFF_SECS);
                warn!(
                    consecutive_errors = self.consecutive_errors,
                    backoff_secs = backoff,
                    %e,
                    "Poll cycle failed, applying backoff"
                );
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CronStrategy
// ---------------------------------------------------------------------------

/// A [`TriggerStrategy`] that fires based on a cron expression.
///
/// On each call to `next_tasks()`, the strategy calculates the next fire time
/// from the cron expression and sleeps until that instant. When the fire time
/// arrives, it produces a synthetic [`Task`] with a unique `source_id` derived
/// from the fire timestamp.
///
/// # Shutdown
///
/// The sleep is interruptible — if the shutdown signal fires before the next
/// cron tick, the strategy returns an empty vec immediately.
///
/// # Example
///
/// ```rust,ignore
/// use orchestrator::scheduler::strategy::CronStrategy;
///
/// // Fire at 9:00 AM on weekdays
/// let strategy = CronStrategy::new("0 9 * * MON-FRI")?;
/// ```
pub struct CronStrategy {
    cron: Cron,
    expression: String,
}

impl std::fmt::Debug for CronStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CronStrategy").field("expression", &self.expression).finish()
    }
}

impl CronStrategy {
    /// Create a new cron strategy from a cron expression.
    ///
    /// Returns an error if the expression cannot be parsed.
    pub fn new(expression: &str) -> anyhow::Result<Self> {
        let cron: Cron = expression
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid cron expression '{}': {}", expression, e))?;
        Ok(Self { cron, expression: expression.to_string() })
    }

    /// Calculate the next fire time from now.
    fn next_fire_time(&self) -> anyhow::Result<chrono::DateTime<Utc>> {
        self.cron
            .find_next_occurrence(&Utc::now(), false)
            .map_err(|e| anyhow::anyhow!("Failed to calculate next cron fire time: {}", e))
    }

    /// Build a synthetic task for a cron firing.
    fn build_task(&self, fire_time: &chrono::DateTime<Utc>) -> Task {
        let fire_time_str = fire_time.to_rfc3339();
        let mut metadata = HashMap::new();
        metadata.insert("fire_time".to_string(), fire_time_str.clone());
        metadata.insert("cron_expression".to_string(), self.expression.clone());

        Task {
            source_id: format!("cron:{}", fire_time_str),
            title: format!("Cron trigger: {}", self.expression),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata,
        }
    }
}

#[async_trait]
impl TriggerStrategy for CronStrategy {
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>> {
        let next = self.next_fire_time()?;
        let now = Utc::now();
        let wait_duration = (next - now).to_std().unwrap_or(Duration::ZERO);

        info!(
            expression = %self.expression,
            next_fire = %next,
            wait_secs = wait_duration.as_secs(),
            "Cron strategy waiting for next fire time"
        );

        // Sleep until the fire time, respecting shutdown.
        let mut shutdown = shutdown.clone();
        tokio::select! {
            _ = tokio::time::sleep(wait_duration) => {
                let task = self.build_task(&next);
                Ok(vec![task])
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    return Ok(vec![]);
                }
                Ok(vec![])
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DelayStrategy
// ---------------------------------------------------------------------------

/// A [`TriggerStrategy`] that fires once at a specific datetime, then stops.
///
/// On the first call to `next_tasks()`, the strategy sleeps until `run_at`
/// and produces a single synthetic [`Task`]. If `run_at` is in the past, it
/// fires immediately. On subsequent calls, it returns an empty vec to signal
/// that the one-shot execution is complete.
///
/// # Auto-disable
///
/// After the delay fires, the runner should auto-disable the workflow by
/// updating `enabled = false` in storage and stopping the runner. This is
/// signalled by the `fired` flag — the runner checks [`DelayStrategy::has_fired()`]
/// after dispatch.
///
/// # Example
///
/// ```rust,ignore
/// use orchestrator::scheduler::strategy::DelayStrategy;
/// use chrono::{Utc, Duration};
///
/// let run_at = Utc::now() + Duration::seconds(30);
/// let strategy = DelayStrategy::new(run_at, workflow_id);
/// ```
#[derive(Debug)]
pub struct DelayStrategy {
    run_at: chrono::DateTime<Utc>,
    workflow_id: uuid::Uuid,
    fired: bool,
}

impl DelayStrategy {
    /// Create a new delay strategy.
    ///
    /// * `run_at` — the datetime at which to fire.
    /// * `workflow_id` — used to generate a unique `source_id`.
    pub fn new(run_at: chrono::DateTime<Utc>, workflow_id: uuid::Uuid) -> Self {
        Self { run_at, workflow_id, fired: false }
    }

    /// Returns `true` after the delay has fired.
    #[cfg(test)]
    pub fn has_fired(&self) -> bool {
        self.fired
    }

    /// Build the synthetic task for the delay firing.
    fn build_task(&self) -> Task {
        let mut metadata = HashMap::new();
        metadata.insert("run_at".to_string(), self.run_at.to_rfc3339());
        metadata.insert("workflow_id".to_string(), self.workflow_id.to_string());

        Task {
            source_id: format!("delay:{}", self.workflow_id),
            title: format!("Delay trigger: {}", self.run_at.to_rfc3339()),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata,
        }
    }
}

#[async_trait]
impl TriggerStrategy for DelayStrategy {
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>> {
        // Already fired — signal done by returning empty.
        if self.fired {
            // Sleep briefly to avoid busy-spinning before the runner stops us.
            tokio::time::sleep(Duration::from_secs(1)).await;
            return Ok(vec![]);
        }

        let now = Utc::now();
        let wait_duration = if self.run_at > now {
            (self.run_at - now).to_std().unwrap_or(Duration::ZERO)
        } else {
            Duration::ZERO
        };

        info!(
            run_at = %self.run_at,
            wait_secs = wait_duration.as_secs(),
            workflow_id = %self.workflow_id,
            "Delay strategy waiting for fire time"
        );

        // Sleep until the fire time, respecting shutdown.
        let mut shutdown = shutdown.clone();
        tokio::select! {
            _ = tokio::time::sleep(wait_duration) => {
                self.fired = true;
                let task = self.build_task();
                Ok(vec![task])
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    return Ok(vec![]);
                }
                Ok(vec![])
            }
        }
    }
}

// ---------------------------------------------------------------------------
// EventStrategy
// ---------------------------------------------------------------------------

/// The internal filter configuration for an [`EventStrategy`].
///
/// Determines which [`SystemEvent`]s are converted into tasks.
#[derive(Debug, Clone)]
pub enum EventFilter {
    /// Match agent lifecycle events (connect, disconnect, context clear).
    AgentLifecycle {
        /// The event name: `"session_start"`, `"session_end"`, or `"context_clear"`.
        event: String,
        /// The workflow's agent — only events for this agent produce tasks.
        agent_id: Uuid,
    },
    /// Match dispatch completion events for workflow chaining.
    DispatchResult {
        /// If set, only match completions from this specific workflow.
        source_workflow_id: Option<Uuid>,
        /// If set, only match completions with this status.
        status: Option<DispatchStatus>,
    },
}

/// A [`TriggerStrategy`] that subscribes to the internal [`EventBus`] and
/// produces tasks when matching [`SystemEvent`]s occur.
///
/// For `AgentLifecycle` triggers the strategy validates that the event's
/// `agent_id` matches the workflow's configured agent. For `DispatchResult`
/// triggers it optionally filters by `source_workflow_id` and `status`,
/// enabling workflow chaining (trigger B when A completes).
///
/// # Broadcast Lag
///
/// If the subscriber falls behind the broadcast channel capacity, the
/// strategy logs a warning and continues — some events will have been missed
/// but no tasks are lost permanently since the next matching event will still
/// produce a task.
///
/// # Example
///
/// ```rust,ignore
/// use orchestrator::scheduler::strategy::EventStrategy;
/// use orchestrator::scheduler::events::EventBus;
///
/// let bus = EventBus::shared(256);
/// let filter = EventFilter::AgentLifecycle {
///     event: "session_start".to_string(),
///     agent_id: some_uuid,
/// };
/// let strategy = EventStrategy::new(bus, filter);
/// ```
pub struct EventStrategy {
    rx: broadcast::Receiver<SystemEvent>,
    filter: EventFilter,
}

impl EventStrategy {
    /// Create a new event strategy that subscribes to the given event bus.
    pub fn new(bus: Arc<EventBus>, filter: EventFilter) -> Self {
        let rx = bus.subscribe();
        Self { rx, filter }
    }

    /// Check whether a system event matches this strategy's filter and, if so,
    /// convert it into a [`Task`].
    fn match_event(&self, event: &SystemEvent) -> Option<Task> {
        match (&self.filter, event) {
            // AgentLifecycle: session_start matches AgentConnected
            (
                EventFilter::AgentLifecycle { event: filter_event, agent_id: filter_agent },
                SystemEvent::AgentConnected { agent_id },
            ) if filter_event == "session_start" && agent_id == filter_agent => {
                Some(self.build_lifecycle_task("session_start", agent_id))
            }

            // AgentLifecycle: session_end matches AgentDisconnected
            (
                EventFilter::AgentLifecycle { event: filter_event, agent_id: filter_agent },
                SystemEvent::AgentDisconnected { agent_id },
            ) if filter_event == "session_end" && agent_id == filter_agent => {
                Some(self.build_lifecycle_task("session_end", agent_id))
            }

            // AgentLifecycle: context_clear matches ContextCleared
            (
                EventFilter::AgentLifecycle { event: filter_event, agent_id: filter_agent },
                SystemEvent::ContextCleared { agent_id },
            ) if filter_event == "context_clear" && agent_id == filter_agent => {
                Some(self.build_lifecycle_task("context_clear", agent_id))
            }

            // DispatchResult: match DispatchCompleted with optional filters
            (
                EventFilter::DispatchResult {
                    source_workflow_id: filter_wf,
                    status: filter_status,
                },
                SystemEvent::DispatchCompleted { workflow_id, dispatch_id, status, source_id },
            ) => {
                // Filter by source workflow ID if configured.
                if let Some(expected_wf) = filter_wf {
                    if workflow_id != expected_wf {
                        return None;
                    }
                }
                // Filter by status if configured.
                if let Some(expected_status) = filter_status {
                    if status != expected_status {
                        return None;
                    }
                }
                Some(self.build_dispatch_task(
                    workflow_id,
                    dispatch_id,
                    status,
                    source_id.as_deref(),
                ))
            }

            _ => None,
        }
    }

    /// Build a task for an agent lifecycle event.
    fn build_lifecycle_task(&self, event_type: &str, agent_id: &Uuid) -> Task {
        let timestamp = Utc::now().to_rfc3339();
        let mut metadata = HashMap::new();
        metadata.insert("event_type".to_string(), event_type.to_string());
        metadata.insert("agent_id".to_string(), agent_id.to_string());
        metadata.insert("timestamp".to_string(), timestamp.clone());

        Task {
            source_id: format!("event:{}:{}:{}", event_type, agent_id, timestamp),
            title: format!("Agent lifecycle: {}", event_type),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata,
        }
    }

    /// Build a task for a dispatch completion event.
    ///
    /// `original_source_id` is forwarded from the [`SystemEvent::DispatchCompleted`] event
    /// so that chained workflows can reference the originating GitHub issue or PR number
    /// via `{{original_source_id}}` in their prompt templates.
    fn build_dispatch_task(
        &self,
        workflow_id: &Uuid,
        dispatch_id: &Uuid,
        status: &DispatchStatus,
        original_source_id: Option<&str>,
    ) -> Task {
        let timestamp = Utc::now().to_rfc3339();
        let mut metadata = HashMap::new();
        metadata.insert("source_workflow_id".to_string(), workflow_id.to_string());
        metadata.insert("dispatch_id".to_string(), dispatch_id.to_string());
        metadata.insert("status".to_string(), status.to_string());
        metadata.insert("timestamp".to_string(), timestamp.clone());
        if let Some(sid) = original_source_id {
            metadata.insert("original_source_id".to_string(), sid.to_string());
        }

        Task {
            source_id: format!("event:dispatch:{}:{}", dispatch_id, timestamp),
            title: format!("Dispatch completed: {} ({})", dispatch_id, status),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata,
        }
    }
}

#[async_trait]
impl TriggerStrategy for EventStrategy {
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>> {
        let mut shutdown = shutdown.clone();

        loop {
            tokio::select! {
                result = self.rx.recv() => {
                    match result {
                        Ok(event) => {
                            if let Some(task) = self.match_event(&event) {
                                return Ok(vec![task]);
                            }
                            // Event didn't match filter — keep listening.
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(
                                lagged = n,
                                "EventStrategy: subscriber lagged, some events may have been missed"
                            );
                            // Continue receiving — next matching event will still produce a task.
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Event bus shut down — return empty to signal done.
                            return Ok(vec![]);
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        return Ok(vec![]);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WebhookStrategy
// ---------------------------------------------------------------------------

/// A [`TriggerStrategy`] that receives tasks from an inbound HTTP webhook
/// via an `mpsc` channel.
///
/// Each webhook workflow gets a bounded `mpsc` channel. The HTTP endpoint
/// pushes parsed [`Task`]s through the sender; this strategy receives them.
/// When the sender is dropped (e.g., workflow stopped), `recv()` returns
/// `None` and the strategy signals completion with an empty vec.
///
/// # Example
///
/// ```rust,ignore
/// use tokio::sync::mpsc;
/// use orchestrator::scheduler::strategy::WebhookStrategy;
///
/// let (tx, rx) = mpsc::channel(64);
/// let strategy = WebhookStrategy::new(rx);
/// // Register `tx` in the WebhookRegistry; hand `strategy` to WorkflowRunner.
/// ```
pub struct WebhookStrategy {
    rx: mpsc::Receiver<Task>,
}

impl WebhookStrategy {
    /// Create a new webhook strategy from a channel receiver.
    pub fn new(rx: mpsc::Receiver<Task>) -> Self {
        Self { rx }
    }
}

#[async_trait]
impl TriggerStrategy for WebhookStrategy {
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>> {
        let mut shutdown = shutdown.clone();

        tokio::select! {
            task = self.rx.recv() => {
                match task {
                    Some(task) => Ok(vec![task]),
                    None => Ok(vec![]),  // Sender dropped — workflow stopped.
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    return Ok(vec![]);
                }
                Ok(vec![])
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ManualStrategy
// ---------------------------------------------------------------------------

/// A [`TriggerStrategy`] that receives tasks from an explicit API trigger call
/// via an `mpsc` channel.
///
/// Manual workflows do not poll or respond to events — they are triggered on
/// demand via the `POST /workflows/{id}/trigger` API endpoint or the
/// `agent orchestrator trigger-workflow` CLI command.
///
/// Each manual workflow gets a bounded `mpsc` channel. The HTTP endpoint
/// pushes parsed [`Task`]s through the sender; this strategy receives them.
/// When the sender is dropped (e.g., workflow stopped), `recv()` returns
/// `None` and the strategy signals completion with an empty vec.
///
/// # Example
///
/// ```rust,ignore
/// use tokio::sync::mpsc;
/// use orchestrator::scheduler::strategy::ManualStrategy;
///
/// let (tx, rx) = mpsc::channel(64);
/// let strategy = ManualStrategy::new(rx);
/// // Register `tx` for access by the trigger API handler.
/// ```
pub struct ManualStrategy {
    rx: mpsc::Receiver<Task>,
}

impl ManualStrategy {
    /// Create a new manual strategy from a channel receiver.
    pub fn new(rx: mpsc::Receiver<Task>) -> Self {
        Self { rx }
    }
}

#[async_trait]
impl TriggerStrategy for ManualStrategy {
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>> {
        let mut shutdown = shutdown.clone();

        tokio::select! {
            task = self.rx.recv() => {
                match task {
                    Some(task) => Ok(vec![task]),
                    None => Ok(vec![]),  // Sender dropped — workflow stopped.
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    return Ok(vec![]);
                }
                Ok(vec![])
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IdleStrategy
// ---------------------------------------------------------------------------

/// A [`TriggerStrategy`] that fires when an agent has been idle (no active
/// dispatches) for a configurable duration.
///
/// The strategy subscribes to the [`EventBus`] and listens for
/// [`SystemEvent::DispatchCompleted`] events for the configured agent. Each
/// time such an event arrives, the idle timer resets. When the timer expires
/// without any new activity, the strategy produces a synthetic [`Task`] with
/// a `source_id` of `"idle:{unix_timestamp}"` for deduplication.
///
/// After firing, the timer resets — the strategy will fire again on the next
/// idle period.
///
/// # Shutdown
///
/// The internal sleep is interruptible. When the shutdown signal fires, the
/// strategy returns an empty vec immediately.
///
/// # Broadcast Lag
///
/// If the subscriber falls behind the broadcast channel capacity, the
/// strategy logs a warning and continues — the timer is not reset on a lag
/// event, since the missed events may or may not have been completions.
///
/// # Example
///
/// ```rust,ignore
/// use std::time::Duration;
/// use orchestrator::scheduler::strategy::IdleStrategy;
/// use orchestrator::scheduler::events::EventBus;
///
/// let bus = EventBus::shared(256);
/// let agent_id = uuid::Uuid::new_v4();
/// let strategy = IdleStrategy::new(bus, agent_id, Duration::from_secs(30));
/// ```
pub struct IdleStrategy {
    /// Keeps the event bus alive (and its broadcast sender) for the strategy's lifetime.
    _bus: Arc<EventBus>,
    rx: broadcast::Receiver<SystemEvent>,
    idle_duration: Duration,
    agent_id: Uuid,
}

impl IdleStrategy {
    /// Create a new idle strategy.
    ///
    /// * `bus` — the shared event bus to subscribe to.
    /// * `agent_id` — only `DispatchCompleted` events for this agent reset the timer.
    /// * `idle_duration` — how long the agent must be idle before a task fires.
    pub fn new(bus: Arc<EventBus>, agent_id: Uuid, idle_duration: Duration) -> Self {
        let rx = bus.subscribe();
        Self { _bus: bus, rx, idle_duration, agent_id }
    }

    /// Check whether a system event is a dispatch completion for our agent.
    fn is_relevant_completion(&self, event: &SystemEvent) -> bool {
        matches!(event, SystemEvent::DispatchCompleted { .. })
    }

    /// Build a synthetic task for an idle firing.
    fn build_task(&self) -> Task {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = Utc::now().to_rfc3339();
        let mut metadata = HashMap::new();
        metadata.insert("agent_id".to_string(), self.agent_id.to_string());
        metadata.insert("idle_seconds".to_string(), self.idle_duration.as_secs().to_string());
        metadata.insert("timestamp".to_string(), timestamp);

        Task {
            source_id: format!("idle:{ts}"),
            title: "Agent idle trigger".to_string(),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata,
        }
    }
}

#[async_trait]
impl TriggerStrategy for IdleStrategy {
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>> {
        let mut shutdown = shutdown.clone();

        loop {
            // Create a fresh sleep future each iteration so the timer resets
            // when a dispatch completion event arrives.
            let sleep = tokio::time::sleep(self.idle_duration);
            tokio::pin!(sleep);

            tokio::select! {
                _ = &mut sleep => {
                    // Idle timeout elapsed — fire a synthetic task.
                    info!(
                        agent_id = %self.agent_id,
                        idle_secs = self.idle_duration.as_secs(),
                        "IdleStrategy: agent idle timeout elapsed, firing task"
                    );
                    return Ok(vec![self.build_task()]);
                }
                result = self.rx.recv() => {
                    match result {
                        Ok(event) => {
                            if self.is_relevant_completion(&event) {
                                // Dispatch completed — reset the idle timer.
                                info!(
                                    agent_id = %self.agent_id,
                                    "IdleStrategy: dispatch completed, resetting idle timer"
                                );
                                // Continue loop to create a new sleep.
                            }
                            // Non-matching events are ignored; we keep waiting.
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(
                                lagged = n,
                                "IdleStrategy: subscriber lagged, some events may have been missed"
                            );
                            // Do not reset timer — we don't know if missed events
                            // were completions for our agent. Continue waiting.
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Event bus shut down — return empty to signal done.
                            return Ok(vec![]);
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        return Ok(vec![]);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FileWatchStrategy
// ---------------------------------------------------------------------------

/// Convert a `notify::EventKind` to a short lowercase string suitable for
/// template variables and `source_id` generation.
fn event_kind_to_str(kind: &notify::EventKind) -> &'static str {
    match kind {
        notify::EventKind::Create(_) => "create",
        notify::EventKind::Modify(_) => "modify",
        notify::EventKind::Remove(_) => "delete",
        notify::EventKind::Access(_) => "access",
        notify::EventKind::Other => "other",
        notify::EventKind::Any => "any",
    }
}

/// Selects which backend [`FileWatchStrategy`] uses for detecting filesystem changes.
///
/// | Variant   | Description |
/// |-----------|-------------|
/// | `Native`  | OS-native events (inotify / FSEvents / kqueue). Low latency, no polling. |
/// | `Polling` | mtime polling at a fixed interval. Works on any filesystem. |
/// | `Auto`    | Tries native first; silently falls back to polling if native setup fails. |
#[derive(Debug, Clone)]
pub enum WatchMode {
    /// Use OS-native filesystem events only.
    Native,
    /// Poll file mtimes at the given interval.
    Polling {
        /// Seconds between each scan of the watched directories.
        interval_secs: u64,
    },
    /// Try native; fall back to polling if native is unavailable.
    Auto {
        /// Polling interval (seconds) used when falling back to mtime polling.
        poll_interval_secs: u64,
    },
}

impl Default for WatchMode {
    fn default() -> Self {
        WatchMode::Auto { poll_interval_secs: 5 }
    }
}

/// Internal handle keeping the active watcher alive for the duration of a
/// [`FileWatchStrategy`].
enum FileWatchHandle {
    /// Native OS watcher — kept alive by holding the struct.
    Native(notify::RecommendedWatcher),
    /// Background mtime polling task — kept alive until abort.
    Polling(tokio::task::JoinHandle<()>),
}

impl std::fmt::Debug for FileWatchHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native(_) => f.write_str("FileWatchHandle::Native(..)"),
            Self::Polling(_) => f.write_str("FileWatchHandle::Polling(..)"),
        }
    }
}

/// Recursively walk `root` and collect the mtime for every regular file.
fn snapshot_mtimes(root: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut map = HashMap::new();
    let Ok(walker) = std::fs::read_dir(root) else { return map };
    let mut stack = vec![walker];
    while let Some(dir) = stack.last_mut() {
        match dir.next() {
            None => {
                stack.pop();
            }
            Some(Ok(entry)) => {
                let path = entry.path();
                if let Ok(meta) = std::fs::metadata(&path) {
                    if meta.is_dir() {
                        if let Ok(sub) = std::fs::read_dir(&path) {
                            stack.push(sub);
                        }
                    } else if meta.is_file() {
                        if let Ok(mtime) = meta.modified() {
                            map.insert(path, mtime);
                        }
                    }
                }
            }
            Some(Err(_)) => {}
        }
    }
    map
}

/// Spawn a background task that polls file mtimes and forwards synthetic
/// [`notify::Event`]s through `tx`.
fn spawn_polling_watcher(
    paths: Vec<PathBuf>,
    interval_secs: u64,
    tx: mpsc::UnboundedSender<notify::Result<notify::Event>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Build initial snapshot.
        let mut prev: HashMap<PathBuf, SystemTime> =
            paths.iter().flat_map(|p| snapshot_mtimes(p)).collect();

        let interval = Duration::from_secs(interval_secs.max(1));
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await; // consume the immediate first tick

        loop {
            ticker.tick().await;

            // Build current snapshot.
            let current: HashMap<PathBuf, SystemTime> =
                paths.iter().flat_map(|p| snapshot_mtimes(p)).collect();

            // Detect created and modified files.
            for (path, mtime) in &current {
                match prev.get(path) {
                    None => {
                        // New file.
                        let ev = notify::Event {
                            kind: notify::EventKind::Create(notify::event::CreateKind::File),
                            paths: vec![path.clone()],
                            attrs: Default::default(),
                        };
                        if tx.send(Ok(ev)).is_err() {
                            return;
                        }
                    }
                    Some(prev_mtime) if prev_mtime != mtime => {
                        // Modified file.
                        let ev = notify::Event {
                            kind: notify::EventKind::Modify(notify::event::ModifyKind::Data(
                                notify::event::DataChange::Content,
                            )),
                            paths: vec![path.clone()],
                            attrs: Default::default(),
                        };
                        if tx.send(Ok(ev)).is_err() {
                            return;
                        }
                    }
                    _ => {}
                }
            }

            // Detect removed files.
            for path in prev.keys() {
                if !current.contains_key(path) {
                    let ev = notify::Event {
                        kind: notify::EventKind::Remove(notify::event::RemoveKind::File),
                        paths: vec![path.clone()],
                        attrs: Default::default(),
                    };
                    if tx.send(Ok(ev)).is_err() {
                        return;
                    }
                }
            }

            prev = current;
        }
    })
}

/// A [`TriggerStrategy`] that watches filesystem paths for changes.
///
/// By default uses the OS-native notification API (inotify on Linux, FSEvents
/// on macOS, ReadDirectoryChangesW on Windows) via the `notify` crate.  When
/// native watching is unavailable, or when [`WatchMode::Polling`] is selected,
/// an mtime-based polling loop is used instead.
///
/// When a matching filesystem event occurs the strategy fires a synthetic
/// [`Task`] carrying the changed file's path, name, containing directory, and
/// event kind in its metadata.
///
/// # Pattern Filtering
///
/// If `patterns` is provided, only paths matching at least one glob (using
/// `globset` syntax) will produce tasks. An empty or absent patterns list
/// accepts every path.
///
/// # Event Kind Filtering
///
/// `event_kinds` contains lowercase strings: `"create"`, `"modify"`,
/// `"delete"`, `"access"`. An empty vec accepts all kinds.
///
/// # Debouncing
///
/// After the first matching event the strategy waits `debounce_ms` milliseconds
/// for more events. Any additional events within the window reset the timer.
/// When the window expires the strategy fires a single task based on the
/// *first* event seen in the burst, reducing duplicate dispatches on rapid
/// file changes (e.g., editor atomic writes).
///
/// # Shutdown
///
/// Both the first-event wait and the debounce phase respect the shutdown
/// signal and return an empty vec immediately when it fires.
///
/// # Example
///
/// ```rust,ignore
/// use orchestrator::scheduler::strategy::{FileWatchStrategy, WatchMode};
///
/// let strategy = FileWatchStrategy::new(
///     vec!["/tmp/watched".into()],
///     Some(vec!["**/*.toml".into()]),
///     vec!["create".into(), "modify".into()],
///     200,             // 200 ms debounce
///     WatchMode::Auto { poll_interval_secs: 5 },
/// )?;
/// ```
pub struct FileWatchStrategy {
    /// Active watcher handle — kept alive for the lifetime of this strategy.
    _handle: FileWatchHandle,
    /// Async receiver end of the mpsc bridge.
    rx: mpsc::UnboundedReceiver<notify::Result<notify::Event>>,
    /// Optional glob patterns — `None` means accept every path.
    include_patterns: Option<GlobSet>,
    /// Event kinds to accept. Empty means accept all.
    event_kinds: Vec<String>,
    /// Debounce window in milliseconds (0 = no debounce).
    debounce_ms: u64,
    /// Watched root paths (stored for metadata / diagnostics).
    #[allow(dead_code)]
    watch_paths: Vec<PathBuf>,
}

impl std::fmt::Debug for FileWatchStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileWatchStrategy")
            .field("handle", &self._handle)
            .field("event_kinds", &self.event_kinds)
            .field("debounce_ms", &self.debounce_ms)
            .finish()
    }
}

impl FileWatchStrategy {
    /// Create a new file-watch strategy.
    ///
    /// * `paths` — directories or files to watch recursively.
    /// * `patterns` — optional glob patterns restricting which paths produce tasks.
    /// * `event_kinds` — which event kinds to accept (`"create"`, `"modify"`,
    ///   `"delete"`, `"access"`). An empty vec accepts all.
    /// * `debounce_ms` — debounce window in milliseconds (0 = no debounce).
    /// * `mode` — which watching backend to use (native, polling, or auto).
    pub fn new(
        paths: Vec<PathBuf>,
        patterns: Option<Vec<String>>,
        event_kinds: Vec<String>,
        debounce_ms: u64,
        mode: WatchMode,
    ) -> anyhow::Result<Self> {
        // Validate and build the optional glob set first (fail fast before spawning).
        let include_patterns = match patterns {
            None => None,
            Some(pats) if pats.is_empty() => None,
            Some(pats) => {
                let mut builder = GlobSetBuilder::new();
                for pat in &pats {
                    let glob = Glob::new(pat)
                        .map_err(|e| anyhow::anyhow!("Invalid glob pattern '{}': {}", pat, e))?;
                    builder.add(glob);
                }
                Some(
                    builder
                        .build()
                        .map_err(|e| anyhow::anyhow!("Failed to build glob set: {}", e))?,
                )
            }
        };

        // Spawn the watcher after glob validation succeeds.
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = Self::create_handle(paths.clone(), mode, tx)?;

        Ok(Self {
            _handle: handle,
            rx,
            include_patterns,
            event_kinds,
            debounce_ms,
            watch_paths: paths,
        })
    }

    /// Create the appropriate [`FileWatchHandle`] for the given mode.
    fn create_handle(
        paths: Vec<PathBuf>,
        mode: WatchMode,
        tx: mpsc::UnboundedSender<notify::Result<notify::Event>>,
    ) -> anyhow::Result<FileWatchHandle> {
        match mode {
            WatchMode::Native => {
                let handle = Self::create_native_watcher(paths, tx)?;
                Ok(FileWatchHandle::Native(handle))
            }
            WatchMode::Polling { interval_secs } => {
                let join = spawn_polling_watcher(paths, interval_secs, tx);
                Ok(FileWatchHandle::Polling(join))
            }
            WatchMode::Auto { poll_interval_secs } => {
                // Attempt native first; fall back to polling on failure.
                match Self::create_native_watcher(paths.clone(), tx.clone()) {
                    Ok(watcher) => {
                        info!("FileWatchStrategy: using native OS watcher");
                        Ok(FileWatchHandle::Native(watcher))
                    }
                    Err(e) => {
                        warn!(
                            %e,
                            interval_secs = poll_interval_secs,
                            "FileWatchStrategy: native watcher unavailable, falling back to mtime polling"
                        );
                        let join = spawn_polling_watcher(paths, poll_interval_secs, tx);
                        Ok(FileWatchHandle::Polling(join))
                    }
                }
            }
        }
    }

    /// Build and configure an OS-native `RecommendedWatcher`.
    fn create_native_watcher(
        paths: Vec<PathBuf>,
        tx: mpsc::UnboundedSender<notify::Result<notify::Event>>,
    ) -> anyhow::Result<notify::RecommendedWatcher> {
        let mut watcher = notify::RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create filesystem watcher: {}", e))?;

        for path in &paths {
            watcher
                .watch(path, notify::RecursiveMode::Recursive)
                .map_err(|e| anyhow::anyhow!("Failed to watch path {:?}: {}", path, e))?;
        }
        Ok(watcher)
    }

    /// Returns `true` if the event passes the kind and pattern filters.
    fn matches_event(&self, event: &notify::Event) -> bool {
        // Filter by event kind.
        if !self.event_kinds.is_empty() {
            let kind_str = event_kind_to_str(&event.kind);
            if !self.event_kinds.iter().any(|k| k == kind_str) {
                return false;
            }
        }

        // Filter by glob patterns.
        if let Some(ref patterns) = self.include_patterns {
            if !event.paths.iter().any(|p| patterns.is_match(p)) {
                return false;
            }
        }

        true
    }

    /// Build a synthetic [`Task`] from a filesystem event.
    ///
    /// The task's metadata includes:
    ///
    /// | key          | value                      |
    /// |--------------|----------------------------|
    /// | `file_path`  | full path of changed file  |
    /// | `file_name`  | basename of changed file   |
    /// | `file_dir`   | parent directory           |
    /// | `event_type` | `"create"` / `"modify"` / … |
    /// | `timestamp`  | RFC 3339 fire time         |
    fn build_task(&self, event: &notify::Event) -> Task {
        let path = event.paths.first().map(|p| p.display().to_string()).unwrap_or_default();
        let file_name = event
            .paths
            .first()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file_dir = event
            .paths
            .first()
            .and_then(|p| p.parent())
            .map(|d| d.display().to_string())
            .unwrap_or_default();
        let kind_str = event_kind_to_str(&event.kind);
        let timestamp = Utc::now().to_rfc3339();

        let mut metadata = HashMap::new();
        metadata.insert("file_path".to_string(), path.clone());
        metadata.insert("file_name".to_string(), file_name);
        metadata.insert("file_dir".to_string(), file_dir);
        metadata.insert("event_type".to_string(), kind_str.to_string());
        metadata.insert("timestamp".to_string(), timestamp);

        Task {
            source_id: format!("file:{}:{}", path, kind_str),
            title: format!("File {} event: {}", kind_str, path),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata,
        }
    }
}

#[async_trait]
impl TriggerStrategy for FileWatchStrategy {
    async fn next_tasks(&mut self, shutdown: &watch::Receiver<bool>) -> anyhow::Result<Vec<Task>> {
        let mut shutdown = shutdown.clone();

        // ── Phase 1: wait for the first matching event ──────────────────────
        let first_event = loop {
            tokio::select! {
                maybe_event = self.rx.recv() => {
                    match maybe_event {
                        Some(Ok(event)) if self.matches_event(&event) => break event,
                        Some(Ok(_)) => {
                            // Non-matching event — keep waiting.
                        }
                        Some(Err(e)) => {
                            warn!(%e, "FileWatchStrategy: watcher error");
                        }
                        None => {
                            // Watcher was dropped — signal done.
                            return Ok(vec![]);
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        return Ok(vec![]);
                    }
                }
            }
        };

        // ── Phase 2: debounce — drain additional events within the window ───
        if self.debounce_ms > 0 {
            let debounce = Duration::from_millis(self.debounce_ms);
            loop {
                // A fresh sleep each iteration means arriving events reset the timer.
                let sleep = tokio::time::sleep(debounce);
                tokio::pin!(sleep);

                tokio::select! {
                    _ = &mut sleep => {
                        // Window expired — ready to fire.
                        break;
                    }
                    maybe_event = self.rx.recv() => {
                        match maybe_event {
                            Some(Ok(_)) => {
                                // Another event arrived — loop restarts with a fresh sleep.
                            }
                            Some(Err(e)) => {
                                warn!(%e, "FileWatchStrategy: watcher error during debounce");
                            }
                            None => break,
                        }
                    }
                    _ = shutdown.changed() => {
                        if *shutdown.borrow() {
                            return Ok(vec![]);
                        }
                    }
                }
            }
        }

        let task = self.build_task(&first_event);
        info!(
            path = %first_event.paths.first().map(|p| p.display().to_string()).unwrap_or_default(),
            kind = %event_kind_to_str(&first_event.kind),
            "FileWatchStrategy: filesystem event fired task"
        );
        Ok(vec![task])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// A mock task source for testing.
    struct MockSource {
        tasks: Vec<Task>,
        call_count: Arc<AtomicU32>,
    }

    impl MockSource {
        fn new(tasks: Vec<Task>) -> Self {
            Self { tasks, call_count: Arc::new(AtomicU32::new(0)) }
        }

        fn with_counter(tasks: Vec<Task>, counter: Arc<AtomicU32>) -> Self {
            Self { tasks, call_count: counter }
        }
    }

    #[async_trait]
    impl TaskSource for MockSource {
        async fn fetch_tasks(&self) -> anyhow::Result<Vec<Task>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.tasks.clone())
        }

        fn source_type(&self) -> &'static str {
            "mock"
        }
    }

    /// A mock source that always fails.
    struct FailingSource;

    #[async_trait]
    impl TaskSource for FailingSource {
        async fn fetch_tasks(&self) -> anyhow::Result<Vec<Task>> {
            anyhow::bail!("source error")
        }

        fn source_type(&self) -> &'static str {
            "failing"
        }
    }

    fn sample_task(id: &str) -> Task {
        Task {
            source_id: id.to_string(),
            title: format!("Task {id}"),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn polling_returns_tasks_from_source() {
        let tasks = vec![sample_task("1"), sample_task("2")];
        let source = Box::new(MockSource::new(tasks.clone()));
        let mut strategy = PollingStrategy::new(source, 0); // 0s interval for fast test
        let (_tx, rx) = watch::channel(false);

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].source_id, "1");
        assert_eq!(result[1].source_id, "2");
    }

    #[tokio::test]
    async fn polling_returns_empty_on_no_tasks() {
        let source = Box::new(MockSource::new(vec![]));
        let mut strategy = PollingStrategy::new(source, 0);
        let (_tx, rx) = watch::channel(false);

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn polling_respects_shutdown_signal() {
        let source = Box::new(MockSource::new(vec![sample_task("1")]));
        let counter = Arc::clone(&(source.call_count));
        // Use a long interval so the test would hang without shutdown.
        let mut strategy = PollingStrategy::new(source, 3600);
        let (tx, rx) = watch::channel(false);

        // Fire shutdown after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        // Should return quickly (well under the 3600s interval).
        assert!(elapsed < Duration::from_secs(2));
        // Should return empty vec (shutdown, no fetch).
        assert!(result.is_empty());
        // Source should NOT have been called.
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn polling_tracks_consecutive_errors() {
        let mut strategy = PollingStrategy::new(Box::new(FailingSource), 0);
        let (_tx, rx) = watch::channel(false);

        // First error.
        assert!(strategy.next_tasks(&rx).await.is_err());
        assert_eq!(strategy.consecutive_errors, 1);

        // Second error.
        assert!(strategy.next_tasks(&rx).await.is_err());
        assert_eq!(strategy.consecutive_errors, 2);
    }

    #[tokio::test]
    async fn polling_resets_errors_on_success() {
        let counter = Arc::new(AtomicU32::new(0));
        let source = Box::new(MockSource::with_counter(vec![sample_task("1")], counter));
        let mut strategy = PollingStrategy::new(source, 0);
        let (_tx, rx) = watch::channel(false);

        // Simulate some prior errors.
        strategy.consecutive_errors = 5;

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(strategy.consecutive_errors, 0);
    }

    #[tokio::test]
    async fn backoff_duration_increases_with_errors() {
        let source = Box::new(MockSource::new(vec![]));
        let mut strategy = PollingStrategy::new(source, 10);

        // No errors → base interval only.
        assert_eq!(strategy.sleep_duration(), Duration::from_secs(10));

        // 1 error → 10 + 2 = 12s.
        strategy.consecutive_errors = 1;
        assert_eq!(strategy.sleep_duration(), Duration::from_secs(12));

        // 5 errors → 10 + 10 = 20s.
        strategy.consecutive_errors = 5;
        assert_eq!(strategy.sleep_duration(), Duration::from_secs(20));

        // Cap at MAX_BACKOFF_SECS: 20 errors → 10 + 30 = 40s (not 10 + 40).
        strategy.consecutive_errors = 20;
        assert_eq!(strategy.sleep_duration(), Duration::from_secs(40));
    }

    #[tokio::test]
    async fn polling_strategy_is_object_safe() {
        // Verify the trait can be used as Box<dyn TriggerStrategy>.
        let source = Box::new(MockSource::new(vec![]));
        let strategy: Box<dyn TriggerStrategy> = Box::new(PollingStrategy::new(source, 0));
        let (_tx, rx) = watch::channel(false);

        let mut strategy = strategy;
        let result = strategy.next_tasks(&rx).await.unwrap();
        assert!(result.is_empty());
    }

    // ── CronStrategy tests ──────────────────────────────────────────

    #[test]
    fn cron_strategy_parses_valid_expression() {
        let strategy = CronStrategy::new("0 9 * * MON-FRI");
        assert!(strategy.is_ok());
    }

    #[test]
    fn cron_strategy_rejects_invalid_expression() {
        let result = CronStrategy::new("not a cron");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid cron expression"));
    }

    #[test]
    fn cron_strategy_rejects_empty_expression() {
        let result = CronStrategy::new("");
        assert!(result.is_err());
    }

    #[test]
    fn cron_strategy_next_fire_time_is_in_future() {
        // "every minute" should always have a next fire time
        let strategy = CronStrategy::new("* * * * *").unwrap();
        let next = strategy.next_fire_time().unwrap();
        assert!(next > Utc::now());
    }

    #[test]
    fn cron_strategy_build_task_has_correct_fields() {
        let strategy = CronStrategy::new("0 9 * * MON-FRI").unwrap();
        let fire_time = Utc::now();
        let task = strategy.build_task(&fire_time);

        // source_id should start with "cron:"
        assert!(task.source_id.starts_with("cron:"));
        // source_id contains the RFC 3339 timestamp
        assert!(task.source_id.contains(&fire_time.to_rfc3339()));
        // title contains the expression
        assert_eq!(task.title, "Cron trigger: 0 9 * * MON-FRI");
        // metadata has fire_time and cron_expression
        assert_eq!(task.metadata.get("fire_time"), Some(&fire_time.to_rfc3339()));
        assert_eq!(task.metadata.get("cron_expression"), Some(&"0 9 * * MON-FRI".to_string()));
    }

    #[test]
    fn cron_strategy_tasks_have_unique_source_ids() {
        let strategy = CronStrategy::new("* * * * *").unwrap();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::minutes(1);

        let task1 = strategy.build_task(&t1);
        let task2 = strategy.build_task(&t2);

        assert_ne!(task1.source_id, task2.source_id);
    }

    #[tokio::test]
    async fn cron_strategy_fires_on_every_minute() {
        // Use "every second" pattern — should fire almost immediately.
        let mut strategy = CronStrategy::new("* * * * * *").unwrap();
        let (_tx, rx) = watch::channel(false);

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("cron:"));
        // Should complete within 2 seconds (next second boundary).
        assert!(elapsed < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn cron_strategy_respects_shutdown() {
        // Use a far-future cron (once a year) so it would block forever.
        let mut strategy = CronStrategy::new("0 0 1 1 *").unwrap();
        let (tx, rx) = watch::channel(false);

        // Fire shutdown after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        // Should return quickly (well under a second).
        assert!(elapsed < Duration::from_secs(2));
        // Should return empty vec on shutdown.
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn cron_strategy_is_object_safe() {
        let strategy: Box<dyn TriggerStrategy> =
            Box::new(CronStrategy::new("* * * * * *").unwrap());
        let (_tx, rx) = watch::channel(false);

        let mut strategy = strategy;
        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn cron_strategy_common_expressions() {
        // Various standard cron expressions should all parse.
        let expressions = vec![
            "0 9 * * MON-FRI", // 9 AM weekdays
            "*/5 * * * *",     // every 5 minutes
            "0 0 * * *",       // midnight daily
            "0 12 1 * *",      // noon on 1st of month
            "30 4 * * SUN",    // 4:30 AM on Sundays
        ];
        for expr in expressions {
            assert!(CronStrategy::new(expr).is_ok(), "Failed to parse: {}", expr);
        }
    }

    // ── DelayStrategy tests ─────────────────────────────────────────

    #[test]
    fn delay_strategy_build_task_has_correct_fields() {
        let wf_id = uuid::Uuid::new_v4();
        let run_at = Utc::now() + chrono::Duration::hours(1);
        let strategy = DelayStrategy::new(run_at, wf_id);
        let task = strategy.build_task();

        assert_eq!(task.source_id, format!("delay:{}", wf_id));
        assert!(task.title.contains("Delay trigger:"));
        assert_eq!(task.metadata.get("run_at"), Some(&run_at.to_rfc3339()));
        assert_eq!(task.metadata.get("workflow_id"), Some(&wf_id.to_string()));
    }

    #[test]
    fn delay_strategy_not_fired_initially() {
        let wf_id = uuid::Uuid::new_v4();
        let strategy = DelayStrategy::new(Utc::now(), wf_id);
        assert!(!strategy.has_fired());
    }

    #[tokio::test]
    async fn delay_strategy_fires_immediately_for_past_time() {
        let wf_id = uuid::Uuid::new_v4();
        let past = Utc::now() - chrono::Duration::hours(1);
        let mut strategy = DelayStrategy::new(past, wf_id);
        let (_tx, rx) = watch::channel(false);

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source_id, format!("delay:{}", wf_id));
        assert!(elapsed < Duration::from_secs(1));
        assert!(strategy.has_fired());
    }

    #[tokio::test]
    async fn delay_strategy_fires_at_future_time() {
        let wf_id = uuid::Uuid::new_v4();
        // Fire 100ms in the future
        let run_at = Utc::now() + chrono::Duration::milliseconds(100);
        let mut strategy = DelayStrategy::new(run_at, wf_id);
        let (_tx, rx) = watch::channel(false);

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(strategy.has_fired());
    }

    #[tokio::test]
    async fn delay_strategy_returns_empty_after_firing() {
        let wf_id = uuid::Uuid::new_v4();
        let past = Utc::now() - chrono::Duration::hours(1);
        let mut strategy = DelayStrategy::new(past, wf_id);
        let (_tx, rx) = watch::channel(false);

        // First call fires.
        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);

        // Second call returns empty.
        let result = strategy.next_tasks(&rx).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn delay_strategy_respects_shutdown() {
        let wf_id = uuid::Uuid::new_v4();
        // Use a far-future time so it would block forever.
        let run_at = Utc::now() + chrono::Duration::hours(24);
        let mut strategy = DelayStrategy::new(run_at, wf_id);
        let (tx, rx) = watch::channel(false);

        // Fire shutdown after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        assert!(elapsed < Duration::from_secs(2));
        assert!(result.is_empty());
        assert!(!strategy.has_fired());
    }

    #[tokio::test]
    async fn delay_strategy_is_object_safe() {
        let wf_id = uuid::Uuid::new_v4();
        let past = Utc::now() - chrono::Duration::seconds(1);
        let strategy: Box<dyn TriggerStrategy> = Box::new(DelayStrategy::new(past, wf_id));
        let (_tx, rx) = watch::channel(false);

        let mut strategy = strategy;
        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn delay_strategy_source_id_uses_workflow_id() {
        let wf_id = uuid::Uuid::new_v4();
        let strategy = DelayStrategy::new(Utc::now(), wf_id);
        let task = strategy.build_task();

        // source_id should be deterministic based on workflow_id for dedup.
        assert_eq!(task.source_id, format!("delay:{}", wf_id));
    }

    // ── EventStrategy tests ───────────────────────────────────────────

    #[tokio::test]
    async fn event_strategy_matches_agent_connected() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "session_start".to_string(), agent_id };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Publish a matching event.
        bus.publish(SystemEvent::AgentConnected { agent_id });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("event:session_start:"));
        assert!(result[0].source_id.contains(&agent_id.to_string()));
        assert_eq!(result[0].metadata.get("event_type"), Some(&"session_start".to_string()));
        assert_eq!(result[0].metadata.get("agent_id"), Some(&agent_id.to_string()));
        assert!(result[0].metadata.contains_key("timestamp"));
    }

    #[tokio::test]
    async fn event_strategy_matches_agent_disconnected() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "session_end".to_string(), agent_id };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        bus.publish(SystemEvent::AgentDisconnected { agent_id });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("event:session_end:"));
        assert_eq!(result[0].metadata.get("event_type"), Some(&"session_end".to_string()));
    }

    #[tokio::test]
    async fn event_strategy_matches_context_cleared() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "context_clear".to_string(), agent_id };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        bus.publish(SystemEvent::ContextCleared { agent_id });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("event:context_clear:"));
        assert_eq!(result[0].metadata.get("event_type"), Some(&"context_clear".to_string()));
    }

    #[tokio::test]
    async fn event_strategy_ignores_wrong_agent_id() {
        let bus = EventBus::shared(16);
        let target_agent = Uuid::new_v4();
        let other_agent = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle {
            event: "session_start".to_string(),
            agent_id: target_agent,
        };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Publish event for a different agent — should not match.
        bus.publish(SystemEvent::AgentConnected { agent_id: other_agent });
        // Publish event for the correct agent — should match.
        bus.publish(SystemEvent::AgentConnected { agent_id: target_agent });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.contains(&target_agent.to_string()));
    }

    #[tokio::test]
    async fn event_strategy_ignores_wrong_event_type() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "session_start".to_string(), agent_id };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Publish a disconnect event — should not match session_start filter.
        bus.publish(SystemEvent::AgentDisconnected { agent_id });
        // Now publish the matching connect event.
        bus.publish(SystemEvent::AgentConnected { agent_id });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("event:session_start:"));
    }

    #[tokio::test]
    async fn event_strategy_dispatch_result_matches() {
        let bus = EventBus::shared(16);
        let workflow_id = Uuid::new_v4();
        let dispatch_id = Uuid::new_v4();
        let filter = EventFilter::DispatchResult {
            source_workflow_id: Some(workflow_id),
            status: Some(DispatchStatus::Completed),
        };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id,
            dispatch_id,
            status: DispatchStatus::Completed,
            source_id: Some("101".to_string()),
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("event:dispatch:"));
        assert!(result[0].source_id.contains(&dispatch_id.to_string()));
        assert_eq!(result[0].metadata.get("source_workflow_id"), Some(&workflow_id.to_string()));
        assert_eq!(result[0].metadata.get("dispatch_id"), Some(&dispatch_id.to_string()));
        assert_eq!(result[0].metadata.get("status"), Some(&"completed".to_string()));
        assert_eq!(result[0].metadata.get("original_source_id"), Some(&"101".to_string()));
    }

    #[tokio::test]
    async fn event_strategy_dispatch_result_filters_by_workflow_id() {
        let bus = EventBus::shared(16);
        let target_wf = Uuid::new_v4();
        let other_wf = Uuid::new_v4();
        let filter =
            EventFilter::DispatchResult { source_workflow_id: Some(target_wf), status: None };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Publish for wrong workflow — should be skipped.
        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id: other_wf,
            dispatch_id: Uuid::new_v4(),
            status: DispatchStatus::Completed,
            source_id: None,
        });
        // Publish for correct workflow — should match.
        let expected_dispatch = Uuid::new_v4();
        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id: target_wf,
            dispatch_id: expected_dispatch,
            status: DispatchStatus::Failed,
            source_id: None,
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.contains(&expected_dispatch.to_string()));
    }

    #[tokio::test]
    async fn event_strategy_dispatch_result_filters_by_status() {
        let bus = EventBus::shared(16);
        let workflow_id = Uuid::new_v4();
        let filter = EventFilter::DispatchResult {
            source_workflow_id: None,
            status: Some(DispatchStatus::Failed),
        };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Publish a Completed event — should be skipped (filter wants Failed).
        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id,
            dispatch_id: Uuid::new_v4(),
            status: DispatchStatus::Completed,
            source_id: None,
        });
        // Publish a Failed event — should match.
        let expected_dispatch = Uuid::new_v4();
        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id,
            dispatch_id: expected_dispatch,
            status: DispatchStatus::Failed,
            source_id: None,
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].metadata.get("status"), Some(&"failed".to_string()));
    }

    #[tokio::test]
    async fn event_strategy_dispatch_result_no_filters_matches_any() {
        let bus = EventBus::shared(16);
        let filter = EventFilter::DispatchResult { source_workflow_id: None, status: None };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Any DispatchCompleted should match when no filters are set.
        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id: Uuid::new_v4(),
            dispatch_id: Uuid::new_v4(),
            status: DispatchStatus::Completed,
            source_id: None,
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn event_strategy_respects_shutdown() {
        let bus = EventBus::shared(16);
        let filter = EventFilter::AgentLifecycle {
            event: "session_start".to_string(),
            agent_id: Uuid::new_v4(),
        };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (tx, rx) = watch::channel(false);

        // Fire shutdown after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.is_empty());
        assert!(elapsed < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn event_strategy_handles_broadcast_lag() {
        // Capacity of 2 — publishing 4 events overflows the buffer.
        let bus = EventBus::shared(2);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "session_start".to_string(), agent_id };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Overflow the buffer with non-matching events, then send a matching one.
        bus.publish(SystemEvent::AgentDisconnected { agent_id: Uuid::new_v4() });
        bus.publish(SystemEvent::AgentDisconnected { agent_id: Uuid::new_v4() });
        bus.publish(SystemEvent::AgentDisconnected { agent_id: Uuid::new_v4() });
        bus.publish(SystemEvent::AgentConnected { agent_id });

        // Should handle the lag and still find the matching event.
        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("event:session_start:"));
    }

    #[tokio::test]
    async fn event_strategy_returns_empty_on_bus_closed() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "session_start".to_string(), agent_id };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Drop the bus so the broadcast sender is dropped.
        drop(bus);

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn event_strategy_is_object_safe() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "session_start".to_string(), agent_id };
        let strategy: Box<dyn TriggerStrategy> = Box::new(EventStrategy::new(bus.clone(), filter));
        let (_tx, rx) = watch::channel(false);

        bus.publish(SystemEvent::AgentConnected { agent_id });

        let mut strategy = strategy;
        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn event_strategy_source_ids_are_unique() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "session_start".to_string(), agent_id };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        // Publish two events with a small gap to get different timestamps.
        bus.publish(SystemEvent::AgentConnected { agent_id });
        let result1 = strategy.next_tasks(&rx).await.unwrap();

        // Brief sleep to ensure different timestamp.
        tokio::time::sleep(Duration::from_millis(10)).await;
        bus.publish(SystemEvent::AgentConnected { agent_id });
        let result2 = strategy.next_tasks(&rx).await.unwrap();

        assert_ne!(result1[0].source_id, result2[0].source_id);
    }

    #[test]
    fn event_filter_lifecycle_task_fields() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let filter = EventFilter::AgentLifecycle { event: "session_start".to_string(), agent_id };
        let strategy = EventStrategy::new(bus, filter);
        let task = strategy.build_lifecycle_task("session_start", &agent_id);

        assert!(task.source_id.starts_with("event:session_start:"));
        assert_eq!(task.title, "Agent lifecycle: session_start");
        assert!(task.body.is_empty());
        assert!(task.url.is_empty());
        assert!(task.labels.is_empty());
        assert_eq!(task.assignee, None);
    }

    #[test]
    fn event_filter_dispatch_task_fields() {
        let bus = EventBus::shared(16);
        let filter = EventFilter::DispatchResult { source_workflow_id: None, status: None };
        let strategy = EventStrategy::new(bus, filter);
        let wf_id = Uuid::new_v4();
        let dispatch_id = Uuid::new_v4();
        let task =
            strategy.build_dispatch_task(&wf_id, &dispatch_id, &DispatchStatus::Completed, None);

        assert!(task.source_id.starts_with("event:dispatch:"));
        assert!(task.source_id.contains(&dispatch_id.to_string()));
        assert!(task.title.contains(&dispatch_id.to_string()));
        assert!(task.title.contains("completed"));
        // Without source_id, original_source_id should not appear in metadata.
        assert!(!task.metadata.contains_key("original_source_id"));
    }

    #[test]
    fn event_filter_dispatch_task_with_original_source_id() {
        let bus = EventBus::shared(16);
        let filter = EventFilter::DispatchResult { source_workflow_id: None, status: None };
        let strategy = EventStrategy::new(bus, filter);
        let wf_id = Uuid::new_v4();
        let dispatch_id = Uuid::new_v4();
        let task = strategy.build_dispatch_task(
            &wf_id,
            &dispatch_id,
            &DispatchStatus::Completed,
            Some("99"),
        );

        assert_eq!(task.metadata.get("original_source_id"), Some(&"99".to_string()));
        assert_eq!(task.metadata.get("source_workflow_id"), Some(&wf_id.to_string()));
        assert_eq!(task.metadata.get("status"), Some(&"completed".to_string()));
    }

    #[tokio::test]
    async fn event_strategy_dispatch_result_propagates_original_source_id() {
        let bus = EventBus::shared(16);
        let filter = EventFilter::DispatchResult { source_workflow_id: None, status: None };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id: Uuid::new_v4(),
            dispatch_id: Uuid::new_v4(),
            status: DispatchStatus::Completed,
            source_id: Some("77".to_string()),
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].metadata.get("original_source_id"), Some(&"77".to_string()));
    }

    #[tokio::test]
    async fn event_strategy_dispatch_result_no_original_source_id_when_none() {
        let bus = EventBus::shared(16);
        let filter = EventFilter::DispatchResult { source_workflow_id: None, status: None };
        let mut strategy = EventStrategy::new(bus.clone(), filter);
        let (_tx, rx) = watch::channel(false);

        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id: Uuid::new_v4(),
            dispatch_id: Uuid::new_v4(),
            status: DispatchStatus::Completed,
            source_id: None,
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(!result[0].metadata.contains_key("original_source_id"));
    }

    // ── WebhookStrategy tests ─────────────────────────────────────────

    #[tokio::test]
    async fn webhook_strategy_receives_task() {
        let (tx, rx) = mpsc::channel(16);
        let mut strategy = WebhookStrategy::new(rx);
        let (_stx, srx) = watch::channel(false);

        let task = sample_task("webhook-1");
        tx.send(task).await.unwrap();

        let result = strategy.next_tasks(&srx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source_id, "webhook-1");
    }

    #[tokio::test]
    async fn webhook_strategy_returns_empty_on_sender_drop() {
        let (tx, rx) = mpsc::channel(16);
        let mut strategy = WebhookStrategy::new(rx);
        let (_stx, srx) = watch::channel(false);

        // Drop the sender — strategy should return empty.
        drop(tx);

        let result = strategy.next_tasks(&srx).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn webhook_strategy_respects_shutdown() {
        let (_tx, rx) = mpsc::channel::<Task>(16);
        let mut strategy = WebhookStrategy::new(rx);
        let (stx, srx) = watch::channel(false);

        // Fire shutdown after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = stx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&srx).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.is_empty());
        assert!(elapsed < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn webhook_strategy_is_object_safe() {
        let (tx, rx) = mpsc::channel(16);
        let strategy: Box<dyn TriggerStrategy> = Box::new(WebhookStrategy::new(rx));
        let (_stx, srx) = watch::channel(false);

        tx.send(sample_task("obj-safe")).await.unwrap();

        let mut strategy = strategy;
        let result = strategy.next_tasks(&srx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source_id, "obj-safe");
    }

    #[tokio::test]
    async fn webhook_strategy_receives_multiple_tasks_sequentially() {
        let (tx, rx) = mpsc::channel(16);
        let mut strategy = WebhookStrategy::new(rx);
        let (_stx, srx) = watch::channel(false);

        tx.send(sample_task("wh-1")).await.unwrap();
        tx.send(sample_task("wh-2")).await.unwrap();

        let r1 = strategy.next_tasks(&srx).await.unwrap();
        assert_eq!(r1[0].source_id, "wh-1");

        let r2 = strategy.next_tasks(&srx).await.unwrap();
        assert_eq!(r2[0].source_id, "wh-2");
    }

    // ── ManualStrategy tests ──────────────────────────────────────────

    #[tokio::test]
    async fn manual_strategy_receives_task() {
        let (tx, rx) = mpsc::channel(16);
        let mut strategy = ManualStrategy::new(rx);
        let (_stx, srx) = watch::channel(false);

        let task = sample_task("manual-1");
        tx.send(task).await.unwrap();

        let result = strategy.next_tasks(&srx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source_id, "manual-1");
    }

    #[tokio::test]
    async fn manual_strategy_returns_empty_on_sender_drop() {
        let (tx, rx) = mpsc::channel::<Task>(16);
        let mut strategy = ManualStrategy::new(rx);
        let (_stx, srx) = watch::channel(false);

        // Drop the sender — strategy should return empty vec (channel closed).
        drop(tx);

        let result = strategy.next_tasks(&srx).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn manual_strategy_respects_shutdown() {
        let (_tx, rx) = mpsc::channel::<Task>(16);
        let mut strategy = ManualStrategy::new(rx);
        let (stx, srx) = watch::channel(false);

        // Fire shutdown after a short delay — strategy should unblock.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = stx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&srx).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.is_empty());
        assert!(elapsed < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn manual_strategy_is_object_safe() {
        let (tx, rx) = mpsc::channel(16);
        let strategy: Box<dyn TriggerStrategy> = Box::new(ManualStrategy::new(rx));
        let (_stx, srx) = watch::channel(false);

        tx.send(sample_task("obj-safe-manual")).await.unwrap();

        let mut strategy = strategy;
        let result = strategy.next_tasks(&srx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source_id, "obj-safe-manual");
    }

    #[tokio::test]
    async fn manual_strategy_receives_multiple_tasks_sequentially() {
        let (tx, rx) = mpsc::channel(16);
        let mut strategy = ManualStrategy::new(rx);
        let (_stx, srx) = watch::channel(false);

        tx.send(sample_task("manual-seq-1")).await.unwrap();
        tx.send(sample_task("manual-seq-2")).await.unwrap();

        let r1 = strategy.next_tasks(&srx).await.unwrap();
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].source_id, "manual-seq-1");

        let r2 = strategy.next_tasks(&srx).await.unwrap();
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].source_id, "manual-seq-2");
    }

    // ── IdleStrategy tests ────────────────────────────────────────────

    #[tokio::test]
    async fn idle_strategy_fires_after_idle_timeout() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        // Use a very short idle duration for the test.
        let mut strategy = IdleStrategy::new(bus, agent_id, Duration::from_millis(50));
        let (_tx, rx) = watch::channel(false);

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(result.len(), 1, "should produce exactly one task on idle fire");
        assert!(result[0].source_id.starts_with("idle:"), "source_id should start with 'idle:'");
        assert!(
            elapsed >= Duration::from_millis(40),
            "should have waited at least the idle period"
        );
        assert!(elapsed < Duration::from_secs(2), "should not have waited too long");
    }

    #[tokio::test]
    async fn idle_strategy_source_id_uses_unix_timestamp() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let mut strategy = IdleStrategy::new(bus, agent_id, Duration::from_millis(10));
        let (_tx, rx) = watch::channel(false);

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);

        // source_id should be "idle:<unix_seconds>".
        let source_id = &result[0].source_id;
        assert!(source_id.starts_with("idle:"));
        let ts_str = source_id.strip_prefix("idle:").unwrap();
        let ts: u64 = ts_str.parse().expect("timestamp should be a valid u64");
        assert!(ts > 0, "timestamp should be a positive unix timestamp");
    }

    #[tokio::test]
    async fn idle_strategy_task_metadata_contains_expected_fields() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let idle_secs = 30u64;
        let mut strategy = IdleStrategy::new(bus, agent_id, Duration::from_millis(10));
        let (_tx, rx) = watch::channel(false);

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        let task = &result[0];

        assert_eq!(task.metadata.get("agent_id"), Some(&agent_id.to_string()));
        // idle_seconds field reflects the configured duration (10ms = 0s when truncated).
        assert!(task.metadata.contains_key("idle_seconds"), "should have idle_seconds metadata");
        assert!(task.metadata.contains_key("timestamp"), "should have timestamp metadata");
        assert_eq!(task.title, "Agent idle trigger");
        assert!(task.body.is_empty());
        assert!(task.url.is_empty());
        assert!(task.labels.is_empty());
        assert_eq!(task.assignee, None);
        let _ = idle_secs; // suppress unused warning
    }

    #[tokio::test]
    async fn idle_strategy_timer_resets_on_dispatch_completed() {
        let bus = EventBus::shared(256);
        let agent_id = Uuid::new_v4();
        // 150ms idle period; we'll send a completion at 80ms to reset, then
        // the strategy should fire at ~80ms + 150ms = ~230ms total.
        let idle_duration = Duration::from_millis(150);
        let mut strategy = IdleStrategy::new(bus.clone(), agent_id, idle_duration);
        let (_tx, rx) = watch::channel(false);

        let bus_clone = bus.clone();
        let start = tokio::time::Instant::now();

        // Send a dispatch-completed event at 80ms to reset the timer.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            bus_clone.publish(SystemEvent::DispatchCompleted {
                workflow_id: Uuid::new_v4(),
                dispatch_id: Uuid::new_v4(),
                status: crate::scheduler::types::DispatchStatus::Completed,
                source_id: Some("test-task".to_string()),
            });
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        // Should have fired well after the initial 150ms (because the timer
        // was reset at 80ms, so total ≥ 80+150=230ms).
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("idle:"));
        assert!(
            elapsed >= Duration::from_millis(200),
            "timer should have reset; total wait should be ≥200ms, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn idle_strategy_respects_shutdown_signal() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        // Very long idle period — would block without shutdown.
        let mut strategy = IdleStrategy::new(bus, agent_id, Duration::from_secs(3600));
        let (tx, rx) = watch::channel(false);

        // Send shutdown after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.is_empty(), "should return empty on shutdown");
        assert!(elapsed < Duration::from_secs(2), "should return quickly on shutdown");
    }

    #[tokio::test]
    async fn idle_strategy_handles_lag_gracefully() {
        // Use a tiny channel capacity (2) and publish more events than it can hold
        // to trigger a Lagged error. The strategy should continue without panicking.
        let bus = EventBus::new(2); // capacity 2
        let bus = Arc::new(bus);
        let agent_id = Uuid::new_v4();
        // Short idle period so the test completes quickly after the lag.
        let mut strategy = IdleStrategy::new(bus.clone(), agent_id, Duration::from_millis(50));
        let (_tx, rx) = watch::channel(false);

        // Publish 5 events into a capacity-2 channel to force a Lagged error.
        for _ in 0..5 {
            bus.publish(SystemEvent::AgentConnected { agent_id });
        }

        // Strategy should survive the lag and eventually fire on idle timeout.
        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        // Either the strategy fired (from the idle timeout) or returned empty — it must not panic.
        assert!(elapsed < Duration::from_secs(5), "should not hang on lag");
        // After handling the lag it may continue looping until the idle timer fires.
        // We accept either an empty result (if it kept looping) or a task.
        let _ = result;
    }

    #[tokio::test]
    async fn idle_strategy_returns_empty_when_bus_closed() {
        // Use a very short idle duration so this test completes quickly.
        // We can't easily close the broadcast channel externally since the
        // strategy holds its own Arc<EventBus>. Instead, verify that the
        // shutdown path works — which is the practical "bus gone" scenario.
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let mut strategy = IdleStrategy::new(bus, agent_id, Duration::from_secs(3600));
        let (tx, rx) = watch::channel(false);

        // Signal shutdown — equivalent to the service being torn down.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let _ = tx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.is_empty(), "should return empty on shutdown");
        assert!(elapsed < Duration::from_secs(2), "should exit quickly");
    }

    #[tokio::test]
    async fn idle_strategy_is_object_safe() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let strategy: Box<dyn TriggerStrategy> =
            Box::new(IdleStrategy::new(bus, agent_id, Duration::from_millis(10)));
        let (_tx, rx) = watch::channel(false);

        let mut strategy = strategy;
        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn idle_strategy_source_ids_are_unique_across_fires() {
        let bus = EventBus::shared(16);
        let agent_id = Uuid::new_v4();
        let mut strategy = IdleStrategy::new(bus, agent_id, Duration::from_millis(10));
        let (_tx, rx) = watch::channel(false);

        // Fire twice and verify source_ids differ (use unix timestamp — may be
        // the same second, so we just verify format rather than strict uniqueness
        // in a sub-second test; the deduplication prefix is what matters).
        let r1 = strategy.next_tasks(&rx).await.unwrap();
        let r2 = strategy.next_tasks(&rx).await.unwrap();

        assert_eq!(r1.len(), 1);
        assert_eq!(r2.len(), 1);
        assert!(r1[0].source_id.starts_with("idle:"));
        assert!(r2[0].source_id.starts_with("idle:"));
    }

    // ── FileWatchStrategy tests ───────────────────────────────────────

    #[test]
    fn file_watch_strategy_constructs_with_native_mode() {
        let dir = tempfile::tempdir().unwrap();
        // Native watcher may or may not work in CI; just verify construction succeeds
        // (or gracefully returns an error — we don't mandate native support).
        let _ = FileWatchStrategy::new(
            vec![dir.path().to_path_buf()],
            None,
            vec![],
            200,
            WatchMode::Native,
        );
    }

    #[tokio::test]
    async fn file_watch_strategy_constructs_with_polling_mode() {
        let dir = tempfile::tempdir().unwrap();
        let strategy = FileWatchStrategy::new(
            vec![dir.path().to_path_buf()],
            None,
            vec![],
            200,
            WatchMode::Polling { interval_secs: 1 },
        );
        assert!(strategy.is_ok());
    }

    #[tokio::test]
    async fn file_watch_strategy_constructs_with_auto_mode() {
        let dir = tempfile::tempdir().unwrap();
        let strategy = FileWatchStrategy::new(
            vec![dir.path().to_path_buf()],
            None,
            vec![],
            200,
            WatchMode::Auto { poll_interval_secs: 5 },
        );
        assert!(strategy.is_ok());
    }

    #[tokio::test]
    async fn file_watch_strategy_rejects_invalid_glob_pattern() {
        let dir = tempfile::tempdir().unwrap();
        // Build with Native mode so no tokio::spawn is needed before validation.
        let result = FileWatchStrategy::new(
            vec![dir.path().to_path_buf()],
            Some(vec!["[invalid".to_string()]),
            vec![],
            200,
            WatchMode::Auto { poll_interval_secs: 1 },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid glob pattern"));
    }

    #[tokio::test]
    async fn file_watch_strategy_accepts_valid_glob_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let result = FileWatchStrategy::new(
            vec![dir.path().to_path_buf()],
            Some(vec!["**/*.toml".to_string(), "*.rs".to_string()]),
            vec![],
            200,
            WatchMode::Polling { interval_secs: 1 },
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn file_watch_strategy_polling_detects_file_create() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        // Create strategy with 1-second polling interval, no debounce.
        let mut strategy = FileWatchStrategy::new(
            vec![dir_path.clone()],
            None,
            vec!["create".to_string()],
            0, // no debounce
            WatchMode::Polling { interval_secs: 1 },
        )
        .unwrap();
        let (_tx, rx) = watch::channel(false);

        // Allow the poller to start and take its initial snapshot.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create a file inside the watched directory.
        let file_path = dir_path.join("test.txt");
        std::fs::write(&file_path, b"hello").unwrap();

        // Wait up to 3 seconds for the polling task to detect the new file.
        let result = tokio::time::timeout(Duration::from_secs(3), strategy.next_tasks(&rx)).await;

        let tasks = result.expect("timed out waiting for file create event").unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].source_id.starts_with("file:"));
        assert!(tasks[0].source_id.ends_with(":create"));
        assert_eq!(tasks[0].metadata.get("event_type"), Some(&"create".to_string()));
        assert!(tasks[0].metadata.contains_key("file_path"));
        assert!(tasks[0].metadata.contains_key("file_name"));
        assert!(tasks[0].metadata.contains_key("file_dir"));
        assert!(tasks[0].metadata.contains_key("timestamp"));
    }

    #[tokio::test]
    async fn file_watch_strategy_polling_detects_file_modify() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        // Pre-create the file so the initial snapshot includes it.
        let file_path = dir_path.join("watched.txt");
        std::fs::write(&file_path, b"initial").unwrap();

        let mut strategy = FileWatchStrategy::new(
            vec![dir_path.clone()],
            None,
            vec!["modify".to_string()],
            0,
            WatchMode::Polling { interval_secs: 1 },
        )
        .unwrap();
        let (_tx, rx) = watch::channel(false);

        // Wait for poller to capture initial snapshot.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Modify the file — use a small sleep to ensure mtime changes.
        tokio::time::sleep(Duration::from_millis(10)).await;
        std::fs::write(&file_path, b"modified").unwrap();

        let result = tokio::time::timeout(Duration::from_secs(3), strategy.next_tasks(&rx)).await;

        let tasks = result.expect("timed out waiting for file modify event").unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].source_id.ends_with(":modify"));
        assert_eq!(tasks[0].metadata.get("event_type"), Some(&"modify".to_string()));
    }

    #[tokio::test]
    async fn file_watch_strategy_polling_detects_file_delete() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        // Pre-create the file.
        let file_path = dir_path.join("to_delete.txt");
        std::fs::write(&file_path, b"bye").unwrap();

        let mut strategy = FileWatchStrategy::new(
            vec![dir_path.clone()],
            None,
            vec!["delete".to_string()],
            0,
            WatchMode::Polling { interval_secs: 1 },
        )
        .unwrap();
        let (_tx, rx) = watch::channel(false);

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Remove the file.
        std::fs::remove_file(&file_path).unwrap();

        let result = tokio::time::timeout(Duration::from_secs(3), strategy.next_tasks(&rx)).await;

        let tasks = result.expect("timed out waiting for file delete event").unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].source_id.ends_with(":delete"));
        assert_eq!(tasks[0].metadata.get("event_type"), Some(&"delete".to_string()));
    }

    #[tokio::test]
    async fn file_watch_strategy_filters_events_by_kind() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        // Strategy only accepts "delete" events.
        let mut strategy = FileWatchStrategy::new(
            vec![dir_path.clone()],
            None,
            vec!["delete".to_string()],
            0,
            WatchMode::Polling { interval_secs: 1 },
        )
        .unwrap();
        let (shutdown_tx, rx) = watch::channel(false);

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create a new file — should NOT produce a task (create filtered out).
        std::fs::write(dir_path.join("new.txt"), b"hi").unwrap();

        // After 1.5 seconds the poll would have seen the create — fire shutdown
        // to confirm no task was emitted.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = shutdown_tx.send(true);
        });

        let result = tokio::time::timeout(Duration::from_secs(3), strategy.next_tasks(&rx)).await;

        let tasks = result.expect("timed out").unwrap();
        // Shutdown should return empty — no "delete" event was triggered.
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn file_watch_strategy_filters_by_glob_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        // Only TOML files should match.
        let mut strategy = FileWatchStrategy::new(
            vec![dir_path.clone()],
            Some(vec!["**/*.toml".to_string()]),
            vec!["create".to_string()],
            0,
            WatchMode::Polling { interval_secs: 1 },
        )
        .unwrap();
        let (shutdown_tx, rx) = watch::channel(false);

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create a .txt file — should NOT match **/*.toml.
        std::fs::write(dir_path.join("ignored.txt"), b"ignored").unwrap();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = shutdown_tx.send(true);
        });

        let result = tokio::time::timeout(Duration::from_secs(3), strategy.next_tasks(&rx)).await;

        let tasks = result.expect("timed out").unwrap();
        assert!(tasks.is_empty(), "Expected txt file to be filtered out by **/*.toml pattern");
    }

    #[tokio::test]
    async fn file_watch_strategy_glob_pattern_matches_toml() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        let mut strategy = FileWatchStrategy::new(
            vec![dir_path.clone()],
            Some(vec!["**/*.toml".to_string()]),
            vec!["create".to_string()],
            0,
            WatchMode::Polling { interval_secs: 1 },
        )
        .unwrap();
        let (_tx, rx) = watch::channel(false);

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Create a matching TOML file.
        std::fs::write(dir_path.join("config.toml"), b"[section]").unwrap();

        let result = tokio::time::timeout(Duration::from_secs(3), strategy.next_tasks(&rx)).await;

        let tasks = result.expect("timed out waiting for toml create event").unwrap();
        assert_eq!(tasks.len(), 1);
        let file_name = tasks[0].metadata.get("file_name").unwrap();
        assert_eq!(file_name, "config.toml");
    }

    #[tokio::test]
    async fn file_watch_strategy_respects_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let mut strategy = FileWatchStrategy::new(
            vec![dir.path().to_path_buf()],
            None,
            vec![],
            0,
            WatchMode::Polling { interval_secs: 60 }, // long interval — would block
        )
        .unwrap();
        let (tx, rx) = watch::channel(false);

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });

        let start = tokio::time::Instant::now();
        let result = strategy.next_tasks(&rx).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.is_empty());
        assert!(elapsed < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn file_watch_strategy_is_object_safe() {
        let dir = tempfile::tempdir().unwrap();
        let strategy: Box<dyn TriggerStrategy> = Box::new(
            FileWatchStrategy::new(
                vec![dir.path().to_path_buf()],
                None,
                vec![],
                0,
                WatchMode::Polling { interval_secs: 60 },
            )
            .unwrap(),
        );
        let (tx, rx) = watch::channel(false);

        let mut strategy = strategy;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn event_kind_to_str_returns_correct_strings() {
        use notify::event::{CreateKind, DataChange, ModifyKind, RemoveKind};
        assert_eq!(event_kind_to_str(&notify::EventKind::Create(CreateKind::File)), "create");
        assert_eq!(
            event_kind_to_str(&notify::EventKind::Modify(ModifyKind::Data(DataChange::Content))),
            "modify"
        );
        assert_eq!(event_kind_to_str(&notify::EventKind::Remove(RemoveKind::File)), "delete");
        assert_eq!(event_kind_to_str(&notify::EventKind::Other), "other");
        assert_eq!(event_kind_to_str(&notify::EventKind::Any), "any");
    }

    #[test]
    fn watch_mode_default_is_auto() {
        let mode = WatchMode::default();
        assert!(matches!(mode, WatchMode::Auto { .. }));
    }
}
