//! Integration tests for notification inspection and management tools.

mod common;
use common::{mock_notify_server, test_client};

use agentd_mcp::tools::notifications;

// ---------------------------------------------------------------------------
// list_notifications
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_notifications_all() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    let result = notifications::run_list_notifications(&client, None, None, None).await;

    assert!(result.contains("## Notifications"), "expected heading: {result}");
    assert!(result.contains("Test Notification"), "expected notification title: {result}");
    assert!(result.contains("📬"), "expected pending icon: {result}");
}

#[tokio::test]
async fn test_list_notifications_filter_by_status() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    // Pending status → returns the mock notification
    let result = notifications::run_list_notifications(&client, Some("pending"), None, None).await;
    assert!(result.contains("Test Notification"), "expected notification: {result}");

    // Dismissed status → returns empty
    let result =
        notifications::run_list_notifications(&client, Some("dismissed"), None, None).await;
    assert!(result.contains("No notifications"), "expected empty: {result}");
}

#[tokio::test]
async fn test_list_notifications_filter_by_priority() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    // high priority matches the mock notification
    let result = notifications::run_list_notifications(&client, None, Some("high"), None).await;
    assert!(result.contains("Test Notification"), "expected high priority notification: {result}");

    // low priority → filtered out
    let result = notifications::run_list_notifications(&client, None, Some("low"), None).await;
    assert!(result.contains("No notifications"), "expected empty for low priority: {result}");
}

#[tokio::test]
async fn test_list_notifications_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = notifications::run_list_notifications(&client, None, None, None).await;

    assert!(result.contains("Error"), "expected error: {result}");
    assert!(result.contains("unreachable"), "expected unreachable: {result}");
}

// ---------------------------------------------------------------------------
// get_notification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_notification_found() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    let result =
        notifications::run_get_notification(&client, "dddddddd-0000-0000-0000-000000000004").await;

    assert!(result.contains("Test Notification"), "expected title: {result}");
    assert!(result.contains("high"), "expected priority: {result}");
    assert!(result.contains("System"), "expected source: {result}");
    assert!(result.contains("This is a test notification"), "expected body: {result}");
}

#[tokio::test]
async fn test_get_notification_not_found() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    let result =
        notifications::run_get_notification(&client, "00000000-0000-0000-0000-000000000000").await;

    assert!(result.contains("Error"), "expected error: {result}");
    assert!(result.contains("not found"), "expected not found: {result}");
}

#[tokio::test]
async fn test_get_notification_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result =
        notifications::run_get_notification(&client, "dddddddd-0000-0000-0000-000000000004").await;

    assert!(result.contains("Error"), "expected error: {result}");
}

// ---------------------------------------------------------------------------
// get_actionable_notifications
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_actionable_notifications() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    let result = notifications::run_get_actionable_notifications(&client).await;

    assert!(result.contains("Actionable Notifications"), "expected heading: {result}");
    assert!(result.contains("Test Notification"), "expected notification: {result}");
}

#[tokio::test]
async fn test_get_actionable_notifications_service_down() {
    let client = test_client("http://127.0.0.1:1", "http://127.0.0.1:1", "http://127.0.0.1:1");

    let result = notifications::run_get_actionable_notifications(&client).await;

    assert!(result.contains("Error"), "expected error: {result}");
}

// ---------------------------------------------------------------------------
// create_notification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_notification() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    let result = notifications::run_create_notification(
        &client,
        "Diagnostic Alert",
        "System detected a potential issue.",
        Some("high"),
    )
    .await;

    assert!(result.contains("✅"), "expected success: {result}");
    assert!(result.contains("created"), "expected created: {result}");
}

#[tokio::test]
async fn test_create_notification_default_priority() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    let result = notifications::run_create_notification(
        &client,
        "Test",
        "Default priority notification.",
        None,
    )
    .await;

    assert!(result.contains("✅"), "expected success: {result}");
}

// ---------------------------------------------------------------------------
// dismiss_notification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dismiss_notification_success() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    let result =
        notifications::run_dismiss_notification(&client, "dddddddd-0000-0000-0000-000000000004")
            .await;

    assert!(result.contains("✅"), "expected success: {result}");
    assert!(result.contains("dismissed"), "expected dismissed: {result}");
}

#[tokio::test]
async fn test_dismiss_notification_not_found() {
    let notify = mock_notify_server().await;
    let client = test_client("http://127.0.0.1:1", &notify.url(), "http://127.0.0.1:1");

    let result =
        notifications::run_dismiss_notification(&client, "00000000-0000-0000-0000-000000000000")
            .await;

    assert!(result.contains("Error"), "expected error: {result}");
    assert!(result.contains("not found"), "expected not found: {result}");
}
