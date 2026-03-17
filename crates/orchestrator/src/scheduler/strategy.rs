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
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch};
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
                SystemEvent::DispatchCompleted { workflow_id, dispatch_id, status },
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
                Some(self.build_dispatch_task(workflow_id, dispatch_id, status))
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
    fn build_dispatch_task(
        &self,
        workflow_id: &Uuid,
        dispatch_id: &Uuid,
        status: &DispatchStatus,
    ) -> Task {
        let timestamp = Utc::now().to_rfc3339();
        let mut metadata = HashMap::new();
        metadata.insert("source_workflow_id".to_string(), workflow_id.to_string());
        metadata.insert("dispatch_id".to_string(), dispatch_id.to_string());
        metadata.insert("status".to_string(), status.to_string());
        metadata.insert("timestamp".to_string(), timestamp.clone());

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
        });

        let result = strategy.next_tasks(&rx).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].source_id.starts_with("event:dispatch:"));
        assert!(result[0].source_id.contains(&dispatch_id.to_string()));
        assert_eq!(result[0].metadata.get("source_workflow_id"), Some(&workflow_id.to_string()));
        assert_eq!(result[0].metadata.get("dispatch_id"), Some(&dispatch_id.to_string()));
        assert_eq!(result[0].metadata.get("status"), Some(&"completed".to_string()));
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
        });
        // Publish for correct workflow — should match.
        let expected_dispatch = Uuid::new_v4();
        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id: target_wf,
            dispatch_id: expected_dispatch,
            status: DispatchStatus::Failed,
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
        });
        // Publish a Failed event — should match.
        let expected_dispatch = Uuid::new_v4();
        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id,
            dispatch_id: expected_dispatch,
            status: DispatchStatus::Failed,
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
        let task = strategy.build_dispatch_task(&wf_id, &dispatch_id, &DispatchStatus::Completed);

        assert!(task.source_id.starts_with("event:dispatch:"));
        assert!(task.source_id.contains(&dispatch_id.to_string()));
        assert!(task.title.contains(&dispatch_id.to_string()));
        assert!(task.title.contains("completed"));
    }
}
