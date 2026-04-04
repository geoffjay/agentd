//! Integration tests for the ask service REST API.
//!
//! Uses Tower's `oneshot` helper to drive the Axum router in-process,
//! without binding to a network port. A fresh in-memory SQLite database
//! is used for each test.

use ask::{
    api::{create_router_with_tracing, ApiState},
    state::AppState,
    storage::QuestionStorage,
    types::{
        AnswerQuestionRequest, CreateQuestionRequest, Question, QuestionPriority, QuestionStatus,
        QuestionsListResponse,
    },
};
use axum::body::Body;
use http_body_util::BodyExt;
use hyper::{Request, StatusCode};
use tower::ServiceExt;
use uuid::Uuid;

// ─── Helpers ────────────────────────────────────────────────────────────────

async fn make_app() -> axum::Router {
    let storage = QuestionStorage::in_memory().await.unwrap();
    let app_state = AppState::new_with_storage(storage);
    let api_state = ApiState { app_state, orchestrator_url: None };
    create_router_with_tracing(api_state)
}

/// Deserialize a response body into T.
async fn parse_body<T: serde::de::DeserializeOwned>(body: Body) -> T {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn create_question_request() -> CreateQuestionRequest {
    CreateQuestionRequest {
        agent_id: "dietician".to_string(),
        workflow_id: None,
        dispatch_id: None,
        category: Some("health".to_string()),
        question: "What did you eat yesterday?".to_string(),
        context: None,
        priority: Some(QuestionPriority::Normal),
        expires_in_seconds: None,
    }
}

/// POST /questions and return the created Question.
async fn create_question(app: axum::Router, req: &CreateQuestionRequest) -> Question {
    let body = serde_json::to_vec(req).unwrap();
    let request = Request::builder()
        .method("POST")
        .uri("/questions")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    parse_body(response.into_body()).await
}

// ─── Health ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let app = make_app().await;
    let request = Request::builder().method("GET").uri("/health").body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = parse_body(response.into_body()).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "agentd-ask");
}

// ─── POST /questions ─────────────────────────────────────────────────────────

#[tokio::test]
async fn create_question_returns_201() {
    let app = make_app().await;
    let req = create_question_request();
    let q = create_question(app, &req).await;

    assert_eq!(q.agent_id, "dietician");
    assert_eq!(q.question, "What did you eat yesterday?");
    assert_eq!(q.status, QuestionStatus::Pending);
    assert_eq!(q.priority, QuestionPriority::Normal);
    assert!(q.answer.is_none());
}

#[tokio::test]
async fn create_question_with_all_fields() {
    let app = make_app().await;
    let wf_id = Uuid::new_v4();
    let dp_id = Uuid::new_v4();
    let req = CreateQuestionRequest {
        agent_id: "test-agent".to_string(),
        workflow_id: Some(wf_id),
        dispatch_id: Some(dp_id),
        category: Some("deployment".to_string()),
        question: "Should I deploy?".to_string(),
        context: Some("Staging passed.".to_string()),
        priority: Some(QuestionPriority::High),
        expires_in_seconds: Some(3600),
    };
    let q = create_question(app, &req).await;

    assert_eq!(q.workflow_id, Some(wf_id));
    assert_eq!(q.dispatch_id, Some(dp_id));
    assert_eq!(q.category, Some("deployment".to_string()));
    assert_eq!(q.priority, QuestionPriority::High);
    assert!(q.expires_at.is_some());
}

#[tokio::test]
async fn create_question_empty_agent_id_returns_400() {
    let app = make_app().await;
    let mut req = create_question_request();
    req.agent_id = "".to_string();
    let body = serde_json::to_vec(&req).unwrap();

    let request = Request::builder()
        .method("POST")
        .uri("/questions")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_question_empty_question_text_returns_400() {
    let app = make_app().await;
    let mut req = create_question_request();
    req.question = "".to_string();
    let body = serde_json::to_vec(&req).unwrap();

    let request = Request::builder()
        .method("POST")
        .uri("/questions")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ─── POST /questions/{id}/answer ─────────────────────────────────────────────

#[tokio::test]
async fn answer_question_returns_200() {
    let app = make_app().await;
    let q = create_question(app.clone(), &create_question_request()).await;

    let answer_body =
        serde_json::to_vec(&AnswerQuestionRequest { answer: "I had oatmeal.".to_string() })
            .unwrap();
    let request = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", q.id))
        .header("content-type", "application/json")
        .body(Body::from(answer_body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let updated: Question = parse_body(response.into_body()).await;
    assert_eq!(updated.status, QuestionStatus::Answered);
    assert_eq!(updated.answer, Some("I had oatmeal.".to_string()));
    assert!(updated.answered_at.is_some());
}

#[tokio::test]
async fn answer_question_empty_answer_returns_400() {
    let app = make_app().await;
    let q = create_question(app.clone(), &create_question_request()).await;

    let answer_body =
        serde_json::to_vec(&AnswerQuestionRequest { answer: "".to_string() }).unwrap();
    let request = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", q.id))
        .header("content-type", "application/json")
        .body(Body::from(answer_body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn answer_question_nonexistent_returns_404() {
    let app = make_app().await;
    let answer_body =
        serde_json::to_vec(&AnswerQuestionRequest { answer: "yes".to_string() }).unwrap();
    let request = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", Uuid::new_v4()))
        .header("content-type", "application/json")
        .body(Body::from(answer_body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn answer_already_answered_question_returns_409() {
    let app = make_app().await;
    let q = create_question(app.clone(), &create_question_request()).await;

    // First answer — should succeed.
    let body =
        serde_json::to_vec(&AnswerQuestionRequest { answer: "first answer".to_string() }).unwrap();
    let r1 = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", q.id))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let resp1 = app.clone().oneshot(r1).await.unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);

    // Second answer — should fail with 409 Conflict.
    let body2 =
        serde_json::to_vec(&AnswerQuestionRequest { answer: "second answer".to_string() }).unwrap();
    let r2 = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", q.id))
        .header("content-type", "application/json")
        .body(Body::from(body2))
        .unwrap();
    let resp2 = app.oneshot(r2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::CONFLICT);
}

// ─── POST /questions/{id}/dismiss ────────────────────────────────────────────

#[tokio::test]
async fn dismiss_question_returns_200() {
    let app = make_app().await;
    let q = create_question(app.clone(), &create_question_request()).await;

    let request = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/dismiss", q.id))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let updated: Question = parse_body(response.into_body()).await;
    assert_eq!(updated.status, QuestionStatus::Dismissed);
    assert!(updated.answer.is_none());
}

#[tokio::test]
async fn dismiss_already_answered_question_returns_409() {
    let app = make_app().await;
    let q = create_question(app.clone(), &create_question_request()).await;

    // Answer it first.
    let body = serde_json::to_vec(&AnswerQuestionRequest { answer: "yes".to_string() }).unwrap();
    let r1 = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", q.id))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    app.clone().oneshot(r1).await.unwrap();

    // Now try to dismiss — should fail.
    let r2 = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/dismiss", q.id))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(r2).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// ─── GET /questions ───────────────────────────────────────────────────────────

#[tokio::test]
async fn list_questions_returns_all() {
    let app = make_app().await;

    let mut req1 = create_question_request();
    req1.agent_id = "agent-1".to_string();
    let mut req2 = create_question_request();
    req2.agent_id = "agent-2".to_string();
    create_question(app.clone(), &req1).await;
    create_question(app.clone(), &req2).await;

    let request = Request::builder().method("GET").uri("/questions").body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let result: QuestionsListResponse = parse_body(response.into_body()).await;
    assert_eq!(result.total, 2);
    assert_eq!(result.questions.len(), 2);
}

#[tokio::test]
async fn list_questions_filters_by_status() {
    let app = make_app().await;
    let q = create_question(app.clone(), &create_question_request()).await;

    // Answer the question.
    let body = serde_json::to_vec(&AnswerQuestionRequest { answer: "salad".to_string() }).unwrap();
    let r = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", q.id))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    app.clone().oneshot(r).await.unwrap();

    // List pending — should be empty.
    let r_pending = Request::builder()
        .method("GET")
        .uri("/questions?status=Pending")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(r_pending).await.unwrap();
    let pending: QuestionsListResponse = parse_body(resp.into_body()).await;
    assert_eq!(pending.total, 0);

    // List answered — should have 1.
    let r_answered = Request::builder()
        .method("GET")
        .uri("/questions?status=Answered")
        .body(Body::empty())
        .unwrap();
    let resp2 = app.oneshot(r_answered).await.unwrap();
    let answered: QuestionsListResponse = parse_body(resp2.into_body()).await;
    assert_eq!(answered.total, 1);
}

#[tokio::test]
async fn list_questions_filters_by_agent_id() {
    let app = make_app().await;

    let mut req1 = create_question_request();
    req1.agent_id = "dietician".to_string();
    let mut req2 = create_question_request();
    req2.agent_id = "assistant".to_string();
    create_question(app.clone(), &req1).await;
    create_question(app.clone(), &req2).await;

    let request = Request::builder()
        .method("GET")
        .uri("/questions?agent_id=dietician")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let result: QuestionsListResponse = parse_body(response.into_body()).await;

    assert_eq!(result.total, 1);
    assert_eq!(result.questions[0].agent_id, "dietician");
}

#[tokio::test]
async fn list_questions_filters_by_category() {
    let app = make_app().await;

    let mut req1 = create_question_request();
    req1.category = Some("health".to_string());
    let mut req2 = create_question_request();
    req2.category = Some("deployment".to_string());
    create_question(app.clone(), &req1).await;
    create_question(app.clone(), &req2).await;

    let request = Request::builder()
        .method("GET")
        .uri("/questions?category=health")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let result: QuestionsListResponse = parse_body(response.into_body()).await;

    assert_eq!(result.total, 1);
    assert_eq!(result.questions[0].category, Some("health".to_string()));
}

#[tokio::test]
async fn list_questions_invalid_status_returns_400() {
    let app = make_app().await;
    let request = Request::builder()
        .method("GET")
        .uri("/questions?status=NotAStatus")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ─── GET /questions/{id} ─────────────────────────────────────────────────────

#[tokio::test]
async fn get_question_returns_200() {
    let app = make_app().await;
    let q = create_question(app.clone(), &create_question_request()).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!("/questions/{}", q.id))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let fetched: Question = parse_body(response.into_body()).await;
    assert_eq!(fetched.id, q.id);
    assert_eq!(fetched.agent_id, "dietician");
}

#[tokio::test]
async fn get_question_nonexistent_returns_404() {
    let app = make_app().await;
    let request = Request::builder()
        .method("GET")
        .uri(format!("/questions/{}", Uuid::new_v4()))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ─── Orchestrator callback (fire-and-forget) ─────────────────────────────────

#[tokio::test]
async fn answer_fires_orchestrator_callback() {
    // Start a mock HTTP server to capture the callback.
    let mut server = mockito::Server::new_async().await;
    let mock =
        server.mock("POST", "/events/ask").with_status(200).with_body("{}").create_async().await;

    let storage = QuestionStorage::in_memory().await.unwrap();
    let app_state = AppState::new_with_storage(storage);
    let api_state = ApiState { app_state, orchestrator_url: Some(server.url()) };
    let app = create_router_with_tracing(api_state);

    let q = create_question(app.clone(), &create_question_request()).await;

    let body =
        serde_json::to_vec(&AnswerQuestionRequest { answer: "oatmeal".to_string() }).unwrap();
    let request = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", q.id))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Give the background task a moment to fire the callback.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    mock.assert_async().await;
}

#[tokio::test]
async fn dismiss_fires_orchestrator_callback() {
    let mut server = mockito::Server::new_async().await;
    let mock =
        server.mock("POST", "/events/ask").with_status(200).with_body("{}").create_async().await;

    let storage = QuestionStorage::in_memory().await.unwrap();
    let app_state = AppState::new_with_storage(storage);
    let api_state = ApiState { app_state, orchestrator_url: Some(server.url()) };
    let app = create_router_with_tracing(api_state);

    let q = create_question(app.clone(), &create_question_request()).await;

    let request = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/dismiss", q.id))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    mock.assert_async().await;
}

#[tokio::test]
async fn callback_failure_does_not_block_answer_response() {
    // Point at a URL that will refuse the connection.
    let storage = QuestionStorage::in_memory().await.unwrap();
    let app_state = AppState::new_with_storage(storage);
    let api_state =
        ApiState { app_state, orchestrator_url: Some("http://127.0.0.1:19999".to_string()) };
    let app = create_router_with_tracing(api_state);

    let q = create_question(app.clone(), &create_question_request()).await;

    // Answer — must still return 200 even though the callback will fail.
    let body = serde_json::to_vec(&AnswerQuestionRequest { answer: "pasta".to_string() }).unwrap();
    let request = Request::builder()
        .method("POST")
        .uri(format!("/questions/{}/answer", q.id))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
