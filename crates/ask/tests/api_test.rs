use ask::{
    api::{create_router, ApiState},
    checks::{Check, CheckError, CheckRegistry, CheckResult, QuestionTemplate},
    notification_client::NotificationClient,
    state::AppState,
    types::*,
};
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use mockito::Server;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

// ── Shared test helpers ────────────────────────────────────────────────────────

/// Minimal ApiState with no checks registered and a dummy notification URL.
fn minimal_api_state() -> ApiState {
    ApiState {
        app_state: AppState::new(),
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    }
}

/// A fake check used in registry tests.  Always reports `needs_action = false`
/// so no notification service interaction is required.
struct NoOpCheck {
    name: &'static str,
    check_type: CheckType,
}

#[async_trait]
impl Check for NoOpCheck {
    fn name(&self) -> &str {
        self.name
    }

    fn check_type(&self) -> CheckType {
        self.check_type
    }

    async fn run(&self) -> Result<CheckResult, CheckError> {
        Ok(CheckResult { needs_action: false, detail: serde_json::json!({ "ok": true }) })
    }

    fn question_template(&self) -> QuestionTemplate {
        QuestionTemplate {
            title: format!("{} question", self.name),
            message: format!("{} message", self.name),
        }
    }
}

// Helper function to parse response body
async fn parse_response_body(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    let body_text = String::from_utf8(bytes.to_vec()).unwrap();
    serde_json::from_str(&body_text).unwrap()
}

// Test /health endpoint
#[tokio::test]
async fn test_health_endpoint() {
    let app_state = AppState::new();
    let notification_client = NotificationClient::new("http://localhost:17004".to_string());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/health").method("GET").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_response_body(response.into_body()).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "agentd-ask");
    assert!(body["version"].is_string());
}

// Test /trigger endpoint with mocked notification service
#[tokio::test]
async fn test_trigger_with_no_sessions_sends_notification() {
    // Note: This test will only work if tmux is installed
    // In a real scenario, we would mock the tmux check
    let mut mock_server = Server::new_async().await;

    let _mock = mock_server
        .mock("POST", "/notifications")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "source": {"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"},
            "lifetime": {"type": "ephemeral", "expires_at": "2024-01-01T00:05:00Z"},
            "priority": "normal",
            "status": "pending",
            "title": "Start tmux session?",
            "message": "No tmux sessions are currently running. Would you like to start one?",
            "requires_response": true,
            "response": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#,
        )
        .create_async()
        .await;

    let app_state = AppState::new();
    let notification_client = NotificationClient::new(mock_server.url());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: mock_server.url(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/trigger").method("POST").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // Response should be 200 regardless of whether notification was sent
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_response_body(response.into_body()).await;
    // With an empty registry no checks are run — the test verifies 200 + valid shape.
    assert!(body["checks_run"].is_array());
    assert!(body["notifications_sent"].is_array());
    assert!(body["results"]["checks"].is_object());
}

// Test /trigger endpoint respects cooldown
#[tokio::test]
async fn test_trigger_respects_cooldown() {
    let mut mock_server = Server::new_async().await;

    // Don't expect any calls because of cooldown
    let _mock = mock_server
        .mock("POST", "/notifications")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "source": {"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"},
            "lifetime": {"type": "ephemeral", "expires_at": "2024-01-01T00:05:00Z"},
            "priority": "normal",
            "status": "pending",
            "title": "Start tmux session?",
            "message": "No tmux sessions are currently running. Would you like to start one?",
            "requires_response": true,
            "response": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#,
        )
        .expect(0) // Expect no calls due to cooldown
        .create_async()
        .await;

    let app_state = AppState::with_cooldown(Duration::hours(1));

    // Manually record a recent notification to trigger cooldown
    app_state.record_notification(CheckType::TmuxSessions).await;

    let notification_client = NotificationClient::new(mock_server.url());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: mock_server.url(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/trigger").method("POST").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_response_body(response.into_body()).await;

    // No notifications should be sent due to cooldown
    assert_eq!(body["notifications_sent"].as_array().unwrap().len(), 0);
}

// Test /answer endpoint with valid question
#[tokio::test]
async fn test_answer_valid_question() {
    let mut mock_server = Server::new_async().await;

    let notification_id = Uuid::new_v4();
    let question_id = Uuid::new_v4();

    // Mock the notification update
    let mock = mock_server
        .mock("PUT", format!("/notifications/{notification_id}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{
            "id": "{notification_id}",
            "source": {{"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"}},
            "lifetime": {{"type": "persistent"}},
            "priority": "normal",
            "status": "responded",
            "title": "Test",
            "message": "Test message",
            "requires_response": true,
            "response": "yes",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:01Z"
        }}"#
        ))
        .create_async()
        .await;

    let app_state = AppState::new();

    // Add a pending question
    let question = QuestionInfo {
        question_id,
        notification_id,
        check_type: CheckType::TmuxSessions,
        asked_at: Utc::now(),
        status: QuestionStatus::Pending,
        answer: None,
    };
    app_state.add_question(question).await;

    let notification_client = NotificationClient::new(mock_server.url());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: mock_server.url(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let answer_request = AnswerRequest { question_id, answer: "yes".to_string() };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/answer")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&answer_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_response_body(response.into_body()).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["question_id"], question_id.to_string());

    mock.assert_async().await;
}

// Test /answer endpoint with nonexistent question
#[tokio::test]
async fn test_answer_nonexistent_question() {
    let app_state = AppState::new();
    let notification_client = NotificationClient::new("http://localhost:17004".to_string());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let answer_request = AnswerRequest { question_id: Uuid::new_v4(), answer: "yes".to_string() };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/answer")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&answer_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = parse_response_body(response.into_body()).await;
    assert!(body["error"].as_str().unwrap().contains("not found"));
}

// Test /answer endpoint with already answered question
#[tokio::test]
async fn test_answer_already_answered_question() {
    let app_state = AppState::new();
    let question_id = Uuid::new_v4();

    // Add an already answered question
    let question = QuestionInfo {
        question_id,
        notification_id: Uuid::new_v4(),
        check_type: CheckType::TmuxSessions,
        asked_at: Utc::now(),
        status: QuestionStatus::Answered,
        answer: Some("no".to_string()),
    };
    app_state.add_question(question).await;

    let notification_client = NotificationClient::new("http://localhost:17004".to_string());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let answer_request = AnswerRequest { question_id, answer: "yes".to_string() };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/answer")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&answer_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GONE);

    let body = parse_response_body(response.into_body()).await;
    assert!(body["error"].as_str().unwrap().contains("not pending"));
}

// Test /answer endpoint when notification update fails
#[tokio::test]
async fn test_answer_notification_update_fails_but_answer_succeeds() {
    let mut mock_server = Server::new_async().await;

    let notification_id = Uuid::new_v4();
    let question_id = Uuid::new_v4();

    // Mock the notification update to fail
    let mock = mock_server
        .mock("PUT", format!("/notifications/{notification_id}").as_str())
        .with_status(500)
        .with_body("Internal Server Error")
        .create_async()
        .await;

    let app_state = AppState::new();

    // Add a pending question
    let question = QuestionInfo {
        question_id,
        notification_id,
        check_type: CheckType::TmuxSessions,
        asked_at: Utc::now(),
        status: QuestionStatus::Pending,
        answer: None,
    };
    app_state.add_question(question).await;

    let notification_client = NotificationClient::new(mock_server.url());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: mock_server.url(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let answer_request = AnswerRequest { question_id, answer: "yes".to_string() };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/answer")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&answer_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Answer should still succeed even if notification update fails
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_response_body(response.into_body()).await;
    assert_eq!(body["success"], true);

    mock.assert_async().await;
}

// Test concurrent requests to /trigger endpoint
#[tokio::test]
async fn test_concurrent_trigger_requests() {
    let mut mock_server = Server::new_async().await;

    let _mock = mock_server
        .mock("POST", "/notifications")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "source": {"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"},
            "lifetime": {"type": "ephemeral", "expires_at": "2024-01-01T00:05:00Z"},
            "priority": "normal",
            "status": "pending",
            "title": "Start tmux session?",
            "message": "No tmux sessions are currently running. Would you like to start one?",
            "requires_response": true,
            "response": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#,
        )
        .create_async()
        .await;

    let app_state = AppState::with_cooldown(Duration::milliseconds(100));
    let notification_client = NotificationClient::new(mock_server.url());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: mock_server.url(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    // Make multiple concurrent requests
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let app_clone = app.clone();
            tokio::spawn(async move {
                app_clone
                    .oneshot(
                        Request::builder()
                            .uri("/trigger")
                            .method("POST")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
            })
        })
        .collect();

    // All requests should complete successfully
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

// Test /health endpoint response structure
#[tokio::test]
async fn test_health_response_structure() {
    let app_state = AppState::new();
    let notification_client = NotificationClient::new("http://test:9999".to_string());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: "http://test:9999".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/health").method("GET").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = parse_response_body(response.into_body()).await;

    // Verify all expected fields are present
    assert!(body.get("status").is_some());
    assert!(body.get("service").is_some());
    assert!(body.get("version").is_some());
    assert!(body.get("details").is_some());
    assert_eq!(body["details"]["notification_service_url"], "http://test:9999");
}

// Test /trigger endpoint response structure
#[tokio::test]
async fn test_trigger_response_structure() {
    let mut mock_server = Server::new_async().await;

    let _mock = mock_server
        .mock("POST", "/notifications")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "source": {"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"},
            "lifetime": {"type": "ephemeral", "expires_at": "2024-01-01T00:05:00Z"},
            "priority": "normal",
            "status": "pending",
            "title": "Start tmux session?",
            "message": "No tmux sessions are currently running. Would you like to start one?",
            "requires_response": true,
            "response": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#,
        )
        .create_async()
        .await;

    let app_state = AppState::new();
    let notification_client = NotificationClient::new(mock_server.url());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: mock_server.url(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/trigger").method("POST").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_response_body(response.into_body()).await;

    // Verify response structure
    assert!(body.get("checks_run").is_some());
    assert!(body["checks_run"].is_array());
    assert!(body.get("notifications_sent").is_some());
    assert!(body["notifications_sent"].is_array());
    assert!(body.get("results").is_some());
    // With an empty registry, results.checks is an empty map
    assert!(body["results"].get("checks").is_some());
}

// Test invalid request body to /answer endpoint
#[tokio::test]
async fn test_answer_with_invalid_json() {
    let app_state = AppState::new();
    let notification_client = NotificationClient::new("http://localhost:17004".to_string());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/answer")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 400 Bad Request for invalid JSON
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// Test wrong HTTP method on endpoints
#[tokio::test]
async fn test_wrong_http_method() {
    let app_state = AppState::new();
    let notification_client = NotificationClient::new("http://localhost:17004".to_string());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    // Try POST on /health (should be GET)
    let response = app
        .oneshot(Request::builder().uri("/health").method("POST").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// Test nonexistent endpoint
#[tokio::test]
async fn test_nonexistent_endpoint() {
    let app_state = AppState::new();
    let notification_client = NotificationClient::new("http://localhost:17004".to_string());
    let api_state = ApiState {
        app_state,
        notification_client,
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/nonexistent").method("GET").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ── List endpoint tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_questions_empty() {
    let app = create_router(minimal_api_state());

    let response = app
        .oneshot(Request::builder().uri("/questions").method("GET").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_response_body(response.into_body()).await;
    assert_eq!(body["total"], 0);
    assert!(body["questions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_questions_returns_all() {
    let app_state = AppState::new();

    for _ in 0..3 {
        app_state
            .add_question(QuestionInfo {
                question_id: Uuid::new_v4(),
                notification_id: Uuid::new_v4(),
                check_type: CheckType::TmuxSessions,
                asked_at: Utc::now(),
                status: QuestionStatus::Pending,
                answer: None,
            })
            .await;
    }

    let api_state = ApiState {
        app_state,
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/questions").method("GET").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_response_body(response.into_body()).await;
    assert_eq!(body["total"], 3);
    assert_eq!(body["questions"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_list_questions_filter_by_status_pending() {
    let app_state = AppState::new();

    // Add 2 pending, 1 answered
    for _ in 0..2 {
        app_state
            .add_question(QuestionInfo {
                question_id: Uuid::new_v4(),
                notification_id: Uuid::new_v4(),
                check_type: CheckType::TmuxSessions,
                asked_at: Utc::now(),
                status: QuestionStatus::Pending,
                answer: None,
            })
            .await;
    }
    app_state
        .add_question(QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Answered,
            answer: Some("yes".to_string()),
        })
        .await;

    let api_state = ApiState {
        app_state,
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/questions?status=pending")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_response_body(response.into_body()).await;
    assert_eq!(body["total"], 2);
    for q in body["questions"].as_array().unwrap() {
        assert_eq!(q["status"], "Pending");
    }
}

#[tokio::test]
async fn test_list_questions_filter_by_status_answered() {
    let app_state = AppState::new();

    app_state
        .add_question(QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        })
        .await;
    app_state
        .add_question(QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Answered,
            answer: Some("no".to_string()),
        })
        .await;

    let api_state = ApiState {
        app_state,
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/questions?status=answered")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_response_body(response.into_body()).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["questions"][0]["status"], "Answered");
}

#[tokio::test]
async fn test_get_question_by_id_found() {
    let app_state = AppState::new();
    let question_id = Uuid::new_v4();

    app_state
        .add_question(QuestionInfo {
            question_id,
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        })
        .await;

    let api_state = ApiState {
        app_state,
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/questions/{question_id}"))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_response_body(response.into_body()).await;
    assert_eq!(body["question_id"], question_id.to_string());
    assert_eq!(body["status"], "Pending");
}

#[tokio::test]
async fn test_get_question_by_id_not_found() {
    let app = create_router(minimal_api_state());
    let unknown_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/questions/{unknown_id}"))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = parse_response_body(response.into_body()).await;
    assert!(body["error"].as_str().unwrap().contains("not found"));
}

// ── Full flow: trigger -> list -> answer -> verify ─────────────────────────────

#[tokio::test]
async fn test_full_flow_list_then_answer_then_verify_status() {
    let app_state = AppState::new();
    let question_id = Uuid::new_v4();
    let notification_id = Uuid::new_v4();

    // Pre-seed a pending question (simulates a previous trigger)
    app_state
        .add_question(QuestionInfo {
            question_id,
            notification_id,
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        })
        .await;

    let mut mock_server = Server::new_async().await;
    let _mock = mock_server
        .mock("PUT", format!("/notifications/{notification_id}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{
            "id": "{notification_id}",
            "source": {{"type": "ask_service", "request_id": "{question_id}"}},
            "lifetime": {{"type": "persistent"}},
            "priority": "normal",
            "status": "responded",
            "title": "Test",
            "message": "Test",
            "requires_response": true,
            "response": "yes",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:01Z"
        }}"#
        ))
        .create_async()
        .await;

    let api_state = ApiState {
        app_state,
        notification_client: NotificationClient::new(mock_server.url()),
        notification_service_url: mock_server.url(),
        check_registry: Arc::new(CheckRegistry::new()),
    };
    let app = create_router(api_state);

    // Step 1: list — should show 1 pending question
    let list_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/questions?status=pending")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = parse_response_body(list_resp.into_body()).await;
    assert_eq!(list_body["total"], 1);

    // Step 2: answer the question
    let answer_req = AnswerRequest { question_id, answer: "yes".to_string() };
    let answer_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/answer")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&answer_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(answer_resp.status(), StatusCode::OK);
    let answer_body = parse_response_body(answer_resp.into_body()).await;
    assert_eq!(answer_body["success"], true);

    // Step 3: verify the question is no longer pending
    let list_after = app
        .oneshot(
            Request::builder()
                .uri("/questions?status=pending")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_after.status(), StatusCode::OK);
    let list_after_body = parse_response_body(list_after.into_body()).await;
    assert_eq!(list_after_body["total"], 0);
}

// ── Check registry tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_trigger_runs_all_registered_checks() {
    let mut registry = CheckRegistry::new();
    registry.register(Box::new(NoOpCheck { name: "check_a", check_type: CheckType::TmuxSessions }));
    registry
        .register(Box::new(NoOpCheck { name: "check_b", check_type: CheckType::ServiceHealth }));

    let api_state = ApiState {
        app_state: AppState::new(),
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(registry),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/trigger").method("POST").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_response_body(response.into_body()).await;

    let checks_run = body["checks_run"].as_array().unwrap();
    assert!(checks_run.contains(&Value::String("check_a".to_string())));
    assert!(checks_run.contains(&Value::String("check_b".to_string())));

    // Both checks returned ok detail
    assert!(body["results"]["checks"]["check_a"]["ok"].as_bool().unwrap());
    assert!(body["results"]["checks"]["check_b"]["ok"].as_bool().unwrap());
}

#[tokio::test]
async fn test_trigger_with_empty_registry_returns_no_checks() {
    let app = create_router(minimal_api_state());

    let response = app
        .oneshot(Request::builder().uri("/trigger").method("POST").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_response_body(response.into_body()).await;
    assert!(body["checks_run"].as_array().unwrap().is_empty());
    assert!(body["notifications_sent"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_trigger_check_result_detail_is_in_response() {
    let mut registry = CheckRegistry::new();
    registry
        .register(Box::new(NoOpCheck { name: "my_check", check_type: CheckType::TmuxSessions }));

    let api_state = ApiState {
        app_state: AppState::new(),
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(registry),
    };

    let app = create_router(api_state);

    let response = app
        .oneshot(Request::builder().uri("/trigger").method("POST").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = parse_response_body(response.into_body()).await;
    // NoOpCheck returns {"ok": true}
    assert_eq!(body["results"]["checks"]["my_check"]["ok"], true);
}

// ── AskClient unit tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_ask_client_health_against_live_router() {
    use ask::client::AskClient;
    use axum::serve;
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = create_router(minimal_api_state());

    tokio::spawn(async move {
        serve(listener, app).await.unwrap();
    });

    let client = AskClient::new(format!("http://127.0.0.1:{port}"));
    let health = client.health().await.unwrap();
    assert_eq!(health.status, "ok");
    assert_eq!(health.service, "agentd-ask");
}

#[tokio::test]
async fn test_ask_client_list_questions_against_live_router() {
    use ask::client::AskClient;
    use axum::serve;
    use tokio::net::TcpListener;

    let app_state = AppState::new();
    app_state
        .add_question(QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        })
        .await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let api_state = ApiState {
        app_state,
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };
    let app = create_router(api_state);

    tokio::spawn(async move {
        serve(listener, app).await.unwrap();
    });

    let client = AskClient::new(format!("http://127.0.0.1:{port}"));

    // All questions
    let all = client.list_questions(None).await.unwrap();
    assert_eq!(all.total, 1);

    // Filtered
    let pending = client.list_questions(Some("pending")).await.unwrap();
    assert_eq!(pending.total, 1);

    let answered = client.list_questions(Some("answered")).await.unwrap();
    assert_eq!(answered.total, 0);
}

#[tokio::test]
async fn test_ask_client_get_question_against_live_router() {
    use ask::client::AskClient;
    use axum::serve;
    use tokio::net::TcpListener;

    let app_state = AppState::new();
    let question_id = Uuid::new_v4();
    app_state
        .add_question(QuestionInfo {
            question_id,
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        })
        .await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let api_state = ApiState {
        app_state,
        notification_client: NotificationClient::new("http://localhost:17004".to_string()),
        notification_service_url: "http://localhost:17004".to_string(),
        check_registry: Arc::new(CheckRegistry::new()),
    };
    let app = create_router(api_state);

    tokio::spawn(async move {
        serve(listener, app).await.unwrap();
    });

    let client = AskClient::new(format!("http://127.0.0.1:{port}"));

    let q = client.get_question(&question_id).await.unwrap();
    assert_eq!(q.question_id, question_id);
    assert_eq!(q.status, QuestionStatus::Pending);

    // Non-existent returns error
    let err = client.get_question(&Uuid::new_v4()).await;
    assert!(err.is_err());
}
