use crate::scheduler::github::{GithubIssueSource, GithubPullRequestSource};
use crate::scheduler::source::TaskSource;
use crate::scheduler::storage::SchedulerStorage;
use crate::scheduler::template::render_template;
use crate::scheduler::types::{DispatchRecord, DispatchStatus, TaskSourceConfig, WorkflowConfig};
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

/// Runs the poll-dispatch loop for a single workflow.
pub struct WorkflowRunner {
    config: WorkflowConfig,
    storage: SchedulerStorage,
    registry: ConnectionRegistry,
    source: Box<dyn TaskSource>,
    busy: Arc<Mutex<BusyState>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl WorkflowRunner {
    pub fn new(
        config: WorkflowConfig,
        storage: SchedulerStorage,
        registry: ConnectionRegistry,
    ) -> Self {
        let source = create_source(&config.source_config);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Self {
            config,
            storage,
            registry,
            source,
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

    /// Run the polling loop. This should be spawned as a tokio task.
    pub async fn run(mut self) {
        let workflow_id = self.config.id;
        let agent_id = self.config.agent_id;
        let interval_secs = self.config.poll_interval_secs;
        let mut consecutive_errors: u32 = 0;

        info!(%workflow_id, %agent_id, interval_secs, "Workflow runner started");

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        // Don't tick immediately on start — wait one interval first.
        interval.tick().await;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Check if agent is busy.
                    {
                        let busy = self.busy.lock().await;
                        if busy.active_dispatch_id.is_some() {
                            debug!(%workflow_id, "Agent busy, skipping poll cycle");
                            continue;
                        }
                    }

                    // Check agent is connected.
                    if !self.registry.is_connected(&agent_id).await {
                        debug!(%workflow_id, %agent_id, "Agent not connected, skipping poll");
                        continue;
                    }

                    match self.poll_and_dispatch().await {
                        Ok(dispatched) => {
                            consecutive_errors = 0;
                            if dispatched {
                                info!(%workflow_id, "Task dispatched to agent");
                            } else {
                                debug!(%workflow_id, "No new tasks to dispatch");
                            }
                        }
                        Err(e) => {
                            consecutive_errors += 1;
                            let backoff = std::cmp::min(consecutive_errors * 2, 30);
                            warn!(
                                %workflow_id,
                                %e,
                                consecutive_errors,
                                backoff_multiplier = backoff,
                                "Poll cycle failed"
                            );
                        }
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        info!(%workflow_id, "Shutdown signal received, stopping runner");
                        break;
                    }
                }
            }
        }

        info!(%workflow_id, "Workflow runner stopped");
    }

    /// Poll the source, find the first undispatched task, and send it to the agent.
    async fn poll_and_dispatch(&self) -> anyhow::Result<bool> {
        let tasks = self.source.fetch_tasks().await?;

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

fn create_source(config: &TaskSourceConfig) -> Box<dyn TaskSource> {
    match config {
        TaskSourceConfig::GithubIssues { owner, repo, labels, state } => Box::new(
            GithubIssueSource::new(owner.clone(), repo.clone(), labels.clone(), state.clone()),
        ),
        TaskSourceConfig::GithubPullRequests { owner, repo, labels, state } => Box::new(
            GithubPullRequestSource::new(
                owner.clone(),
                repo.clone(),
                labels.clone(),
                state.clone(),
            ),
        ),
    }
}
