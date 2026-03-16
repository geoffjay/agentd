use crate::scheduler::github::{GithubIssueSource, GithubPullRequestSource};
use crate::scheduler::source::TaskSource;
use crate::scheduler::storage::SchedulerStorage;
use crate::scheduler::strategy::{PollingStrategy, TriggerStrategy};
use crate::scheduler::template::render_template;
use crate::scheduler::types::{
    DispatchRecord, DispatchStatus, Task, TaskSourceConfig, WorkflowConfig,
};
use crate::websocket::ConnectionRegistry;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Tracks whether an agent is currently processing a task.
#[derive(Debug)]
pub struct BusyState {
    /// The dispatch record ID of the currently active task.
    pub(crate) active_dispatch_id: Option<Uuid>,
}

/// Runs the trigger-dispatch loop for a single workflow.
///
/// The runner delegates trigger timing to a [`TriggerStrategy`] and focuses
/// on dispatch logic: dedup checking, template rendering, and sending prompts
/// to the connected agent.
pub struct WorkflowRunner {
    config: WorkflowConfig,
    storage: SchedulerStorage,
    registry: ConnectionRegistry,
    strategy: Box<dyn TriggerStrategy>,
    busy: Arc<Mutex<BusyState>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl WorkflowRunner {
    /// Create a new runner with an explicit trigger strategy.
    pub fn new(
        config: WorkflowConfig,
        storage: SchedulerStorage,
        registry: ConnectionRegistry,
        strategy: Box<dyn TriggerStrategy>,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Self {
            config,
            storage,
            registry,
            strategy,
            busy: Arc::new(Mutex::new(BusyState { active_dispatch_id: None })),
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Returns a handle to signal shutdown.
    pub fn shutdown_handle(&self) -> watch::Sender<bool> {
        self.shutdown_tx.clone()
    }

    /// Returns a handle to clear the busy state when a task completes.
    pub fn busy_handle(&self) -> Arc<Mutex<BusyState>> {
        self.busy.clone()
    }

    /// Run the trigger-dispatch loop. This should be spawned as a tokio task.
    ///
    /// The loop calls [`TriggerStrategy::next_tasks()`] to wait for tasks,
    /// then dispatches the first undispatched task to the connected agent.
    /// The strategy handles interval timing, backoff, and shutdown detection.
    pub async fn run(mut self) {
        let workflow_id = self.config.id;
        let agent_id = self.config.agent_id;

        info!(%workflow_id, %agent_id, "Workflow runner started");

        loop {
            // Wait for the strategy to produce tasks (includes interval/backoff).
            let tasks = match self.strategy.next_tasks(&self.shutdown_rx).await {
                Ok(tasks) => tasks,
                Err(e) => {
                    // Strategy already logged the error and applied backoff.
                    warn!(%workflow_id, %e, "Strategy returned error, retrying");
                    continue;
                }
            };

            // Check for shutdown signal — strategy returns Ok(vec![]) on shutdown.
            if *self.shutdown_rx.borrow() {
                info!(%workflow_id, "Shutdown signal received, stopping runner");
                break;
            }

            if tasks.is_empty() {
                debug!(%workflow_id, "No new tasks from strategy");
                continue;
            }

            // Check if agent is busy.
            {
                let busy = self.busy.lock().await;
                if busy.active_dispatch_id.is_some() {
                    debug!(%workflow_id, "Agent busy, skipping dispatch");
                    continue;
                }
            }

            // Check agent is connected.
            if !self.registry.is_connected(&agent_id).await {
                debug!(%workflow_id, %agent_id, "Agent not connected, skipping dispatch");
                continue;
            }

            match self.dispatch_tasks(tasks).await {
                Ok(dispatched) => {
                    if dispatched {
                        info!(%workflow_id, "Task dispatched to agent");
                    } else {
                        debug!(%workflow_id, "All tasks already dispatched");
                    }
                }
                Err(e) => {
                    warn!(%workflow_id, %e, "Dispatch failed");
                }
            }
        }

        info!(%workflow_id, "Workflow runner stopped");
    }

    /// Find the first undispatched task and send it to the agent.
    ///
    /// This preserves the original dispatch logic: dedup check, template
    /// rendering, dispatch record creation, tool policy application, and
    /// prompt delivery — dispatching at most one task per cycle.
    async fn dispatch_tasks(&self, tasks: Vec<Task>) -> anyhow::Result<bool> {
        for task in tasks {
            // Skip already-dispatched tasks.
            if self.storage.is_dispatched(&self.config.id, &task.source_id).await? {
                continue;
            }

            // Render prompt from template.
            let prompt = render_template(&self.config.prompt_template, &task);

            // Create dispatch record.
            let record = DispatchRecord {
                id: Uuid::new_v4(),
                workflow_id: self.config.id,
                source_id: task.source_id.clone(),
                agent_id: self.config.agent_id,
                prompt_sent: prompt.clone(),
                status: DispatchStatus::Dispatched,
                dispatched_at: Utc::now(),
                completed_at: None,
            };
            self.storage.add_dispatch(&record).await?;

            // Mark busy before sending.
            {
                let mut busy = self.busy.lock().await;
                busy.active_dispatch_id = Some(record.id);
            }

            // Apply the workflow's tool policy to the agent before dispatching.
            self.registry.set_policy(self.config.agent_id, self.config.tool_policy.clone()).await;

            // Send prompt to agent.
            if let Err(e) = self.registry.send_user_message(&self.config.agent_id, &prompt).await {
                error!(
                    workflow_id = %self.config.id,
                    source_id = %task.source_id,
                    %e,
                    "Failed to send prompt to agent"
                );
                // Mark dispatch as failed and clear busy.
                let _ = self
                    .storage
                    .update_dispatch_status(&record.id, DispatchStatus::Failed, Some(Utc::now()))
                    .await;
                let mut busy = self.busy.lock().await;
                busy.active_dispatch_id = None;
                return Err(e);
            }

            info!(
                workflow_id = %self.config.id,
                source_id = %task.source_id,
                "Dispatched task to agent"
            );

            // Only dispatch one task at a time.
            return Ok(true);
        }

        Ok(false)
    }
}

/// Notify that an agent has completed its task, clearing the busy flag and updating storage.
pub async fn notify_complete(
    busy: &Arc<Mutex<BusyState>>,
    storage: &SchedulerStorage,
    is_error: bool,
) {
    let dispatch_id = {
        let mut busy = busy.lock().await;
        busy.active_dispatch_id.take()
    };

    if let Some(id) = dispatch_id {
        let status = if is_error { DispatchStatus::Failed } else { DispatchStatus::Completed };
        if let Err(e) = storage.update_dispatch_status(&id, status, Some(Utc::now())).await {
            error!(%id, %e, "Failed to update dispatch status on completion");
        }
    }
}

/// Create a [`TaskSource`] from a [`TaskSourceConfig`].
fn create_source(config: &TaskSourceConfig) -> Box<dyn TaskSource> {
    match config {
        TaskSourceConfig::GithubIssues { owner, repo, labels, state } => Box::new(
            GithubIssueSource::new(owner.clone(), repo.clone(), labels.clone(), state.clone()),
        ),
        TaskSourceConfig::GithubPullRequests { owner, repo, labels, state } => {
            Box::new(GithubPullRequestSource::new(
                owner.clone(),
                repo.clone(),
                labels.clone(),
                state.clone(),
            ))
        }
    }
}

/// Create the appropriate [`TriggerStrategy`] for a workflow configuration.
///
/// Currently all workflows use polling, so this builds a [`PollingStrategy`]
/// wrapping the task source derived from the workflow's `source_config`.
pub fn create_strategy(config: &WorkflowConfig) -> Box<dyn TriggerStrategy> {
    let source = create_source(&config.source_config);
    Box::new(PollingStrategy::new(source, config.poll_interval_secs))
}
