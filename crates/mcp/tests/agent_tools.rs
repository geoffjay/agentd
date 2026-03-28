//! Integration tests for agent inspection tools.

mod common;
use common::{mock_orchestrator_server, test_client};

use agentd_mcp::tools::agents;

// ---------------------------------------------------------------------------
// list_agents
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_agents_all() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_list_agents(&client, None).await;

    assert!(result.contains("## Agents"), "expected heading: {result}");
    assert!(result.contains("test-agent-running"), "expected running agent: {result}");
    assert!(result.contains("test-agent-failed"), "expected failed agent: {result}");
    assert!(result.contains("🟢"), "expected running icon: {result}");
    assert!(result.contains("🔴"), "expected failed icon: {result}");
}

#[tokio::test]
async fn test_list_agents_filter_running() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_list_agents(&client, Some("running")).await;

    assert!(result.contains("test-agent-running"), "expected running agent: {result}");
    assert!(!result.contains("test-agent-failed"), "should not contain failed agent: {result}");
}

#[tokio::test]
async fn test_list_agents_filter_failed() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_list_agents(&client, Some("failed")).await;

    assert!(result.contains("test-agent-failed"), "expected failed agent: {result}");
    assert!(!result.contains("test-agent-running"), "should not contain running agent: {result}");
}

#[tokio::test]
async fn test_list_agents_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_list_agents(&client, None).await;

    assert!(result.contains("Error"), "expected error message: {result}");
    assert!(result.contains("unreachable"), "expected unreachable: {result}");
}

// ---------------------------------------------------------------------------
// get_agent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_agent_found() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_get_agent(&client, "aaaaaaaa-0000-0000-0000-000000000001").await;

    assert!(result.contains("test-agent-running"), "expected agent name: {result}");
    assert!(result.contains("running"), "expected status: {result}");
    assert!(result.contains("claude-sonnet-4-5"), "expected model: {result}");
    assert!(result.contains("AllowAll"), "expected tool policy: {result}");
}

#[tokio::test]
async fn test_get_agent_not_found() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_get_agent(&client, "00000000-0000-0000-0000-000000000000").await;

    assert!(result.contains("Error"), "expected error: {result}");
    assert!(result.contains("not found"), "expected not found: {result}");
}

#[tokio::test]
async fn test_get_agent_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_get_agent(&client, "aaaaaaaa-0000-0000-0000-000000000001").await;

    assert!(result.contains("Error"), "expected error: {result}");
}

// ---------------------------------------------------------------------------
// get_agent_status_summary
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_agent_status_summary() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_get_agent_status_summary(&client).await;

    assert!(result.contains("Fleet Status"), "expected fleet heading: {result}");
    assert!(result.contains("🟢 Running"), "expected running count: {result}");
    assert!(result.contains("🔴 Failed"), "expected failed count: {result}");
    // Should list the failed agent
    assert!(result.contains("test-agent-failed"), "expected failed agent name: {result}");
}

#[tokio::test]
async fn test_get_agent_status_summary_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = agents::run_get_agent_status_summary(&client).await;

    assert!(result.contains("Error"), "expected error: {result}");
}
