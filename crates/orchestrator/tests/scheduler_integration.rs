//! Integration tests for the Scheduler and TriggerStrategy implementations.
//!
//! These tests exercise the full workflow lifecycle using an in-memory SQLite
//! database and a mock `ConnectionRegistry`. No real agents or network calls
//! are made.
//!
//! # What is tested
//!
//! - Full workflow lifecycle: create config → `start_workflow` → `trigger_workflow`
//!   → verify dispatch record in storage.
//! - `ManualStrategy` path: trigger via API channel → status is `Pending`.
//! - Direct dispatch path (non-Manual trigger type): status is `Dispatched`.
//! - `notify_task_complete()` updates dispatch status to `Completed`/`Failed`.
//! - Concurrent workflows with different trigger types.

use chrono::Utc;
use orchestrator::{
    scheduler::{
        storage::SchedulerStorage,
        types::{DispatchStatus, Task, TriggerConfig, WorkflowConfig},
        Scheduler,
    },
    storage::AgentStorage,
    websocket::{AgentConnection, ConnectionRegistry},
};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Create an in-memory (temp-file) SQLite database with full migrations applied.
///
/// Returns both the [`SchedulerStorage`] and the [`TempDir`] — the caller must
/// hold `_tmp` alive for the duration of the test.
async fn create_test_storage() -> (SchedulerStorage, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let agent_storage = AgentStorage::with_path(&db_path).await.unwrap();
    let storage = SchedulerStorage::new(agent_storage.db().clone());
    (storage, temp_dir)
}

/// Build a minimal [`WorkflowConfig`] with the given trigger type.
fn make_workflow(agent_id: Uuid, trigger_config: TriggerConfig) -> WorkflowConfig {
    let now = Utc::now();
    WorkflowConfig {
        id: Uuid::new_v4(),
        name: "test-workflow".to_string(),
        agent_id,
        trigger_config,
        prompt_template: "Handle: {{title}}".to_string(),
        poll_interval_secs: 60,
        enabled: true,
        tool_policy: Default::default(),
        created_at: now,
        updated_at: now,
    }
}

/// Build a synthetic [`Task`] with the given source ID.
fn make_task(source_id: &str) -> Task {
    Task {
        source_id: source_id.to_string(),
        title: format!("Task {source_id}"),
        body: String::new(),
        url: String::new(),
        labels: vec![],
        assignee: None,
        metadata: HashMap::new(),
    }
}

/// Register a mock agent connection in `registry` and return the receiver end
/// of the channel (so the test can inspect messages sent to the "agent").
async fn register_mock_agent(
    registry: &ConnectionRegistry,
    agent_id: Uuid,
) -> mpsc::UnboundedReceiver<String> {
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    registry.register(agent_id, AgentConnection { tx }).await;
    rx
}

// ---------------------------------------------------------------------------
// ManualStrategy: trigger_workflow() via channel
// ---------------------------------------------------------------------------

/// Start a Manual workflow and trigger it via `trigger_workflow()`.
/// The dispatch record should be created with `Pending` status (channel path).
#[tokio::test]
async fn manual_workflow_trigger_creates_pending_dispatch() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let agent_id = Uuid::new_v4();
    // Register a mock agent so the scheduler can find it.
    let _agent_rx = register_mock_agent(&registry, agent_id).await;

    let workflow = make_workflow(agent_id, TriggerConfig::Manual {});
    storage.add_workflow(&workflow).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let task = make_task("manual-trigger-1");
    let record = scheduler.trigger_workflow(&workflow.id, task).await.unwrap();

    assert_eq!(record.workflow_id, workflow.id);
    assert_eq!(record.agent_id, agent_id);
    assert_eq!(record.status, DispatchStatus::Pending);
    assert_eq!(record.source_id, "manual-trigger-1");

    // Verify the record was persisted.
    let dispatches = storage.list_dispatches(&workflow.id).await.unwrap();
    assert_eq!(dispatches.len(), 1);
    assert_eq!(dispatches[0].status, DispatchStatus::Pending);
}

// ---------------------------------------------------------------------------
// Direct dispatch: non-Manual trigger type
// ---------------------------------------------------------------------------

/// For a non-Manual workflow, `trigger_workflow()` dispatches directly to the
/// agent. The dispatch record should be created with `Dispatched` status.
#[tokio::test]
async fn non_manual_workflow_trigger_creates_dispatched_record() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let agent_id = Uuid::new_v4();
    // Register a mock agent so the direct-dispatch path can send the prompt.
    let _agent_rx = register_mock_agent(&registry, agent_id).await;

    let trigger = TriggerConfig::GithubIssues {
        owner: "org".to_string(),
        repo: "repo".to_string(),
        labels: vec![],
        state: "open".to_string(),
    };
    let workflow = make_workflow(agent_id, trigger);
    storage.add_workflow(&workflow).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));

    // No need to start_workflow for direct dispatch — trigger_workflow handles it.
    let task = make_task("issue-42");
    let record = scheduler.trigger_workflow(&workflow.id, task).await.unwrap();

    assert_eq!(record.workflow_id, workflow.id);
    assert_eq!(record.agent_id, agent_id);
    assert_eq!(record.status, DispatchStatus::Dispatched);
    assert_eq!(record.source_id, "issue-42");
    assert!(record.prompt_sent.contains("Task issue-42"));

    // Verify the record was persisted.
    let dispatches = storage.list_dispatches(&workflow.id).await.unwrap();
    assert_eq!(dispatches.len(), 1);
    assert_eq!(dispatches[0].status, DispatchStatus::Dispatched);
}

// ---------------------------------------------------------------------------
// trigger_workflow() requires the agent to be connected (direct path)
// ---------------------------------------------------------------------------

/// Triggering a non-Manual workflow when the agent is not connected should fail.
#[tokio::test]
async fn trigger_workflow_fails_when_agent_not_connected() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let agent_id = Uuid::new_v4();
    // Do NOT register the agent — no connection.

    let trigger = TriggerConfig::GithubIssues {
        owner: "org".to_string(),
        repo: "repo".to_string(),
        labels: vec![],
        state: "open".to_string(),
    };
    let workflow = make_workflow(agent_id, trigger);
    storage.add_workflow(&workflow).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));

    let task = make_task("issue-unconnected");
    let result = scheduler.trigger_workflow(&workflow.id, task).await;

    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("not connected"), "Expected 'not connected' in error: {msg}");
}

// ---------------------------------------------------------------------------
// trigger_workflow() requires the workflow to be enabled
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trigger_workflow_fails_for_disabled_workflow() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let agent_id = Uuid::new_v4();

    let mut workflow = make_workflow(agent_id, TriggerConfig::Manual {});
    workflow.enabled = false;
    storage.add_workflow(&workflow).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));
    let task = make_task("disabled-1");
    let result = scheduler.trigger_workflow(&workflow.id, task).await;

    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("not enabled"), "Expected 'not enabled' in error: {msg}");
}

// ---------------------------------------------------------------------------
// notify_task_complete() updates dispatch status
// ---------------------------------------------------------------------------

/// After direct dispatch, calling `notify_task_complete(is_error=false)` should
/// update the dispatch record to `Completed`.
///
/// `trigger_workflow()` only populates the runner's busy state when a runner is
/// registered for the workflow (see `Scheduler::trigger_workflow` direct path).
/// We therefore call `start_workflow()` first so the runner entry exists,
/// enabling `notify_task_complete()` to locate and clear it.
#[tokio::test]
async fn notify_task_complete_marks_dispatched_record_completed() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let agent_id = Uuid::new_v4();
    let _agent_rx = register_mock_agent(&registry, agent_id).await;

    // GithubIssues trigger: runner polls at 60s interval (no immediate I/O),
    // so it safely blocks in the background while the test runs.
    let workflow = make_workflow(
        agent_id,
        TriggerConfig::GithubIssues {
            owner: "org".to_string(),
            repo: "repo".to_string(),
            labels: vec![],
            state: "open".to_string(),
        },
    );
    storage.add_workflow(&workflow).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));

    // Start the runner so the busy-state slot is registered.
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    // trigger_workflow takes the direct path (GithubIssues, not Manual),
    // creates a Dispatched record, and sets the runner's busy state.
    let task = make_task("issue-notify-complete");
    let record = scheduler.trigger_workflow(&workflow.id, task).await.unwrap();
    assert_eq!(record.status, DispatchStatus::Dispatched);

    // Simulate agent sending a "result" message (no error).
    scheduler.notify_task_complete(agent_id, false).await;

    // The dispatch record should now be Completed.
    let dispatches = storage.list_dispatches(&workflow.id).await.unwrap();
    assert_eq!(dispatches.len(), 1);
    assert_eq!(dispatches[0].status, DispatchStatus::Completed);
    assert!(dispatches[0].completed_at.is_some());
}

/// After direct dispatch, calling `notify_task_complete(is_error=true)` should
/// update the dispatch record to `Failed`.
#[tokio::test]
async fn notify_task_complete_marks_dispatched_record_failed() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let agent_id = Uuid::new_v4();
    let _agent_rx = register_mock_agent(&registry, agent_id).await;

    let workflow = make_workflow(
        agent_id,
        TriggerConfig::GithubIssues {
            owner: "org".to_string(),
            repo: "repo".to_string(),
            labels: vec![],
            state: "open".to_string(),
        },
    );
    storage.add_workflow(&workflow).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));

    // Start the runner so trigger_workflow can set the runner's busy state.
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let task = make_task("issue-notify-failed");
    let record = scheduler.trigger_workflow(&workflow.id, task).await.unwrap();
    assert_eq!(record.status, DispatchStatus::Dispatched);

    // Simulate agent returning an error result.
    scheduler.notify_task_complete(agent_id, true).await;

    let dispatches = storage.list_dispatches(&workflow.id).await.unwrap();
    assert_eq!(dispatches.len(), 1);
    assert_eq!(dispatches[0].status, DispatchStatus::Failed);
    assert!(dispatches[0].completed_at.is_some());
}

/// `notify_task_complete()` for an agent with no active dispatch is a no-op.
#[tokio::test]
async fn notify_task_complete_noop_for_unknown_agent() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));
    // Should not panic or error even with no running workflows.
    scheduler.notify_task_complete(Uuid::new_v4(), false).await;
}

// ---------------------------------------------------------------------------
// Concurrent workflows with different trigger types
// ---------------------------------------------------------------------------

/// Two workflows (Manual and direct-dispatch) can run concurrently against
/// different agents without interfering with each other's dispatch records.
#[tokio::test]
async fn concurrent_workflows_different_trigger_types() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();

    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();
    let _rx_a = register_mock_agent(&registry, agent_a).await;
    let _rx_b = register_mock_agent(&registry, agent_b).await;

    // Workflow A: Manual trigger (uses channel path).
    let mut wf_a = make_workflow(agent_a, TriggerConfig::Manual {});
    wf_a.name = "workflow-a-manual".to_string();
    storage.add_workflow(&wf_a).await.unwrap();

    // Workflow B: GithubIssues trigger (uses direct dispatch path).
    let mut wf_b = make_workflow(
        agent_b,
        TriggerConfig::GithubIssues {
            owner: "org".to_string(),
            repo: "repo".to_string(),
            labels: vec![],
            state: "open".to_string(),
        },
    );
    wf_b.name = "workflow-b-github".to_string();
    storage.add_workflow(&wf_b).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));

    // Start both runners so busy-state tracking works for notify_task_complete.
    scheduler.start_workflow(wf_a.clone()).await.unwrap();
    scheduler.start_workflow(wf_b.clone()).await.unwrap();

    // Trigger both workflows.
    let record_a =
        scheduler.trigger_workflow(&wf_a.id, make_task("manual-concurrent")).await.unwrap();
    let record_b =
        scheduler.trigger_workflow(&wf_b.id, make_task("github-concurrent")).await.unwrap();

    // Workflow A (Manual, with running runner) → Pending.
    assert_eq!(record_a.status, DispatchStatus::Pending);
    assert_eq!(record_a.agent_id, agent_a);

    // Workflow B (GithubIssues, direct dispatch) → Dispatched.
    assert_eq!(record_b.status, DispatchStatus::Dispatched);
    assert_eq!(record_b.agent_id, agent_b);

    // Each workflow has exactly one dispatch record — no cross-contamination.
    let dispatches_a = storage.list_dispatches(&wf_a.id).await.unwrap();
    let dispatches_b = storage.list_dispatches(&wf_b.id).await.unwrap();
    assert_eq!(dispatches_a.len(), 1);
    assert_eq!(dispatches_b.len(), 1);
    assert_ne!(dispatches_a[0].agent_id, dispatches_b[0].agent_id);

    // Completing agent B should not affect workflow A's records.
    scheduler.notify_task_complete(agent_b, false).await;

    let dispatches_a_after = storage.list_dispatches(&wf_a.id).await.unwrap();
    assert_eq!(dispatches_a_after[0].status, DispatchStatus::Pending);

    let dispatches_b_after = storage.list_dispatches(&wf_b.id).await.unwrap();
    assert_eq!(dispatches_b_after[0].status, DispatchStatus::Completed);
}

// ---------------------------------------------------------------------------
// start_workflow() prevents duplicate runners
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start_workflow_rejects_duplicate() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let agent_id = Uuid::new_v4();

    let workflow = make_workflow(agent_id, TriggerConfig::Manual {});
    storage.add_workflow(&workflow).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    // Starting the same workflow again should fail.
    let result = scheduler.start_workflow(workflow.clone()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already running"));
}

// ---------------------------------------------------------------------------
// stop_workflow() removes the runner
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stop_workflow_removes_runner() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let agent_id = Uuid::new_v4();

    let workflow = make_workflow(agent_id, TriggerConfig::Manual {});
    storage.add_workflow(&workflow).await.unwrap();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let running = scheduler.running_workflows().await;
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].0, workflow.id);

    scheduler.stop_workflow(&workflow.id).await.unwrap();

    let running_after = scheduler.running_workflows().await;
    assert!(running_after.is_empty());
}

// ---------------------------------------------------------------------------
// Workflow not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trigger_workflow_fails_for_nonexistent_workflow() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();
    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));

    let result = scheduler.trigger_workflow(&Uuid::new_v4(), make_task("ghost")).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}
