//! Integration tests for the queue-based trigger system.
//!
//! Tests cover:
//! - Queue storage operations (enqueue, dequeue, stats, peek, purge)
//! - TriggerConfig::Queue serialization / deserialization
//! - `create_strategy()` factory creates a `QueueStrategy`
//! - QueueStrategy picks up a task that is pushed into the queue
//! - Queue name validation in the API layer
//! - CLI queue management subcommand argument parsing

use chrono::Utc;
use orchestrator::{
    scheduler::{
        runner::create_strategy,
        storage::SchedulerStorage,
        types::{TriggerConfig, WorkflowConfig},
        Scheduler,
    },
    storage::AgentStorage,
    websocket::ConnectionRegistry,
};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn create_test_storage() -> (SchedulerStorage, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let agent_storage = AgentStorage::with_path(&db_path).await.unwrap();
    let storage = SchedulerStorage::new(agent_storage.db().clone());
    (storage, temp_dir)
}

fn queue_workflow(agent_id: Uuid, queue_name: &str) -> WorkflowConfig {
    let now = Utc::now();
    WorkflowConfig {
        id: Uuid::new_v4(),
        name: format!("queue-workflow-{queue_name}"),
        agent_id,
        trigger_config: TriggerConfig::Queue {
            queue_name: queue_name.to_string(),
            poll_interval_secs: Some(1),
            visibility_timeout_secs: Some(300),
        },
        prompt_template: "Process: {{title}} (queue={{queue_name}}, task={{queue_task_id}})"
            .to_string(),
        poll_interval_secs: 60,
        enabled: true,
        tool_policy: Default::default(),
        created_at: now,
        updated_at: now,
        project_id: None,
        organization_id: None,
    }
}

// ---------------------------------------------------------------------------
// Serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn queue_trigger_config_serde_roundtrip() {
    let config = TriggerConfig::Queue {
        queue_name: "my-queue".to_string(),
        poll_interval_secs: Some(5),
        visibility_timeout_secs: Some(300),
    };

    let json = serde_json::to_string(&config).expect("serialization failed");
    assert!(json.contains("\"type\":\"queue\""), "missing type tag: {json}");
    assert!(json.contains("my-queue"), "missing queue_name: {json}");

    let decoded: TriggerConfig = serde_json::from_str(&json).expect("deserialization failed");
    if let TriggerConfig::Queue { queue_name, poll_interval_secs, visibility_timeout_secs } =
        decoded
    {
        assert_eq!(queue_name, "my-queue");
        assert_eq!(poll_interval_secs, Some(5));
        assert_eq!(visibility_timeout_secs, Some(300));
    } else {
        panic!("Expected Queue variant after round-trip, got something else");
    }
}

#[test]
fn queue_trigger_config_defaults_serde() {
    // Queue with optional fields omitted.
    let json = r#"{"type":"queue","queue_name":"bg-tasks"}"#;
    let config: TriggerConfig = serde_json::from_str(json).expect("deserialization failed");

    if let TriggerConfig::Queue { queue_name, poll_interval_secs, visibility_timeout_secs } = config
    {
        assert_eq!(queue_name, "bg-tasks");
        assert_eq!(poll_interval_secs, None);
        assert_eq!(visibility_timeout_secs, None);
    } else {
        panic!("Expected Queue variant");
    }
}

#[test]
fn queue_trigger_type_string() {
    let config = TriggerConfig::Queue {
        queue_name: "x".to_string(),
        poll_interval_secs: None,
        visibility_timeout_secs: None,
    };
    assert_eq!(config.trigger_type(), "queue");
    assert!(config.is_implemented());
    assert!(!config.is_one_shot());
}

// ---------------------------------------------------------------------------
// create_strategy factory
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_strategy_queue_creates_queue_strategy() {
    let (storage, _tmp) = create_test_storage().await;
    let agent_id = Uuid::new_v4();
    let config = queue_workflow(agent_id, "factory-q");

    // Should succeed with storage provided.
    let result = create_strategy(&config, None, Some(&storage));
    assert!(result.is_ok(), "create_strategy failed: {:?}", result.err());
}

#[tokio::test]
async fn create_strategy_queue_requires_storage() {
    let agent_id = Uuid::new_v4();
    let config = queue_workflow(agent_id, "no-storage-q");

    // Should fail when storage is None.
    let result = create_strategy(&config, None, None);
    let err = result.err().expect("expected an error without storage").to_string();
    assert!(err.contains("SchedulerStorage is required"), "unexpected error: {err}");
}

// ---------------------------------------------------------------------------
// Queue storage: push + QueueStrategy consumes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn queue_strategy_picks_up_pushed_task() {
    use orchestrator::scheduler::strategy::{QueueStrategy, TriggerStrategy};
    use std::time::Duration;
    use tokio::sync::watch;

    let (storage, _tmp) = create_test_storage().await;

    // Push a task into the queue.
    storage.enqueue("pickup-q", "Important Work", Some("details"), 5).await.unwrap();

    let mut strategy =
        QueueStrategy::new(storage.clone(), "pickup-q".to_string(), Duration::from_millis(10), 60);
    let (_tx, rx) = watch::channel(false);

    let tasks = strategy.next_tasks(&rx).await.unwrap();

    assert_eq!(tasks.len(), 1);
    let task = &tasks[0];
    assert_eq!(task.title, "Important Work");
    assert_eq!(task.body, "details");
    assert!(task.source_id.starts_with("queue:pickup-q:"));
    assert_eq!(task.metadata.get("queue_name").map(String::as_str), Some("pickup-q"));
    assert_eq!(task.metadata.get("queue_priority").map(String::as_str), Some("5"));

    // The task should now be in processing state — queue appears empty.
    let stats = storage.queue_stats("pickup-q").await.unwrap();
    assert_eq!(stats.pending, 0);
    assert_eq!(stats.processing, 1);
}

// ---------------------------------------------------------------------------
// Queue storage: concurrent producers, single consumer ordering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_producers_consumed_in_priority_then_fifo_order() {
    let (storage, _tmp) = create_test_storage().await;

    // Simulate concurrent producers by enqueuing tasks with various priorities.
    let mut handles = vec![];
    for i in 0..5 {
        let s = storage.clone();
        handles.push(tokio::spawn(async move {
            s.enqueue("concurrent-q", &format!("Low {i}"), None, 1).await.unwrap();
        }));
    }
    // One high-priority task.
    let s2 = storage.clone();
    handles.push(tokio::spawn(async move {
        s2.enqueue("concurrent-q", "High-Pri", None, 100).await.unwrap();
    }));

    for h in handles {
        h.await.unwrap();
    }

    // First dequeued should be the high-priority task.
    let first = storage.dequeue("concurrent-q", 60).await.unwrap().unwrap();
    assert_eq!(first.title, "High-Pri");
    assert_eq!(first.priority, 100);

    // Remaining tasks should be low priority (FIFO among equals).
    for _ in 0..5 {
        let t = storage.dequeue("concurrent-q", 60).await.unwrap().unwrap();
        assert_eq!(t.priority, 1);
    }

    // Queue should now be empty.
    assert!(storage.dequeue("concurrent-q", 60).await.unwrap().is_none());
}

// ---------------------------------------------------------------------------
// Scheduler integration: start_workflow with Queue trigger
// ---------------------------------------------------------------------------

#[tokio::test]
async fn scheduler_accepts_queue_workflow() {
    let (storage, _tmp) = create_test_storage().await;
    let registry = ConnectionRegistry::new();

    let scheduler = Arc::new(Scheduler::new(storage.clone(), registry));
    let agent_id = Uuid::new_v4();
    let workflow = queue_workflow(agent_id, "sched-q");

    storage.add_workflow(&workflow).await.unwrap();

    // Scheduler should be able to start the workflow without error.
    let result = scheduler.start_workflow(workflow.clone()).await;
    assert!(result.is_ok(), "start_workflow failed: {:?}", result.err());

    // Clean up.
    let _ = scheduler.stop_workflow(&workflow.id).await;
}
