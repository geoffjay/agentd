//! Integration tests for agent/workflow creation and management tools.

mod common;
use common::{mock_orchestrator_server, test_client};

use agentd_mcp::tools::creation;
use serde_json::json;

// ---------------------------------------------------------------------------
// create_agent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_agent_success() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = creation::run_create_agent(
        &client,
        "new-agent",
        "/tmp/work",
        Some("sonnet"),
        None,
        false,
        None,
        Some("allow_list"),
        Some(vec!["Read".to_string()]),
        Some(vec!["system".to_string()]),
    )
    .await;

    assert!(result.contains("✅"), "expected success: {result}");
    assert!(result.contains("11111111-0000-0000-0000-000000000099"), "new id: {result}");
    assert!(result.contains("diagnose_agent"), "follow-up hint: {result}");
}

#[tokio::test]
async fn test_create_agent_invalid_policy_mode_fails_client_side() {
    // Service deliberately unreachable: validation must reject before any call.
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = creation::run_create_agent(
        &client,
        "x",
        "/tmp",
        None,
        None,
        false,
        None,
        Some("bogus_mode"),
        None,
        None,
    )
    .await;

    assert!(result.contains("Unknown policy mode"), "{result}");
}

#[tokio::test]
async fn test_create_agent_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result =
        creation::run_create_agent(&client, "x", "/tmp", None, None, false, None, None, None, None)
            .await;

    assert!(result.contains("🔴"), "expected failure message: {result}");
}

// ---------------------------------------------------------------------------
// create_workflow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_workflow_success() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = creation::run_create_workflow(
        &client,
        "nightly-report",
        "aaaaaaaa-0000-0000-0000-000000000001",
        json!({"type": "cron", "expression": "0 9 * * MON-FRI"}),
        "Summarize overnight activity",
        None,
        None,
        None,
        None,
    )
    .await;

    assert!(result.contains("✅"), "expected success: {result}");
    assert!(result.contains("22222222-0000-0000-0000-000000000077"), "new id: {result}");
    assert!(result.contains("trigger_workflow"), "smoke-test hint: {result}");
}

#[tokio::test]
async fn test_create_workflow_unknown_trigger_rejected_client_side() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = creation::run_create_workflow(
        &client,
        "wf",
        "aaaaaaaa-0000-0000-0000-000000000001",
        json!({"type": "carrier_pigeon"}),
        "template",
        None,
        None,
        None,
        None,
    )
    .await;

    assert!(result.contains("carrier_pigeon"), "{result}");
    assert!(result.contains("cron"), "lists valid types: {result}");
}

// ---------------------------------------------------------------------------
// update / enable / trigger / delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_set_workflow_enabled() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result =
        creation::run_set_workflow_enabled(&client, "cccccccc-0000-0000-0000-000000000003", false)
            .await;

    assert!(result.contains("disabled"), "{result}");
}

#[tokio::test]
async fn test_update_workflow_rename() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = creation::run_update_workflow(
        &client,
        "cccccccc-0000-0000-0000-000000000003",
        Some("renamed"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    assert!(result.contains("✅"), "{result}");
    assert!(result.contains("renamed"), "{result}");
}

#[tokio::test]
async fn test_trigger_workflow() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = creation::run_trigger_workflow(
        &client,
        "cccccccc-0000-0000-0000-000000000003",
        Some("manual smoke test"),
        None,
        None,
        None,
        None,
    )
    .await;

    assert!(result.contains("✅"), "{result}");
    assert!(result.contains("99999999-0000-0000-0000-000000000042"), "dispatch id: {result}");
}

#[tokio::test]
async fn test_delete_workflow() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result =
        creation::run_delete_workflow(&client, "cccccccc-0000-0000-0000-000000000003").await;
    assert!(result.contains("✅"), "{result}");

    let result =
        creation::run_delete_workflow(&client, "00000000-0000-0000-0000-000000000000").await;
    assert!(result.contains("🔴"), "missing workflow should fail: {result}");
}
