use agentd_cli::client::ApiClient;
use agentd_cli::types::*;
use chrono::{Duration, Utc};
use mockito::Server;
use serde_json::json;
use uuid::Uuid;

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
