pub mod api;
pub mod github;
pub mod runner;
pub mod source;
pub mod storage;
pub mod strategy;
pub mod template;
pub mod types;

use crate::websocket::ConnectionRegistry;
use runner::{create_strategy, notify_complete, WorkflowRunner};
use std::collections::HashMap;
use std::sync::Arc;
use storage::SchedulerStorage;
use tokio::sync::{watch, Mutex, RwLock};
use tracing::{error, info, warn};
use types::WorkflowConfig;
use uuid::Uuid;

/// Tracks a running workflow's control handles.
struct RunningWorkflow {
    /// The agent this workflow dispatches to.
    agent_id: Uuid,
    shutdown_tx: watch::Sender<bool>,
    busy: Arc<Mutex<runner::BusyState>>,
}

/// Manages all active WorkflowRunners.
pub struct Scheduler {
    storage: SchedulerStorage,
    registry: ConnectionRegistry,
    /// Active workflow runners, keyed by workflow ID.
    runners: RwLock<HashMap<Uuid, RunningWorkflow>>,
}

impl Scheduler {
    pub fn new(storage: SchedulerStorage, registry: ConnectionRegistry) -> Self {
        Self { storage, registry, runners: RwLock::new(HashMap::new()) }
    }

    pub fn storage(&self) -> &SchedulerStorage {
        &self.storage
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

        let strategy = create_strategy(&config);
        let runner = WorkflowRunner::new(config, self.storage.clone(), self.registry.clone(), strategy);
        let shutdown_tx = runner.shutdown_handle();
        let busy = runner.busy_handle();

        // Spawn the runner.
        tokio::spawn(async move {
            runner.run().await;
        });

        // Track the runner.
        let mut runners = self.runners.write().await;
        runners.insert(workflow_id, RunningWorkflow { agent_id, shutdown_tx, busy });

        info!(%workflow_id, "Workflow started");
        Ok(())
    }

    /// Stop a running workflow.
    pub async fn stop_workflow(&self, workflow_id: &Uuid) -> anyhow::Result<()> {
        let mut runners = self.runners.write().await;
        if let Some(running) = runners.remove(workflow_id) {
            let _ = running.shutdown_tx.send(true);
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

            let has_active = {
                let busy = running.busy.lock().await;
                busy.active_dispatch_id.is_some()
            };
            if has_active {
                notify_complete(&running.busy, &self.storage, is_error).await;
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

    /// Shutdown all running workflows gracefully.
    pub async fn shutdown_all(&self) {
        let mut runners = self.runners.write().await;
        for (wf_id, running) in runners.drain() {
            let _ = running.shutdown_tx.send(true);
            info!(%wf_id, "Sent shutdown to workflow runner");
        }
    }
}
