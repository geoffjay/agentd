//! Integration tests for service health and system metrics tools.

mod common;
use common::{mock_monitor_server, mock_orchestrator_server, test_client};

use agentd_mcp::tools::health;

// ---------------------------------------------------------------------------
// check_service_health
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_check_service_health_reachable() {
    let orch = mock_orchestrator_server().await;
    let monitor = mock_monitor_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", &monitor.url());

    let result = health::run_check_service_health(&client).await;

    assert!(result.contains("orchestrator"), "expected orchestrator row: {result}");
    assert!(result.contains("✅"), "expected healthy icon: {result}");
}

#[tokio::test]
async fn test_check_service_health_unreachable() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = health::run_check_service_health(&client).await;

    assert!(result.contains("orchestrator"), "expected orchestrator row: {result}");
    assert!(result.contains("❌"), "expected unreachable icon: {result}");
}

// ---------------------------------------------------------------------------
// check_single_service
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_check_single_service_orchestrator_up() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = health::run_check_single_service(&client, "orchestrator").await;

    assert!(result.contains("✅"), "expected healthy: {result}");
    assert!(result.contains("orchestrator"), "expected service name: {result}");
}

#[tokio::test]
async fn test_check_single_service_unknown_name() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = health::run_check_single_service(&client, "nonexistent").await;

    assert!(result.contains("Unknown service"), "expected unknown service error: {result}");
}

#[tokio::test]
async fn test_check_single_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = health::run_check_single_service(&client, "orchestrator").await;

    assert!(result.contains("❌"), "expected unreachable: {result}");
}

// ---------------------------------------------------------------------------
// get_system_metrics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_system_metrics_available() {
    let monitor = mock_monitor_server().await;
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", &monitor.url());

    let result = health::run_get_system_metrics(&client).await;

    assert!(result.contains("CPU"), "expected CPU info: {result}");
    assert!(result.contains("Memory"), "expected memory info: {result}");
}

#[tokio::test]
async fn test_get_system_metrics_unavailable() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = health::run_get_system_metrics(&client).await;

    // Should degrade gracefully, not panic
    assert!(!result.is_empty(), "expected non-empty response: {result}");
}

// ---------------------------------------------------------------------------
// get_prometheus_metrics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_prometheus_metrics_orchestrator() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = health::run_get_prometheus_metrics(&client, Some("orchestrator")).await;

    // Should contain metric data or a clear error
    assert!(!result.is_empty(), "expected non-empty response: {result}");
}

#[tokio::test]
async fn test_get_prometheus_metrics_default() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    // None defaults to orchestrator
    let result = health::run_get_prometheus_metrics(&client, None).await;

    assert!(!result.is_empty(), "expected non-empty response: {result}");
}

#[tokio::test]
async fn test_get_prometheus_metrics_unknown_service_defaults_to_orchestrator() {
    let orch = mock_orchestrator_server().await;
    let client = test_client(&orch.url(), "http://127.0.0.1:1", "http://127.0.0.1:1");

    // Unknown service names fall through to the orchestrator default (by design)
    let result = health::run_get_prometheus_metrics(&client, Some("invalid")).await;

    assert!(!result.is_empty(), "expected non-empty response: {result}");
    assert!(result.contains("orchestrator"), "expected orchestrator metrics: {result}");
}
