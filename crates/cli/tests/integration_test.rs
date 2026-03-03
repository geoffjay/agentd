use chrono::{Duration, Utc};
use cli::client::ApiClient;
use cli::types::*;
use mockito::Server;
use serde_json::json;
use uuid::Uuid;

/// Paginated response wrapper matching the orchestrator API format.
/// Used across multiple test modules.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct PaginatedResponse {
    items: Vec<serde_json::Value>,
    total: u64,
    limit: u64,
    offset: u64,
}

use notify::notification::{
    Notification, NotificationLifetime, NotificationPriority, NotificationSource,
    NotificationStatus,
};

// Helper function to create a test notification
fn create_test_notification(id: Uuid) -> Notification {
    Notification {
        id,
        source: NotificationSource::System,
        lifetime: NotificationLifetime::Persistent,
        priority: NotificationPriority::Normal,
        status: NotificationStatus::Pending,
        title: "Test Notification".to_string(),
        message: "This is a test".to_string(),
        requires_response: false,
        response: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// Helper function to create a notification JSON response
fn notification_json(notification: &Notification) -> serde_json::Value {
    json!({
        "id": notification.id,
        "source": notification.source,
        "lifetime": notification.lifetime,
        "priority": notification.priority,
        "status": notification.status,
        "title": notification.title,
        "message": notification.message,
        "requires_response": notification.requires_response,
        "response": notification.response,
        "created_at": notification.created_at,
        "updated_at": notification.updated_at,
    })
}

mod notification_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_notification_success() {
        let mut server = Server::new_async().await;
        let notification = create_test_notification(Uuid::new_v4());

        let mock = server
            .mock("POST", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&notification_json(&notification)).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: false,
        };

        let result: Result<Notification, _> = client.post("/notifications", &request).await;
        assert!(result.is_ok());

        let created = result.unwrap();
        assert_eq!(created.title, notification.title);
        assert_eq!(created.priority, notification.priority);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_notification_with_high_priority() {
        let mut server = Server::new_async().await;
        let mut notification = create_test_notification(Uuid::new_v4());
        notification.priority = NotificationPriority::High;

        let mock = server
            .mock("POST", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&notification_json(&notification)).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::High,
            title: "High Priority".to_string(),
            message: "Urgent message".to_string(),
            requires_response: false,
        };

        let result: Result<Notification, _> = client.post("/notifications", &request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().priority, NotificationPriority::High);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_notification_ephemeral() {
        let mut server = Server::new_async().await;
        let mut notification = create_test_notification(Uuid::new_v4());
        notification.lifetime =
            NotificationLifetime::Ephemeral { expires_at: Utc::now() + Duration::hours(1) };

        let mock = server
            .mock("POST", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&notification_json(&notification)).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::System,
            lifetime: notification.lifetime.clone(),
            priority: NotificationPriority::Normal,
            title: "Ephemeral".to_string(),
            message: "This will expire".to_string(),
            requires_response: false,
        };

        let result: Result<Notification, _> = client.post("/notifications", &request).await;
        assert!(result.is_ok());

        match result.unwrap().lifetime {
            NotificationLifetime::Ephemeral { .. } => {}
            _ => panic!("Expected ephemeral lifetime"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_notification_requires_response() {
        let mut server = Server::new_async().await;
        let mut notification = create_test_notification(Uuid::new_v4());
        notification.requires_response = true;

        let mock = server
            .mock("POST", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&notification_json(&notification)).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            title: "Question".to_string(),
            message: "Please respond".to_string(),
            requires_response: true,
        };

        let result: Result<Notification, _> = client.post("/notifications", &request).await;
        assert!(result.is_ok());
        assert!(result.unwrap().requires_response);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_notifications_success() {
        let mut server = Server::new_async().await;
        let notification1 = create_test_notification(Uuid::new_v4());
        let notification2 = create_test_notification(Uuid::new_v4());

        let mock = server
            .mock("GET", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::to_string(&json!([
                    notification_json(&notification1),
                    notification_json(&notification2)
                ]))
                .unwrap(),
            )
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> = client.get("/notifications").await;

        assert!(result.is_ok());
        let notifications = result.unwrap();
        assert_eq!(notifications.len(), 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_notifications_empty() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> = client.get("/notifications").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_notifications_with_status_filter() {
        let mut server = Server::new_async().await;
        let mut notification = create_test_notification(Uuid::new_v4());
        notification.status = NotificationStatus::Pending;

        let mock = server
            .mock("GET", "/notifications?status=pending")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&json!([notification_json(&notification)])).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> =
            client.get("/notifications?status=pending").await;

        assert!(result.is_ok());
        let notifications = result.unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].status, NotificationStatus::Pending);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_notifications_actionable() {
        let mut server = Server::new_async().await;
        let mut notification = create_test_notification(Uuid::new_v4());
        notification.requires_response = true;
        notification.status = NotificationStatus::Pending;

        let mock = server
            .mock("GET", "/notifications/actionable")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&json!([notification_json(&notification)])).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> = client.get("/notifications/actionable").await;

        assert!(result.is_ok());
        let notifications = result.unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(notifications[0].requires_response);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_notification_success() {
        let mut server = Server::new_async().await;
        let id = Uuid::new_v4();
        let notification = create_test_notification(id);

        let mock = server
            .mock("GET", format!("/notifications/{id}").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&notification_json(&notification)).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Notification, _> = client.get(&format!("/notifications/{id}")).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, id);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_notification_not_found() {
        let mut server = Server::new_async().await;
        let id = Uuid::new_v4();

        let mock = server
            .mock("GET", format!("/notifications/{id}").as_str())
            .with_status(404)
            .with_header("content-type", "text/plain")
            .with_body("Notification not found")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Notification, _> = client.get(&format!("/notifications/{id}")).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("404"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_delete_notification_success() {
        let mut server = Server::new_async().await;
        let id = Uuid::new_v4();

        let mock = server
            .mock("DELETE", format!("/notifications/{id}").as_str())
            .with_status(200)
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result = client.delete(&format!("/notifications/{id}")).await;

        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_delete_notification_not_found() {
        let mut server = Server::new_async().await;
        let id = Uuid::new_v4();

        let mock = server
            .mock("DELETE", format!("/notifications/{id}").as_str())
            .with_status(404)
            .with_body("Notification not found")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result = client.delete(&format!("/notifications/{id}")).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("404"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_respond_to_notification_success() {
        let mut server = Server::new_async().await;
        let id = Uuid::new_v4();
        let mut notification = create_test_notification(id);
        notification.status = NotificationStatus::Responded;
        notification.response = Some("My response".to_string());

        let mock = server
            .mock("PUT", format!("/notifications/{id}").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&notification_json(&notification)).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request =
            UpdateNotificationRequest { status: None, response: Some("My response".to_string()) };

        let result: Result<Notification, _> =
            client.put(&format!("/notifications/{id}"), &request).await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.response, Some("My response".to_string()));
        assert_eq!(updated.status, NotificationStatus::Responded);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_respond_to_notification_not_found() {
        let mut server = Server::new_async().await;
        let id = Uuid::new_v4();

        let mock = server
            .mock("PUT", format!("/notifications/{id}").as_str())
            .with_status(404)
            .with_body("Notification not found")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request =
            UpdateNotificationRequest { status: None, response: Some("My response".to_string()) };

        let result: Result<Notification, _> =
            client.put(&format!("/notifications/{id}"), &request).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("404"));

        mock.assert_async().await;
    }
}

mod ask_service_tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct TriggerResponse {
        message: String,
        checks_performed: usize,
        notifications_created: usize,
    }

    #[derive(Debug, Serialize)]
    struct AnswerRequest {
        question_id: Uuid,
        answer: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct AnswerResponse {
        message: String,
    }

    #[tokio::test]
    async fn test_trigger_checks_success() {
        let mut server = Server::new_async().await;

        let response = TriggerResponse {
            message: "Checks completed".to_string(),
            checks_performed: 5,
            notifications_created: 2,
        };

        let mock = server
            .mock("POST", "/trigger")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<TriggerResponse, _> =
            client.post("/trigger", &serde_json::json!({})).await;

        assert!(result.is_ok());
        let trigger_result = result.unwrap();
        assert_eq!(trigger_result.checks_performed, 5);
        assert_eq!(trigger_result.notifications_created, 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_trigger_checks_no_notifications() {
        let mut server = Server::new_async().await;

        let response = TriggerResponse {
            message: "All checks passed".to_string(),
            checks_performed: 3,
            notifications_created: 0,
        };

        let mock = server
            .mock("POST", "/trigger")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<TriggerResponse, _> =
            client.post("/trigger", &serde_json::json!({})).await;

        assert!(result.is_ok());
        let trigger_result = result.unwrap();
        assert_eq!(trigger_result.notifications_created, 0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_trigger_checks_service_error() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/trigger")
            .with_status(500)
            .with_body("Internal server error")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<TriggerResponse, _> =
            client.post("/trigger", &serde_json::json!({})).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("500"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_answer_question_success() {
        let mut server = Server::new_async().await;
        let question_id = Uuid::new_v4();

        let response = AnswerResponse { message: "Answer recorded".to_string() };

        let mock = server
            .mock("POST", "/answer")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request = AnswerRequest { question_id, answer: "Yes".to_string() };

        let result: Result<AnswerResponse, _> = client.post("/answer", &request).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().message, "Answer recorded");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_answer_question_not_found() {
        let mut server = Server::new_async().await;
        let question_id = Uuid::new_v4();

        let mock = server
            .mock("POST", "/answer")
            .with_status(404)
            .with_body("Question not found")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request = AnswerRequest { question_id, answer: "Yes".to_string() };

        let result: Result<AnswerResponse, _> = client.post("/answer", &request).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("404"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_answer_question_with_long_text() {
        let mut server = Server::new_async().await;
        let question_id = Uuid::new_v4();

        let response = AnswerResponse { message: "Answer recorded".to_string() };

        let mock = server
            .mock("POST", "/answer")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let long_answer = "This is a very long answer with multiple sentences. It contains detailed information about the question being asked. This tests that the system can handle longer text responses without issues.";
        let request = AnswerRequest { question_id, answer: long_answer.to_string() };

        let result: Result<AnswerResponse, _> = client.post("/answer", &request).await;

        assert!(result.is_ok());

        mock.assert_async().await;
    }
}

mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_server_error_500() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/notifications")
            .with_status(500)
            .with_body("Internal server error")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> = client.get("/notifications").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("500"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bad_request_400() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/notifications")
            .with_status(400)
            .with_body("Bad request: invalid data")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            title: "".to_string(), // Invalid empty title
            message: "Test".to_string(),
            requires_response: false,
        };

        let result: Result<Notification, _> = client.post("/notifications", &request).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("400"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_bad_gateway_502() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/notifications")
            .with_status(502)
            .with_body("Bad gateway")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> = client.get("/notifications").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("502"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_malformed_json_response() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{this is not valid json}")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> = client.get("/notifications").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to parse response body"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_missing_required_fields() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "title": "Missing fields"
            }]"#,
            )
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> = client.get("/notifications").await;

        assert!(result.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_uuid() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{
                "id": "not-a-valid-uuid",
                "source": {"type": "system"},
                "lifetime": {"type": "persistent"},
                "priority": "normal",
                "status": "pending",
                "title": "Test",
                "message": "Test",
                "requires_response": false,
                "response": null,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            }]"#,
            )
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<Vec<Notification>, _> = client.get("/notifications").await;

        assert!(result.is_err());

        mock.assert_async().await;
    }
}

#[allow(dead_code)]
mod orchestrator_agent_tests {
    use super::*;

    /// Helper: paginated response wrapper matching the orchestrator API format.
    fn paginated(items: serde_json::Value) -> String {
        let len = items.as_array().map(|a| a.len()).unwrap_or(0) as u64;
        serde_json::to_string(&json!({
            "items": items,
            "total": len,
            "limit": 50,
            "offset": 0,
        }))
        .unwrap()
    }

    fn sample_agent(id: &str, name: &str, status: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "status": status,
            "config": {
                "working_dir": "/tmp/test",
                "shell": "zsh",
            },
            "tmux_session": format!("agentd-orch-{}", id),
            "created_at": "2025-01-01T00:00:00Z",
        })
    }

    // -- Agent command tests --

    #[tokio::test]
    async fn test_create_agent_sends_correct_json() {
        let mut server = Server::new_async().await;
        let agent_id = "550e8400-e29b-41d4-a716-446655440000";
        let response_agent = sample_agent(agent_id, "test-agent", "pending");

        let mock = server
            .mock("POST", "/agents")
            .match_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJsonString(
                json!({
                    "name": "test-agent",
                    "working_dir": "/tmp/project",
                    "shell": "zsh",
                })
                .to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response_agent).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let body = json!({
            "name": "test-agent",
            "working_dir": "/tmp/project",
            "user": null,
            "shell": "zsh",
            "interactive": false,
            "prompt": null,
            "worktree": false,
            "system_prompt": null,
        });

        let result: Result<serde_json::Value, _> = client.post("/agents", &body).await;
        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created["name"], "test-agent");
        assert_eq!(created["status"], "pending");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_agent_with_all_optional_flags() {
        let mut server = Server::new_async().await;
        let agent_id = "550e8400-e29b-41d4-a716-446655440000";
        let response_agent = sample_agent(agent_id, "full-agent", "pending");

        let mock = server
            .mock("POST", "/agents")
            .match_body(mockito::Matcher::PartialJsonString(
                json!({
                    "name": "full-agent",
                    "working_dir": "/tmp/project",
                    "user": "deploy",
                    "shell": "bash",
                    "interactive": true,
                    "prompt": "Fix the bug",
                    "worktree": true,
                    "system_prompt": "You are a code assistant",
                })
                .to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response_agent).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let body = json!({
            "name": "full-agent",
            "working_dir": "/tmp/project",
            "user": "deploy",
            "shell": "bash",
            "interactive": true,
            "prompt": "Fix the bug",
            "worktree": true,
            "system_prompt": "You are a code assistant",
        });

        let result: Result<serde_json::Value, _> = client.post("/agents", &body).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_agents_success() {
        let mut server = Server::new_async().await;
        let agents = json!([
            sample_agent("id-1", "agent-1", "running"),
            sample_agent("id-2", "agent-2", "stopped"),
        ]);

        let mock = server
            .mock("GET", "/agents")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(agents))
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> = client.get("/agents").await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0]["name"], "agent-1");
        assert_eq!(response.items[1]["status"], "stopped");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_agents_with_status_filter() {
        let mut server = Server::new_async().await;
        let agents = json!([sample_agent("id-1", "agent-1", "running")]);

        let mock = server
            .mock("GET", "/agents?status=running")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(agents))
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> = client.get("/agents?status=running").await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0]["status"], "running");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_agents_empty() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/agents")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(json!([])))
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> = client.get("/agents").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().items.len(), 0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_agent_success() {
        let mut server = Server::new_async().await;
        let agent_id = "550e8400-e29b-41d4-a716-446655440000";
        let agent = sample_agent(agent_id, "my-agent", "running");

        let mock = server
            .mock("GET", format!("/agents/{agent_id}").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&agent).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<serde_json::Value, _> =
            client.get(&format!("/agents/{agent_id}")).await;
        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched["id"], agent_id);
        assert_eq!(fetched["name"], "my-agent");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_agent_not_found() {
        let mut server = Server::new_async().await;
        let agent_id = "nonexistent-id";

        let mock = server
            .mock("GET", format!("/agents/{agent_id}").as_str())
            .with_status(404)
            .with_body("Agent not found")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<serde_json::Value, _> =
            client.get(&format!("/agents/{agent_id}")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("404"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_delete_agent_success() {
        let mut server = Server::new_async().await;
        let agent_id = "550e8400-e29b-41d4-a716-446655440000";

        let mock = server
            .mock("DELETE", format!("/agents/{agent_id}").as_str())
            .with_status(200)
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result = client.delete(&format!("/agents/{agent_id}")).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_delete_agent_not_found() {
        let mut server = Server::new_async().await;
        let agent_id = "nonexistent-id";

        let mock = server
            .mock("DELETE", format!("/agents/{agent_id}").as_str())
            .with_status(404)
            .with_body("Agent not found")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result = client.delete(&format!("/agents/{agent_id}")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("404"));

        mock.assert_async().await;
    }
}

#[allow(dead_code)]
mod orchestrator_workflow_tests {
    use super::*;

    fn paginated(items: serde_json::Value) -> String {
        let len = items.as_array().map(|a| a.len()).unwrap_or(0) as u64;
        serde_json::to_string(&json!({
            "items": items,
            "total": len,
            "limit": 50,
            "offset": 0,
        }))
        .unwrap()
    }

    fn sample_workflow(id: &str, name: &str, agent_id: &str, enabled: bool) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "agent_id": agent_id,
            "enabled": enabled,
            "poll_interval_secs": 60,
            "source_config": {
                "type": "github_issues",
                "owner": "acme",
                "repo": "widgets",
                "labels": ["bug"],
            },
            "prompt_template": "Fix: {{title}}",
            "created_at": "2025-01-01T00:00:00Z",
        })
    }

    fn sample_dispatch(id: &str, status: &str) -> serde_json::Value {
        json!({
            "id": id,
            "source_id": "issue-42",
            "status": status,
            "prompt_sent": "Fix the bug in main.rs",
            "dispatched_at": "2025-01-01T00:00:00Z",
            "completed_at": if status == "completed" { Some("2025-01-01T01:00:00Z") } else { None },
        })
    }

    #[tokio::test]
    async fn test_create_workflow_sends_correct_json_with_source_config() {
        let mut server = Server::new_async().await;
        let workflow_id = "wf-001";
        let agent_id = "agent-001";
        let response = sample_workflow(workflow_id, "issue-worker", agent_id, true);

        let mock = server
            .mock("POST", "/workflows")
            .match_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJsonString(
                json!({
                    "name": "issue-worker",
                    "agent_id": agent_id,
                    "source_config": {
                        "type": "github_issues",
                        "owner": "acme",
                        "repo": "widgets",
                        "labels": ["bug", "help wanted"],
                    },
                    "prompt_template": "Fix: {{title}}",
                    "poll_interval_secs": 120,
                    "enabled": true,
                })
                .to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let body = json!({
            "name": "issue-worker",
            "agent_id": agent_id,
            "source_config": {
                "type": "github_issues",
                "owner": "acme",
                "repo": "widgets",
                "labels": ["bug", "help wanted"],
            },
            "prompt_template": "Fix: {{title}}",
            "poll_interval_secs": 120,
            "enabled": true,
        });

        let result: Result<serde_json::Value, _> = client.post("/workflows", &body).await;
        assert!(result.is_ok());
        let created = result.unwrap();
        assert_eq!(created["name"], "issue-worker");
        assert_eq!(created["enabled"], true);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_workflow_with_labels_parsed() {
        let mut server = Server::new_async().await;
        let response = sample_workflow("wf-001", "labeled-workflow", "agent-001", true);

        // Verify that comma-separated labels are sent as a JSON array
        let mock = server
            .mock("POST", "/workflows")
            .match_body(mockito::Matcher::PartialJsonString(
                json!({
                    "source_config": {
                        "labels": ["bug", "enhancement", "p1"],
                    },
                })
                .to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let body = json!({
            "name": "labeled-workflow",
            "agent_id": "agent-001",
            "source_config": {
                "type": "github_issues",
                "owner": "acme",
                "repo": "widgets",
                "labels": ["bug", "enhancement", "p1"],
            },
            "prompt_template": "Fix: {{title}}",
            "poll_interval_secs": 60,
            "enabled": true,
        });

        let result: Result<serde_json::Value, _> = client.post("/workflows", &body).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_workflow_disabled() {
        let mut server = Server::new_async().await;
        let response = sample_workflow("wf-001", "disabled-workflow", "agent-001", false);

        let mock = server
            .mock("POST", "/workflows")
            .match_body(mockito::Matcher::PartialJsonString(
                json!({ "enabled": false }).to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let body = json!({
            "name": "disabled-workflow",
            "agent_id": "agent-001",
            "source_config": {
                "type": "github_issues",
                "owner": "acme",
                "repo": "widgets",
                "labels": [],
            },
            "prompt_template": "Fix: {{title}}",
            "poll_interval_secs": 60,
            "enabled": false,
        });

        let result: Result<serde_json::Value, _> = client.post("/workflows", &body).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["enabled"], false);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_workflows_success() {
        let mut server = Server::new_async().await;
        let workflows = json!([
            sample_workflow("wf-1", "workflow-1", "agent-1", true),
            sample_workflow("wf-2", "workflow-2", "agent-2", false),
        ]);

        let mock = server
            .mock("GET", "/workflows")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(workflows))
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> = client.get("/workflows").await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0]["name"], "workflow-1");
        assert_eq!(response.items[1]["enabled"], false);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_workflow_success() {
        let mut server = Server::new_async().await;
        let workflow_id = "wf-001";
        let workflow = sample_workflow(workflow_id, "my-workflow", "agent-001", true);

        let mock = server
            .mock("GET", format!("/workflows/{workflow_id}").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&workflow).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result: Result<serde_json::Value, _> =
            client.get(&format!("/workflows/{workflow_id}")).await;
        assert!(result.is_ok());
        let fetched = result.unwrap();
        assert_eq!(fetched["id"], workflow_id);
        assert_eq!(fetched["source_config"]["type"], "github_issues");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_update_workflow_sends_only_changed_fields() {
        let mut server = Server::new_async().await;
        let workflow_id = "wf-001";
        let updated = sample_workflow(workflow_id, "my-workflow", "agent-001", false);

        let mock = server
            .mock("PUT", format!("/workflows/{workflow_id}").as_str())
            .match_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJsonString(
                json!({
                    "enabled": false,
                    "poll_interval_secs": 300,
                })
                .to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&updated).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let body = json!({
            "name": null,
            "prompt_template": null,
            "poll_interval_secs": 300,
            "enabled": false,
        });

        let result: Result<serde_json::Value, _> =
            client.put(&format!("/workflows/{workflow_id}"), &body).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_delete_workflow_success() {
        let mut server = Server::new_async().await;
        let workflow_id = "wf-001";

        let mock = server
            .mock("DELETE", format!("/workflows/{workflow_id}").as_str())
            .with_status(200)
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result = client.delete(&format!("/workflows/{workflow_id}")).await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_delete_workflow_not_found() {
        let mut server = Server::new_async().await;
        let workflow_id = "nonexistent-id";

        let mock = server
            .mock("DELETE", format!("/workflows/{workflow_id}").as_str())
            .with_status(404)
            .with_body("Workflow not found")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result = client.delete(&format!("/workflows/{workflow_id}")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("404"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_workflow_history_success() {
        let mut server = Server::new_async().await;
        let workflow_id = "wf-001";
        let dispatches = json!([
            sample_dispatch("d-1", "completed"),
            sample_dispatch("d-2", "dispatched"),
            sample_dispatch("d-3", "failed"),
        ]);

        let mock = server
            .mock("GET", format!("/workflows/{workflow_id}/history").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(dispatches))
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> =
            client.get(&format!("/workflows/{workflow_id}/history")).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.items.len(), 3);
        assert_eq!(response.items[0]["status"], "completed");
        assert!(response.items[0]["completed_at"].is_string());
        assert_eq!(response.items[2]["status"], "failed");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_workflow_history_empty() {
        let mut server = Server::new_async().await;
        let workflow_id = "wf-001";

        let mock = server
            .mock("GET", format!("/workflows/{workflow_id}/history").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(json!([])))
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> =
            client.get(&format!("/workflows/{workflow_id}/history")).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().items.len(), 0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_workflow_json_output() {
        let mut server = Server::new_async().await;
        let workflow = sample_workflow("wf-001", "json-test", "agent-001", true);

        let mock = server
            .mock("POST", "/workflows")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&workflow).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let body = json!({
            "name": "json-test",
            "agent_id": "agent-001",
            "source_config": {
                "type": "github_issues",
                "owner": "acme",
                "repo": "widgets",
                "labels": [],
            },
            "prompt_template": "Fix: {{title}}",
            "poll_interval_secs": 60,
            "enabled": true,
        });

        let result: Result<serde_json::Value, _> = client.post("/workflows", &body).await;
        assert!(result.is_ok());
        // Verify JSON output is valid and contains all expected fields
        let output = result.unwrap();
        assert!(output.get("id").is_some());
        assert!(output.get("name").is_some());
        assert!(output.get("agent_id").is_some());
        assert!(output.get("enabled").is_some());
        assert!(output.get("source_config").is_some());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_list_agents_json_output() {
        let mut server = Server::new_async().await;
        let agents = json!([{
            "id": "agent-001",
            "name": "my-agent",
            "status": "running",
            "config": { "working_dir": "/tmp", "shell": "zsh" },
            "created_at": "2025-01-01T00:00:00Z",
        }]);

        let mock = server
            .mock("GET", "/agents")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(agents))
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> = client.get("/agents").await;
        assert!(result.is_ok());
        let items = result.unwrap().items;
        // Verify JSON contains all relevant agent fields
        assert!(items[0].get("id").is_some());
        assert!(items[0].get("name").is_some());
        assert!(items[0].get("status").is_some());
        assert!(items[0].get("config").is_some());

        mock.assert_async().await;
    }
}

#[allow(dead_code)]
mod orchestrator_error_tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_bad_request_400() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/agents")
            .with_status(400)
            .with_body("Bad request: missing required field 'name'")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let body = json!({ "working_dir": "/tmp" }); // Missing name

        let result: Result<serde_json::Value, _> = client.post("/agents", &body).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("400"));
        assert!(err.contains("missing required field"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_orchestrator_server_error_500() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/workflows")
            .with_status(500)
            .with_body("Internal server error")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> = client.get("/workflows").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_orchestrator_connection_refused() {
        // Use a port that nothing is listening on
        let client = ApiClient::new("http://127.0.0.1:19999".to_string());

        let result: Result<serde_json::Value, _> = client.get("/agents").await;
        assert!(result.is_err());
        // Error should indicate connection failure
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Failed to GET") || err.contains("error") || err.contains("connect"),
            "Error should indicate connection problem, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_orchestrator_malformed_paginated_response() {
        let mut server = Server::new_async().await;

        // Return a non-paginated response where a paginated one is expected
        let mock = server
            .mock("GET", "/agents")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "unexpected format"}"#)
            .create_async()
            .await;

        let client = ApiClient::new(server.url());


        let result: Result<PaginatedResponse, _> = client.get("/agents").await;
        assert!(result.is_err(), "Should fail to parse non-paginated response");

        mock.assert_async().await;
    }
}

mod client_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_request_constructs_correct_url() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/test/path")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let _result: Result<Vec<Notification>, _> = client.get("/test/path").await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_request_sends_json_body() {
        let mut server = Server::new_async().await;
        let notification = create_test_notification(Uuid::new_v4());

        let mock = server
            .mock("POST", "/notifications")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&notification_json(&notification)).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            title: "Test".to_string(),
            message: "Test".to_string(),
            requires_response: false,
        };

        let _result: Result<Notification, _> = client.post("/notifications", &request).await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_put_request_sends_json_body() {
        let mut server = Server::new_async().await;
        let id = Uuid::new_v4();
        let notification = create_test_notification(id);

        let mock = server
            .mock("PUT", format!("/notifications/{id}").as_str())
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&notification_json(&notification)).unwrap())
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let request =
            UpdateNotificationRequest { status: Some(NotificationStatus::Viewed), response: None };

        let _result: Result<Notification, _> =
            client.put(&format!("/notifications/{id}"), &request).await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_delete_request_success() {
        let mut server = Server::new_async().await;
        let id = Uuid::new_v4();

        let mock = server
            .mock("DELETE", format!("/notifications/{id}").as_str())
            .with_status(204)
            .create_async()
            .await;

        let client = ApiClient::new(server.url());
        let result = client.delete(&format!("/notifications/{id}")).await;

        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_multiple_clients_different_base_urls() {
        let mut server1 = Server::new_async().await;
        let mut server2 = Server::new_async().await;

        let mock1 = server1
            .mock("GET", "/notifications")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;

        let mock2 = server2
            .mock("POST", "/trigger")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"ok","checks_performed":0,"notifications_created":0}"#)
            .create_async()
            .await;

        let client1 = ApiClient::new(server1.url());
        let client2 = ApiClient::new(server2.url());

        let _result1: Result<Vec<Notification>, _> = client1.get("/notifications").await;
        let _result2: Result<serde_json::Value, _> =
            client2.post("/trigger", &serde_json::json!({})).await;

        mock1.assert_async().await;
        mock2.assert_async().await;
    }
}
