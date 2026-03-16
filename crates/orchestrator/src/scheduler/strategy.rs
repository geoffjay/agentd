//! Trigger strategy abstraction for workflow scheduling.
//!
//! [`TriggerStrategy`] decouples *how* a workflow waits for its next event
//! from the scheduler itself.  The default implementation is polling-based,
//! but the trait is designed so that webhook-driven, cron, or event-stream
//! strategies can be swapped in without changing the runner.

use crate::scheduler::types::Task;
use async_trait::async_trait;
use tokio::sync::watch;

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
