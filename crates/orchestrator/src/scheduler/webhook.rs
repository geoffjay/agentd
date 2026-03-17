//! Webhook infrastructure for inbound HTTP webhook triggers.
//!
//! This module provides:
//! - [`WebhookRegistry`] — a shared registry mapping workflow IDs to channel senders
//! - HMAC-SHA256 signature verification (GitHub-compatible)
//! - Webhook payload parsing (GitHub events and generic JSON)

use crate::scheduler::types::Task;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Default bounded channel capacity for webhook task channels.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 64;

/// Entry in the webhook registry: a channel sender and the optional HMAC secret.
struct WebhookEntry {
    tx: mpsc::Sender<Task>,
    secret: Option<String>,
}

/// A shared registry that maps workflow IDs to their webhook channel senders.
///
/// When a webhook workflow starts, its `mpsc::Sender<Task>` and optional HMAC
/// secret are registered here. The HTTP webhook handler looks up the sender to
/// push incoming tasks to the corresponding `WebhookStrategy`.
#[derive(Default)]
pub struct WebhookRegistry {
    entries: RwLock<HashMap<Uuid, WebhookEntry>>,
}

impl WebhookRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self { entries: RwLock::new(HashMap::new()) }
    }

    /// Register a webhook sender for a workflow.
    pub async fn register(
        &self,
        workflow_id: Uuid,
        tx: mpsc::Sender<Task>,
        secret: Option<String>,
    ) {
        let mut entries = self.entries.write().await;
        entries.insert(workflow_id, WebhookEntry { tx, secret });
    }

    /// Unregister (and drop) the webhook sender for a workflow.
    ///
    /// Dropping the sender causes the `mpsc::Receiver` in `WebhookStrategy` to
    /// return `None`, naturally terminating the strategy.
    pub async fn unregister(&self, workflow_id: &Uuid) {
        let mut entries = self.entries.write().await;
        entries.remove(workflow_id);
    }

    /// Look up a workflow's sender and secret.
    ///
    /// Returns `None` if the workflow is not registered (not running or not a
    /// webhook trigger).
    pub async fn lookup(&self, workflow_id: &Uuid) -> Option<(mpsc::Sender<Task>, Option<String>)> {
        let entries = self.entries.read().await;
        entries.get(workflow_id).map(|e| (e.tx.clone(), e.secret.clone()))
    }

    /// Return the number of registered webhook workflows.
    #[cfg(test)]
    #[allow(clippy::len_without_is_empty)]
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }
}

// ---------------------------------------------------------------------------
// HMAC-SHA256 Signature Verification
// ---------------------------------------------------------------------------

type HmacSha256 = Hmac<Sha256>;

/// Verify a GitHub-style HMAC-SHA256 signature.
///
/// The `signature_header` is expected in the format `sha256=<hex>` (as sent by
/// GitHub in the `X-Hub-Signature-256` header). Raw hex without the `sha256=`
/// prefix is also accepted.
///
/// Uses the `hmac` crate's `verify_slice` which performs constant-time
/// comparison internally, preventing timing attacks.
pub fn verify_signature(secret: &str, body: &[u8], signature_header: &str) -> bool {
    // Strip the "sha256=" prefix if present.
    let hex_sig = signature_header.strip_prefix("sha256=").unwrap_or(signature_header);

    // Decode the hex signature.
    let expected_bytes = match hex::decode(hex_sig) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };

    // Compute HMAC-SHA256.
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => return false,
    };
    mac.update(body);

    // Constant-time comparison via `verify_slice`.
    mac.verify_slice(&expected_bytes).is_ok()
}

// ---------------------------------------------------------------------------
// Webhook Payload Parsing
// ---------------------------------------------------------------------------

/// Parse a webhook payload into a [`Task`].
///
/// If the `X-GitHub-Event` header is present, the payload is parsed as a GitHub
/// webhook event. Otherwise, the raw JSON body is used as the task body.
///
/// The `delivery_id` (from `X-GitHub-Delivery` or auto-generated) is used to
/// produce a unique `source_id` for dedup.
pub fn parse_webhook_payload(
    github_event: Option<&str>,
    delivery_id: Option<&str>,
    body: &[u8],
) -> Task {
    let timestamp = chrono::Utc::now().to_rfc3339();
    let delivery = delivery_id.map(|d| d.to_string()).unwrap_or_else(|| Uuid::new_v4().to_string());

    let mut metadata = HashMap::new();
    metadata.insert("delivery_id".to_string(), delivery.clone());
    metadata.insert("timestamp".to_string(), timestamp.clone());

    // Try to parse as JSON for structured field extraction.
    let body_str = String::from_utf8_lossy(body).to_string();
    let json_value: Option<serde_json::Value> = serde_json::from_slice(body).ok();

    if let Some(event_type) = github_event {
        metadata.insert("github_event".to_string(), event_type.to_string());
        parse_github_event(event_type, &json_value, body_str, delivery, timestamp, metadata)
    } else {
        parse_generic_webhook(json_value.as_ref(), body_str, delivery, timestamp, metadata)
    }
}

/// Parse a GitHub webhook event into a Task with structured fields.
fn parse_github_event(
    event_type: &str,
    json_value: &Option<serde_json::Value>,
    body_str: String,
    delivery: String,
    timestamp: String,
    mut metadata: HashMap<String, String>,
) -> Task {
    let (title, body, url, labels, assignee) = match (event_type, json_value) {
        ("issues", Some(val)) => {
            let issue = &val["issue"];
            let action = val["action"].as_str().unwrap_or("unknown");
            metadata.insert("action".to_string(), action.to_string());

            let title = issue["title"].as_str().unwrap_or("Untitled issue").to_string();
            let body = issue["body"].as_str().unwrap_or("").to_string();
            let url = issue["html_url"].as_str().unwrap_or("").to_string();
            let labels: Vec<String> = issue["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter().filter_map(|l| l["name"].as_str().map(|s| s.to_string())).collect()
                })
                .unwrap_or_default();
            let assignee = issue["assignee"]["login"].as_str().map(|s| s.to_string());

            if let Some(num) = issue["number"].as_u64() {
                metadata.insert("issue_number".to_string(), num.to_string());
            }

            (title, body, url, labels, assignee)
        }
        ("pull_request", Some(val)) => {
            let pr = &val["pull_request"];
            let action = val["action"].as_str().unwrap_or("unknown");
            metadata.insert("action".to_string(), action.to_string());

            let title = pr["title"].as_str().unwrap_or("Untitled PR").to_string();
            let body = pr["body"].as_str().unwrap_or("").to_string();
            let url = pr["html_url"].as_str().unwrap_or("").to_string();
            let labels: Vec<String> = pr["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter().filter_map(|l| l["name"].as_str().map(|s| s.to_string())).collect()
                })
                .unwrap_or_default();
            let assignee = pr["assignee"]["login"].as_str().map(|s| s.to_string());

            if let Some(num) = pr["number"].as_u64() {
                metadata.insert("pr_number".to_string(), num.to_string());
            }

            (title, body, url, labels, assignee)
        }
        _ => {
            // Other GitHub events — use raw body with event type context.
            let title = format!("GitHub event: {}", event_type);
            (title, body_str, String::new(), vec![], None)
        }
    };

    Task {
        source_id: format!("webhook:{}:{}", delivery, timestamp),
        title,
        body,
        url,
        labels,
        assignee,
        metadata,
    }
}

/// Parse a generic (non-GitHub) webhook payload into a Task.
fn parse_generic_webhook(
    json_value: Option<&serde_json::Value>,
    body_str: String,
    delivery: String,
    timestamp: String,
    metadata: HashMap<String, String>,
) -> Task {
    // Try to extract a title from common JSON fields.
    let title = json_value
        .and_then(|v| {
            v["title"]
                .as_str()
                .or_else(|| v["subject"].as_str())
                .or_else(|| v["name"].as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "Webhook payload".to_string());

    Task {
        source_id: format!("webhook:{}:{}", delivery, timestamp),
        title,
        body: body_str,
        url: String::new(),
        labels: vec![],
        assignee: None,
        metadata,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── HMAC Verification tests ───────────────────────────────────────

    #[test]
    fn verify_signature_valid_with_prefix() {
        let secret = "test-secret-key";
        let body = b"hello world";

        // Compute expected signature.
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let result = mac.finalize();
        let hex_sig = hex::encode(result.into_bytes());

        let header = format!("sha256={}", hex_sig);
        assert!(verify_signature(secret, body, &header));
    }

    #[test]
    fn verify_signature_valid_without_prefix() {
        let secret = "test-secret-key";
        let body = b"hello world";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let result = mac.finalize();
        let hex_sig = hex::encode(result.into_bytes());

        // Raw hex without "sha256=" prefix.
        assert!(verify_signature(secret, body, &hex_sig));
    }

    #[test]
    fn verify_signature_rejects_wrong_secret() {
        let body = b"hello world";

        let mut mac = HmacSha256::new_from_slice(b"correct-secret").unwrap();
        mac.update(body);
        let result = mac.finalize();
        let hex_sig = hex::encode(result.into_bytes());

        let header = format!("sha256={}", hex_sig);
        assert!(!verify_signature("wrong-secret", body, &header));
    }

    #[test]
    fn verify_signature_rejects_tampered_body() {
        let secret = "test-secret";
        let body = b"original body";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let result = mac.finalize();
        let hex_sig = hex::encode(result.into_bytes());

        let header = format!("sha256={}", hex_sig);
        // Verify against tampered body.
        assert!(!verify_signature(secret, b"tampered body", &header));
    }

    #[test]
    fn verify_signature_rejects_invalid_hex() {
        assert!(!verify_signature("secret", b"body", "sha256=not-valid-hex!!!"));
    }

    #[test]
    fn verify_signature_rejects_empty_signature() {
        assert!(!verify_signature("secret", b"body", ""));
    }

    // ── Payload parsing tests ────────────────────────────────────────

    #[test]
    fn parse_github_issues_event() {
        let body = serde_json::json!({
            "action": "opened",
            "issue": {
                "number": 42,
                "title": "Bug report",
                "body": "Something is broken",
                "html_url": "https://github.com/owner/repo/issues/42",
                "labels": [{"name": "bug"}, {"name": "urgent"}],
                "assignee": {"login": "octocat"}
            }
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let task = parse_webhook_payload(Some("issues"), Some("delivery-123"), &body_bytes);

        assert_eq!(task.title, "Bug report");
        assert_eq!(task.body, "Something is broken");
        assert_eq!(task.url, "https://github.com/owner/repo/issues/42");
        assert_eq!(task.labels, vec!["bug", "urgent"]);
        assert_eq!(task.assignee, Some("octocat".to_string()));
        assert!(task.source_id.starts_with("webhook:delivery-123:"));
        assert_eq!(task.metadata.get("github_event"), Some(&"issues".to_string()));
        assert_eq!(task.metadata.get("action"), Some(&"opened".to_string()));
        assert_eq!(task.metadata.get("issue_number"), Some(&"42".to_string()));
    }

    #[test]
    fn parse_github_pull_request_event() {
        let body = serde_json::json!({
            "action": "opened",
            "pull_request": {
                "number": 99,
                "title": "Add feature",
                "body": "This adds a new feature",
                "html_url": "https://github.com/owner/repo/pull/99",
                "labels": [{"name": "enhancement"}],
                "assignee": null
            }
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let task = parse_webhook_payload(Some("pull_request"), Some("delivery-456"), &body_bytes);

        assert_eq!(task.title, "Add feature");
        assert_eq!(task.body, "This adds a new feature");
        assert_eq!(task.labels, vec!["enhancement"]);
        assert_eq!(task.assignee, None);
        assert_eq!(task.metadata.get("pr_number"), Some(&"99".to_string()));
    }

    #[test]
    fn parse_github_other_event() {
        let body = serde_json::json!({"ref": "refs/heads/main"});
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let task = parse_webhook_payload(Some("push"), None, &body_bytes);

        assert_eq!(task.title, "GitHub event: push");
        assert_eq!(task.metadata.get("github_event"), Some(&"push".to_string()));
        // Body should be the raw JSON string.
        assert!(task.body.contains("refs/heads/main"));
    }

    #[test]
    fn parse_generic_webhook_with_title() {
        let body = serde_json::json!({
            "title": "Custom task",
            "data": "some data"
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let task = parse_webhook_payload(None, Some("generic-123"), &body_bytes);

        assert_eq!(task.title, "Custom task");
        assert!(task.source_id.starts_with("webhook:generic-123:"));
    }

    #[test]
    fn parse_generic_webhook_without_title() {
        let body = serde_json::json!({"key": "value"});
        let body_bytes = serde_json::to_vec(&body).unwrap();

        let task = parse_webhook_payload(None, None, &body_bytes);

        assert_eq!(task.title, "Webhook payload");
        // Auto-generated delivery ID (UUID format).
        assert!(task.source_id.starts_with("webhook:"));
    }

    #[test]
    fn parse_non_json_body() {
        let body = b"plain text body";

        let task = parse_webhook_payload(None, Some("plain-123"), body);

        assert_eq!(task.title, "Webhook payload");
        assert_eq!(task.body, "plain text body");
    }

    // ── WebhookRegistry tests ────────────────────────────────────────

    #[tokio::test]
    async fn registry_register_and_lookup() {
        let registry = WebhookRegistry::new();
        let (tx, _rx) = mpsc::channel(16);
        let wf_id = Uuid::new_v4();

        registry.register(wf_id, tx, Some("secret".to_string())).await;

        let result = registry.lookup(&wf_id).await;
        assert!(result.is_some());
        let (_, secret) = result.unwrap();
        assert_eq!(secret, Some("secret".to_string()));
    }

    #[tokio::test]
    async fn registry_lookup_missing() {
        let registry = WebhookRegistry::new();
        assert!(registry.lookup(&Uuid::new_v4()).await.is_none());
    }

    #[tokio::test]
    async fn registry_unregister() {
        let registry = WebhookRegistry::new();
        let (tx, _rx) = mpsc::channel(16);
        let wf_id = Uuid::new_v4();

        registry.register(wf_id, tx, None).await;
        assert_eq!(registry.len().await, 1);

        registry.unregister(&wf_id).await;
        assert_eq!(registry.len().await, 0);
        assert!(registry.lookup(&wf_id).await.is_none());
    }

    #[tokio::test]
    async fn registry_send_task_through_channel() {
        let registry = WebhookRegistry::new();
        let (tx, mut rx) = mpsc::channel(16);
        let wf_id = Uuid::new_v4();

        registry.register(wf_id, tx, None).await;

        let (sender, _) = registry.lookup(&wf_id).await.unwrap();
        let task = Task {
            source_id: "test-1".to_string(),
            title: "Test task".to_string(),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata: HashMap::new(),
        };
        sender.send(task).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.source_id, "test-1");
        assert_eq!(received.title, "Test task");
    }
}
