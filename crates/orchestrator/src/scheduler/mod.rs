pub mod api;
pub mod github;
pub mod runner;
pub mod source;
pub mod storage;
pub mod template;
pub mod types;

use crate::websocket::ConnectionRegistry;
use runner::{notify_complete, WorkflowRunner};
use std::collections::HashMap;
use std::sync::Arc;
use storage::SchedulerStorage;
use tokio::sync::{watch, Mutex, RwLock};
use tracing::{error, info, warn};
use types::WorkflowConfig;
use uuid::Uuid;

/// Tracks a running workflow's control handles.
struct RunningWorkflow {
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
        Self {
            storage,
            registry,
            runners: RwLock::new(HashMap::new()),
        }
    }

    pub fn storage(&self) -> &SchedulerStorage {
        &self.storage
    }

    /// Start a workflow runner as a background tokio task.
    pub async fn start_workflow(&self, config: WorkflowConfig) -> anyhow::Result<()> {
        let workflow_id = config.id;

        // Check if already running.
        {
            let runners = self.runners.read().await;
            if runners.contains_key(&workflow_id) {
                anyhow::bail!("Workflow {} is already running", workflow_id);
            }
        }

        let runner = WorkflowRunner::new(config, self.storage.clone(), self.registry.clone());
        let shutdown_tx = runner.shutdown_handle();
        let busy = runner.busy_handle();

        // Spawn the runner.
        tokio::spawn(async move {
            runner.run().await;
        });

        // Track the runner.
        let mut runners = self.runners.write().await;
        runners.insert(workflow_id, RunningWorkflow { shutdown_tx, busy });

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
    pub async fn notify_task_complete(&self, agent_id: Uuid, is_error: bool) {
        let runners = self.runners.read().await;
        for (_wf_id, running) in runners.iter() {
            // Check each runner's busy state — the one with an active dispatch for this agent.
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
    pub async fn resume_workflows(&self) -> anyhow::Result<()> {
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
                info!(
                    workflow_id = %workflow.id,
                    agent_id = %workflow.agent_id,
                    "Skipping workflow resume: agent not connected"
                );
            }
        }

        Ok(())
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
