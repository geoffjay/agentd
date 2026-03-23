//! Integration tests for the webhook HTTP endpoint.
//!
//! Tests the `POST /webhooks/{workflow_id}` handler end-to-end using an
//! in-process Axum router. No real TCP connections are made.
//!
//! # Design
//!
//! HTTP tests use `tower::ServiceExt::oneshot` directly on a cloned Router.
//! This avoids real TCP and is faster than spawning a server.
//!
//! A `NullBackend` implements `ExecutionBackend` with no-op responses so that
//! `AgentManager` can be constructed without a real tmux/Docker environment.
//! The webhook handler only accesses `state.scheduler`; the manager is never
//! invoked during these tests.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use orchestrator::{
    manager::AgentManager,
    scheduler::{
        api::{webhook_routes, WorkflowState},
        storage::SchedulerStorage,
        types::{TriggerConfig, WorkflowConfig},
        Scheduler,
    },
    storage::AgentStorage,
    websocket::ConnectionRegistry,
};
use sha2::Sha256;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;
use wrap::{
    backend::{SessionConfig, SessionExitInfo, SessionHealth},
    ExecutionBackend,
};

struct NullBackend;

#[async_trait]
impl ExecutionBackend for NullBackend {
    async fn create_session(&self, _config: &SessionConfig) -> anyhow::Result<()> {
        Ok(())
    }
    async fn launch_agent(&self, _config: &SessionConfig) -> anyhow::Result<()> {
        Ok(())
    }
    async fn session_exists(&self, _session_name: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
    async fn kill_session(&self, _session_name: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn send_command(&self, _session_name: &str, _command: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }
    fn prefix(&self) -> &str {
        "test"
    }
    async fn session_health(&self, _session_name: &str) -> anyhow::Result<SessionHealth> {
        Ok(SessionHealth::Unknown)
    }
    async fn session_exit_info(
        &self,
        _session_name: &str,
    ) -> anyhow::Result<Option<SessionExitInfo>> {
        Ok(None)
    }
}

async fn build_webhook_app() -> (axum::Router, Arc<Scheduler>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let agent_storage = AgentStorage::with_path(&db_path).await.unwrap();
    let scheduler_storage = SchedulerStorage::new(agent_storage.db().clone());
    let registry = ConnectionRegistry::new();

    let scheduler = Arc::new(Scheduler::new(scheduler_storage, registry.clone()));
    let manager = Arc::new(AgentManager::new(
        Arc::new(agent_storage),
        Arc::new(NullBackend),
        registry,
        "ws://localhost:8080".to_string(),
    ));

    let state = WorkflowState { scheduler: scheduler.clone(), manager };
    let router = webhook_routes(state);

    (router, scheduler, temp_dir)
}

fn make_webhook_workflow(agent_id: Uuid, secret: Option<String>) -> WorkflowConfig {
    let now = Utc::now();
    WorkflowConfig {
        id: Uuid::new_v4(),
        name: "test-webhook-workflow".to_string(),
        agent_id,
        trigger_config: TriggerConfig::Webhook { secret },
        prompt_template: "Review: {{title}}".to_string(),
        poll_interval_secs: 60,
        enabled: true,
        tool_policy: Default::default(),
        created_at: now,
        updated_at: now,
    }
}

fn sign_body(secret: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_post_webhook_no_secret_returns_202() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let workflow = make_webhook_workflow(Uuid::new_v4(), None);
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"action":"opened","title":"fix bug"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn test_post_webhook_empty_body_accepted_without_secret() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let workflow = make_webhook_workflow(Uuid::new_v4(), None);
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn test_post_webhook_valid_signature_returns_202() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let secret = "test-secret-abc123";
    let workflow = make_webhook_workflow(Uuid::new_v4(), Some(secret.to_string()));
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let body = r#"{"action":"opened","number":42}"#;
    let sig = sign_body(secret, body.as_bytes());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .header("Content-Type", "application/json")
                .header("x-hub-signature-256", sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn test_post_webhook_github_pr_event_with_signature_returns_202() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let secret = "pr-review-secret";
    let workflow = make_webhook_workflow(Uuid::new_v4(), Some(secret.to_string()));
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let body = serde_json::json!({
        "action": "labeled",
        "pull_request": {
            "number": 99,
            "title": "Add feature X",
            "body": "This PR adds feature X.",
            "html_url": "https://github.com/org/repo/pull/99",
            "labels": [{"name": "review-agent"}],
            "assignee": null
        }
    })
    .to_string();
    let sig = sign_body(secret, body.as_bytes());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .header("Content-Type", "application/json")
                .header("x-github-event", "pull_request")
                .header("x-github-delivery", "abc-123-delivery-id")
                .header("x-hub-signature-256", sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn test_post_webhook_missing_signature_returns_401() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let workflow = make_webhook_workflow(Uuid::new_v4(), Some("secret".to_string()));
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"action":"opened"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let json = body_json(response).await;
    assert!(json["error"].as_str().unwrap().contains("Missing"));
}

#[tokio::test]
async fn test_post_webhook_invalid_signature_returns_401() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let workflow = make_webhook_workflow(Uuid::new_v4(), Some("correct-secret".to_string()));
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let body = r#"{"action":"opened"}"#;
    let wrong_sig = sign_body("wrong-secret", body.as_bytes());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .header("Content-Type", "application/json")
                .header("x-hub-signature-256", wrong_sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let json = body_json(response).await;
    assert!(json["error"].as_str().unwrap().contains("Invalid"));
}

#[tokio::test]
async fn test_post_webhook_tampered_body_returns_401() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let secret = "tamper-test-secret";
    let workflow = make_webhook_workflow(Uuid::new_v4(), Some(secret.to_string()));
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let original_body = r#"{"action":"opened","number":1}"#;
    let sig = sign_body(secret, original_body.as_bytes());
    let tampered_body = r#"{"action":"opened","number":9999}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .header("Content-Type", "application/json")
                .header("x-hub-signature-256", sig)
                .body(Body::from(tampered_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_post_webhook_unknown_workflow_id_returns_404() {
    let (app, _scheduler, _tmp) = build_webhook_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", Uuid::new_v4()))
                .body(Body::from(r#"{"action":"ping"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_post_webhook_workflow_not_running_returns_404() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let workflow = make_webhook_workflow(Uuid::new_v4(), None);
    scheduler.storage().add_workflow(&workflow).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .body(Body::from(r#"{"action":"ping"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_post_webhook_manual_trigger_workflow_returns_400() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let now = Utc::now();
    let workflow = WorkflowConfig {
        id: Uuid::new_v4(),
        name: "manual-wf".to_string(),
        agent_id: Uuid::new_v4(),
        trigger_config: TriggerConfig::Manual {},
        prompt_template: "Do: {{title}}".to_string(),
        poll_interval_secs: 60,
        enabled: true,
        tool_policy: Default::default(),
        created_at: now,
        updated_at: now,
    };
    scheduler.storage().add_workflow(&workflow).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .body(Body::from(r#"{"action":"ping"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_json(response).await;
    assert!(json["error"].as_str().unwrap().contains("not a webhook trigger"));
}

#[tokio::test]
async fn test_post_webhook_202_response_has_empty_body() {
    let (app, scheduler, _tmp) = build_webhook_app().await;
    let workflow = make_webhook_workflow(Uuid::new_v4(), None);
    scheduler.start_workflow(workflow.clone()).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", workflow.id))
                .body(Body::from(r#"{"action":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(bytes.is_empty());
}

#[tokio::test]
async fn test_post_webhook_error_response_is_json_with_single_error_key() {
    let (app, _scheduler, _tmp) = build_webhook_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhooks/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json.is_object());
    assert!(json["error"].is_string());
    assert_eq!(json.as_object().unwrap().len(), 1);
}
