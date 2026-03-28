//! Integration tests for workflow and dispatch inspection tools.

mod common;
use common::{mock_orchestrator_server, test_client};

use agentd_mcp::tools::workflows;

// ---------------------------------------------------------------------------
// list_workflows
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_workflows() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = workflows::run_list_workflows(&client).await;

    assert!(result.contains("## Workflows"), "expected heading: {result}");
    assert!(result.contains("test-workflow"), "expected workflow name: {result}");
    assert!(result.contains("github_issues"), "expected trigger type: {result}");
    assert!(result.contains("✅"), "expected enabled icon: {result}");
}

#[tokio::test]
async fn test_list_workflows_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = workflows::run_list_workflows(&client).await;

    assert!(result.contains("Error"), "expected error: {result}");
    assert!(result.contains("unreachable"), "expected unreachable: {result}");
}

// ---------------------------------------------------------------------------
// get_workflow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_workflow_found() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = workflows::run_get_workflow(&client, "cccccccc-0000-0000-0000-000000000003").await;

    assert!(result.contains("test-workflow"), "expected workflow name: {result}");
    assert!(result.contains("github_issues"), "expected trigger type: {result}");
    assert!(result.contains("Fix issue"), "expected prompt template: {result}");
    assert!(result.contains("allow_all"), "expected tool policy: {result}");
}

#[tokio::test]
async fn test_get_workflow_not_found() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = workflows::run_get_workflow(&client, "00000000-0000-0000-0000-000000000000").await;

    assert!(result.contains("Error"), "expected error: {result}");
    assert!(result.contains("not found"), "expected not found: {result}");
}

#[tokio::test]
async fn test_get_workflow_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = workflows::run_get_workflow(&client, "cccccccc-0000-0000-0000-000000000003").await;

    assert!(result.contains("Error"), "expected error: {result}");
}

// ---------------------------------------------------------------------------
// list_dispatches
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_dispatches() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result =
        workflows::run_list_dispatches(&client, "cccccccc-0000-0000-0000-000000000003", None, None)
            .await;

    assert!(result.contains("Dispatch History"), "expected heading: {result}");
    assert!(result.contains("completed"), "expected completed status: {result}");
    assert!(result.contains("42"), "expected source ID: {result}");
}

#[tokio::test]
async fn test_list_dispatches_filter_by_status_match() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    // completed filter matches the mock dispatch
    let result = workflows::run_list_dispatches(
        &client,
        "cccccccc-0000-0000-0000-000000000003",
        Some("completed"),
        None,
    )
    .await;
    assert!(result.contains("completed"), "expected completed: {result}");
}

#[tokio::test]
async fn test_list_dispatches_filter_by_status_no_match() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    // failed filter — no failed dispatches in mock
    let result = workflows::run_list_dispatches(
        &client,
        "cccccccc-0000-0000-0000-000000000003",
        Some("failed"),
        None,
    )
    .await;
    assert!(result.contains("No dispatch records found"), "expected empty: {result}");
}

#[tokio::test]
async fn test_list_dispatches_workflow_not_found() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result =
        workflows::run_list_dispatches(&client, "00000000-0000-0000-0000-000000000000", None, None)
            .await;

    assert!(result.contains("Error"), "expected error: {result}");
    assert!(result.contains("not found"), "expected not found: {result}");
}

// ---------------------------------------------------------------------------
// get_failed_dispatches
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_failed_dispatches_none() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    // Mock only has completed dispatches → no failures
    let result = workflows::run_get_failed_dispatches(&client, None).await;

    assert!(result.contains("✅"), "expected no failures: {result}");
    assert!(result.contains("No failed dispatches"), "expected no failures message: {result}");
}

#[tokio::test]
async fn test_get_failed_dispatches_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = workflows::run_get_failed_dispatches(&client, None).await;

    assert!(result.contains("Error"), "expected error: {result}");
}
