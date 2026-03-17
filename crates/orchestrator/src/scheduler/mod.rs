pub mod api;
pub mod events;
pub mod github;
pub mod runner;
pub mod source;
pub mod storage;
pub mod strategy;
pub mod template;
pub mod types;
pub mod webhook;

use crate::websocket::ConnectionRegistry;
use events::{EventBus, SystemEvent};
use runner::{create_strategy, notify_complete, RunOutcome, WorkflowRunner};
use std::collections::HashMap;
use std::sync::Arc;
use storage::SchedulerStorage;
use strategy::{ManualStrategy, WebhookStrategy};
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use tracing::{error, info, warn};
use types::{DispatchRecord, DispatchStatus, Task, TriggerConfig, WorkflowConfig};
use uuid::Uuid;
use webhook::WebhookRegistry;

/// Tracks a running workflow's control handles.
struct RunningWorkflow {
    /// The workflow ID (needed for dispatch completion events).
    workflow_id: Uuid,
    /// The agent this workflow dispatches to.
    agent_id: Uuid,
    shutdown_tx: watch::Sender<bool>,
    busy: Arc<Mutex<runner::BusyState>>,
    /// Channel sender for Manual-type workflows.
    /// Used by the `POST /workflows/{id}/trigger` API endpoint.
    manual_tx: Option<mpsc::Sender<Task>>,
}

/// Manages all active WorkflowRunners.
pub struct Scheduler {
    storage: SchedulerStorage,
    registry: ConnectionRegistry,
    /// Active workflow runners, keyed by workflow ID.
    runners: RwLock<HashMap<Uuid, RunningWorkflow>>,
    /// Optional event bus for publishing dispatch lifecycle events.
    event_bus: Option<Arc<EventBus>>,
    /// Shared registry for webhook channel senders.
    webhook_registry: Arc<WebhookRegistry>,
}

impl Scheduler {
    pub fn new(storage: SchedulerStorage, registry: ConnectionRegistry) -> Self {
        Self {
            storage,
            registry,
            runners: RwLock::new(HashMap::new()),
            event_bus: None,
            webhook_registry: Arc::new(WebhookRegistry::new()),
        }
    }

    /// Create a scheduler with an event bus for publishing lifecycle events.
    pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub fn storage(&self) -> &SchedulerStorage {
        &self.storage
    }

    /// Return a reference to the webhook registry for use by API handlers.
    pub fn webhook_registry(&self) -> &Arc<WebhookRegistry> {
        &self.webhook_registry
    }

    /// Start a workflow runner as a background tokio task.
    pub async fn start_workflow(&self, config: WorkflowConfig) -> anyhow::Result<()> {
        let workflow_id = config.id;
        let agent_id = config.agent_id;

        // Check if already running.
        {
            let runners = self.runners.read().await;
            if runners.contains_key(&workflow_id) {
                anyhow::bail!("Workflow {} is already running", workflow_id);
            }
        }

        // Build strategy and capture any channel sender for the workflow type.
        let mut manual_tx: Option<mpsc::Sender<Task>> = None;
        let strategy: Box<dyn strategy::TriggerStrategy> =
            if let TriggerConfig::Webhook { ref secret } = config.trigger_config {
                let (tx, rx) = mpsc::channel(webhook::DEFAULT_CHANNEL_CAPACITY);
                self.webhook_registry.register(workflow_id, tx, secret.clone()).await;
                Box::new(WebhookStrategy::new(rx))
            } else if matches!(config.trigger_config, TriggerConfig::Manual {}) {
                let (tx, rx) = mpsc::channel(webhook::DEFAULT_CHANNEL_CAPACITY);
                manual_tx = Some(tx);
                Box::new(ManualStrategy::new(rx))
            } else {
                create_strategy(&config, self.event_bus.as_ref())?
            };
        let runner =
            WorkflowRunner::new(config, self.storage.clone(), self.registry.clone(), strategy);
        let shutdown_tx = runner.shutdown_handle();
        let busy = runner.busy_handle();

        // Spawn the runner. If it returns AutoDisable, update the workflow
        // in storage and remove the runner from the active set.
        let storage = self.storage.clone();
        tokio::spawn(async move {
            let outcome = runner.run().await;
            if outcome == RunOutcome::AutoDisable {
                info!(%workflow_id, "Auto-disabling one-shot workflow");
                if let Ok(Some(mut wf)) = storage.get_workflow(&workflow_id).await {
                    wf.enabled = false;
                    wf.updated_at = chrono::Utc::now();
                    if let Err(e) = storage.update_workflow(&wf).await {
                        error!(%workflow_id, %e, "Failed to auto-disable workflow");
                    }
                }
            }
        });

        // Track the runner.
        let mut runners = self.runners.write().await;
        runners.insert(
            workflow_id,
            RunningWorkflow { workflow_id, agent_id, shutdown_tx, busy, manual_tx },
        );

        info!(%workflow_id, "Workflow started");
        Ok(())
    }

    /// Stop a running workflow.
    pub async fn stop_workflow(&self, workflow_id: &Uuid) -> anyhow::Result<()> {
        let mut runners = self.runners.write().await;
        if let Some(running) = runners.remove(workflow_id) {
            let _ = running.shutdown_tx.send(true);
            // Unregister from webhook registry (no-op if not a webhook workflow).
            self.webhook_registry.unregister(workflow_id).await;
            info!(%workflow_id, "Workflow stopped");
            Ok(())
        } else {
            anyhow::bail!("Workflow {} is not running", workflow_id)
        }
    }

    /// Called when an agent produces a "result" message, to clear the busy flag
    /// and update the dispatch record.
    ///
    /// Only notifies the runner whose workflow is dispatched to the matching
    /// `agent_id` — prevents the wrong runner from being notified when
    /// multiple workflows are running concurrently with different agents.
    pub async fn notify_task_complete(&self, agent_id: Uuid, is_error: bool) {
        let runners = self.runners.read().await;
        for (_wf_id, running) in runners.iter() {
            // Only consider runners that target the completing agent.
            if running.agent_id != agent_id {
                continue;
            }

            let dispatch_id = {
                let busy = running.busy.lock().await;
                busy.active_dispatch_id
            };
            if let Some(dispatch_id) = dispatch_id {
                notify_complete(&running.busy, &self.storage, is_error).await;

                // Publish dispatch completion event.
                if let Some(bus) = &self.event_bus {
                    let status =
                        if is_error { DispatchStatus::Failed } else { DispatchStatus::Completed };
                    bus.publish(SystemEvent::DispatchCompleted {
                        workflow_id: running.workflow_id,
                        dispatch_id,
                        status,
                    });
                }

                info!(%agent_id, is_error, "Agent task completion processed");
                return;
            }
        }
        // No active dispatch found — could be a non-workflow agent, which is fine.
    }

    /// Called on startup to resume enabled workflows whose agents are connected.
    ///
    /// Agents restarted during reconcile may take several seconds to initialise
    /// and establish their WebSocket connection. For workflows whose agent is
    /// not yet connected, a background task waits up to 60 seconds for the
    /// agent to appear before giving up.
    pub async fn resume_workflows(self: &Arc<Self>) -> anyhow::Result<()> {
        // First, mark any in-flight dispatches from a previous run as failed.
        let failed = self.storage.fail_inflight_dispatches().await?;
        if failed > 0 {
            warn!(count = failed, "Marked in-flight dispatches as failed on startup");
        }

        let workflows = self.storage.list_workflows().await?;
        for workflow in workflows {
            if !workflow.enabled {
                continue;
            }

            if self.registry.is_connected(&workflow.agent_id).await {
                info!(
                    workflow_id = %workflow.id,
                    agent_id = %workflow.agent_id,
                    "Resuming workflow"
                );
                if let Err(e) = self.start_workflow(workflow).await {
                    error!(%e, "Failed to resume workflow");
                }
            } else {
                // Agent not connected yet — wait for it in the background.
                let scheduler = Arc::clone(self);
                tokio::spawn(async move {
                    info!(
                        workflow_id = %workflow.id,
                        agent_id = %workflow.agent_id,
                        "Waiting for agent to connect before resuming workflow"
                    );
                    let timeout = std::time::Duration::from_secs(60);
                    if scheduler.registry.wait_for_agent(&workflow.agent_id, timeout).await {
                        info!(
                            workflow_id = %workflow.id,
                            agent_id = %workflow.agent_id,
                            "Agent connected, resuming workflow"
                        );
                        if let Err(e) = scheduler.start_workflow(workflow).await {
                            error!(%e, "Failed to resume workflow after agent connected");
                        }
                    } else {
                        warn!(
                            workflow_id = %workflow.id,
                            agent_id = %workflow.agent_id,
                            "Agent did not connect within 60s, workflow not resumed"
                        );
                    }
                });
            }
        }

        Ok(())
    }

    /// Return the set of currently running workflow IDs and their assigned agent IDs.
    pub async fn running_workflows(&self) -> Vec<(Uuid, Uuid)> {
        self.runners.read().await.iter().map(|(wf_id, rw)| (*wf_id, rw.agent_id)).collect()
    }

    /// Manually trigger a workflow, bypassing its normal trigger strategy.
    ///
    /// Creates a synthetic [`Task`] from the provided data, renders the
    /// workflow's prompt template, records the dispatch, and sends the prompt
    /// to the workflow's agent.
    ///
    /// For `Manual`-type workflows, the task is pushed through the workflow's
    /// dedicated `mpsc` channel so the running [`WorkflowRunner`] handles it
    /// with full busy-state tracking.  For all other trigger types the task
    /// is dispatched directly (bypassing the strategy), which is useful for
    /// ad-hoc testing.
    ///
    /// Returns the created [`DispatchRecord`] on success.
    pub async fn trigger_workflow(
        &self,
        workflow_id: &Uuid,
        task: Task,
    ) -> anyhow::Result<DispatchRecord> {
        use crate::scheduler::template::render_template;

        // Load workflow config (must exist and be enabled).
        let workflow = self
            .storage
            .get_workflow(workflow_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Workflow not found"))?;

        if !workflow.enabled {
            anyhow::bail!("Workflow {} is not enabled", workflow_id);
        }

        // For Manual-type workflows with a running runner, send via the channel
        // so the runner can apply its full dispatch logic (busy-state tracking, etc.).
        {
            let runners = self.runners.read().await;
            if let Some(running) = runners.get(workflow_id) {
                if let Some(ref tx) = running.manual_tx {
                    // Check busy state before sending.
                    let busy = running.busy.lock().await;
                    if busy.active_dispatch_id.is_some() {
                        anyhow::bail!("Agent is currently busy processing another task");
                    }
                    drop(busy);

                    tx.try_send(task.clone()).map_err(|e| match e {
                        mpsc::error::TrySendError::Full(_) => {
                            anyhow::anyhow!("Manual trigger channel full")
                        }
                        mpsc::error::TrySendError::Closed(_) => {
                            anyhow::anyhow!("Manual trigger channel closed")
                        }
                    })?;

                    // Create the dispatch record immediately so we can return it.
                    let prompt = render_template(&workflow.prompt_template, &task);
                    let record = DispatchRecord {
                        id: Uuid::new_v4(),
                        workflow_id: *workflow_id,
                        source_id: task.source_id.clone(),
                        agent_id: workflow.agent_id,
                        prompt_sent: prompt,
                        status: DispatchStatus::Pending,
                        dispatched_at: chrono::Utc::now(),
                        completed_at: None,
                    };
                    self.storage.add_dispatch(&record).await?;

                    info!(
                        %workflow_id,
                        source_id = %task.source_id,
                        "Manual trigger queued via channel"
                    );
                    return Ok(record);
                }
            }
        }

        // Direct dispatch: render prompt, record, and send to agent.
        // Works for any workflow type (bypasses normal trigger strategy).
        let prompt = render_template(&workflow.prompt_template, &task);

        // Verify the agent is connected.
        if !self.registry.is_connected(&workflow.agent_id).await {
            anyhow::bail!("Agent {} is not connected", workflow.agent_id);
        }

        let record = DispatchRecord {
            id: Uuid::new_v4(),
            workflow_id: *workflow_id,
            source_id: task.source_id.clone(),
            agent_id: workflow.agent_id,
            prompt_sent: prompt.clone(),
            status: DispatchStatus::Dispatched,
            dispatched_at: chrono::Utc::now(),
            completed_at: None,
        };
        self.storage.add_dispatch(&record).await?;

        // Update the runner's busy state so completion is tracked correctly.
        {
            let runners = self.runners.read().await;
            if let Some(running) = runners.get(workflow_id) {
                let mut busy = running.busy.lock().await;
                busy.active_dispatch_id = Some(record.id);
            }
        }

        // Apply the workflow's tool policy.
        self.registry.set_policy(workflow.agent_id, workflow.tool_policy.clone()).await;

        // Send the prompt to the agent.
        if let Err(e) = self.registry.send_user_message(&workflow.agent_id, &prompt).await {
            // Best-effort: mark the dispatch as failed.
            let _ = self
                .storage
                .update_dispatch_status(
                    &record.id,
                    DispatchStatus::Failed,
                    Some(chrono::Utc::now()),
                )
                .await;
            // Clear busy state.
            let runners = self.runners.read().await;
            if let Some(running) = runners.get(workflow_id) {
                let mut busy = running.busy.lock().await;
                busy.active_dispatch_id = None;
            }
            return Err(e);
        }

        info!(
            %workflow_id,
            source_id = %task.source_id,
            "Manual trigger dispatched directly to agent"
        );

        Ok(record)
    }

    /// Shutdown all running workflows gracefully.
    pub async fn shutdown_all(&self) {
        let mut runners = self.runners.write().await;
        for (wf_id, running) in runners.drain() {
            let _ = running.shutdown_tx.send(true);
            info!(%wf_id, "Sent shutdown to workflow runner");
        }
    }
}
