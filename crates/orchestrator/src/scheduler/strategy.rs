//! Trigger strategy abstraction for workflow scheduling.
//!
//! [`TriggerStrategy`] decouples *how* a workflow waits for its next event
//! from the scheduler itself.  The default implementation is polling-based,
//! but the trait is designed so that webhook-driven, cron, or event-stream
//! strategies can be swapped in without changing the runner.

use crate::scheduler::source::TaskSource;
use crate::scheduler::types::Task;
use async_trait::async_trait;
use chrono::Utc;
use croner::Cron;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{info, warn};

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
        assert_eq!(
            task.metadata.get("cron_expression"),
            Some(&"0 9 * * MON-FRI".to_string())
        );
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
            "0 9 * * MON-FRI",   // 9 AM weekdays
            "*/5 * * * *",       // every 5 minutes
            "0 0 * * *",         // midnight daily
            "0 12 1 * *",        // noon on 1st of month
            "30 4 * * SUN",      // 4:30 AM on Sundays
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
}
