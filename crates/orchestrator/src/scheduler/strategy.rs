//! Trigger strategy abstraction for workflow scheduling.
//!
//! [`TriggerStrategy`] decouples *how* a workflow waits for its next event
//! from the scheduler itself.  The default implementation is polling-based,
//! but the trait is designed so that webhook-driven, cron, or event-stream
//! strategies can be swapped in without changing the runner.

use crate::scheduler::source::TaskSource;
use crate::scheduler::types::Task;
use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::watch;
use tracing::warn;

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
    async fn next_tasks(
        &mut self,
        shutdown: &watch::Receiver<bool>,
    ) -> anyhow::Result<Vec<Task>>;
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
        Self {
            source,
            interval: Duration::from_secs(poll_interval_secs),
            consecutive_errors: 0,
        }
    }

    /// Compute the total sleep duration including any error backoff.
    fn sleep_duration(&self) -> Duration {
        let backoff_secs = std::cmp::min(
            u64::from(self.consecutive_errors) * 2,
            MAX_BACKOFF_SECS,
        );
        self.interval + Duration::from_secs(backoff_secs)
    }
}

#[async_trait]
impl TriggerStrategy for PollingStrategy {
    async fn next_tasks(
        &mut self,
        shutdown: &watch::Receiver<bool>,
    ) -> anyhow::Result<Vec<Task>> {
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
                let backoff = std::cmp::min(
                    u64::from(self.consecutive_errors) * 2,
                    MAX_BACKOFF_SECS,
                );
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
}
