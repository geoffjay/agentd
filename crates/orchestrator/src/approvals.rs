//! In-memory registry for pending tool approval requests.
//!
//! When an agent runs with `RequireApproval` tool policy, tool requests are held
//! here until a human approves or denies them via the API. Each pending approval
//! gets a oneshot channel — the WebSocket handler awaits the receiver while the
//! API endpoint sends the decision through the sender.

use crate::types::{ApprovalDecision, ApprovalStatus, PendingApproval};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

struct ApprovalEntry {
    approval: PendingApproval,
    /// None once the decision has been sent (resolved or timed out).
    tx: Option<oneshot::Sender<ApprovalDecision>>,
}

/// In-memory store of pending tool approval requests.
///
/// Thread-safe and cloneable (Arc-backed). Shared between the WebSocket handler
/// (which registers pending requests) and the API handlers (which resolve them).
#[derive(Clone)]
pub struct ApprovalRegistry {
    entries: Arc<RwLock<HashMap<Uuid, ApprovalEntry>>>,
    default_timeout_secs: u64,
}

impl ApprovalRegistry {
    /// Create a new registry with the given default timeout for pending approvals.
    pub fn new(default_timeout_secs: u64) -> Self {
        Self { entries: Arc::new(RwLock::new(HashMap::new())), default_timeout_secs }
    }

    /// Register a new pending approval and return the record + decision receiver.
    ///
    /// The caller should `.await` the receiver (with a timeout) and then send the
    /// appropriate control_response to the agent.
    pub async fn register(
        &self,
        agent_id: Uuid,
        request_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
    ) -> (PendingApproval, oneshot::Receiver<ApprovalDecision>) {
        let (tx, rx) = oneshot::channel();
        let now = Utc::now();
        let approval = PendingApproval {
            id: Uuid::new_v4(),
            agent_id,
            request_id,
            tool_name,
            tool_input,
            status: ApprovalStatus::Pending,
            created_at: now,
            expires_at: now + chrono::Duration::seconds(self.default_timeout_secs as i64),
        };
        let id = approval.id;
        self.entries
            .write()
            .await
            .insert(id, ApprovalEntry { approval: approval.clone(), tx: Some(tx) });
        (approval, rx)
    }

    /// Resolve a pending approval with a human decision.
    ///
    /// Sends the decision through the oneshot channel to unblock the waiting
    /// WebSocket handler task. Returns the updated approval record.
    pub async fn resolve(
        &self,
        id: &Uuid,
        decision: ApprovalDecision,
    ) -> anyhow::Result<PendingApproval> {
        let mut entries = self.entries.write().await;
        let entry =
            entries.get_mut(id).ok_or_else(|| anyhow::anyhow!("Approval {} not found", id))?;

        if entry.tx.is_none() {
            anyhow::bail!("Approval {} has already been resolved", id);
        }

        let tx = entry.tx.take().unwrap();
        entry.approval.status = match decision {
            ApprovalDecision::Approve => ApprovalStatus::Approved,
            ApprovalDecision::Deny => ApprovalStatus::Denied,
        };

        // Send decision. If the receiver was dropped (agent disconnected), that's ok.
        let _ = tx.send(decision);

        Ok(entry.approval.clone())
    }

    /// Mark a pending approval as timed out (called by the waiting task itself).
    pub async fn mark_timed_out(&self, id: &Uuid) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(id) {
            entry.approval.status = ApprovalStatus::TimedOut;
            entry.tx = None;
        }
    }

    /// Get a single approval by ID.
    pub async fn get(&self, id: &Uuid) -> Option<PendingApproval> {
        self.entries.read().await.get(id).map(|e| e.approval.clone())
    }

    /// List approvals, optionally filtered by agent and/or status.
    pub async fn list(
        &self,
        agent_id: Option<&Uuid>,
        status_filter: Option<&ApprovalStatus>,
    ) -> Vec<PendingApproval> {
        self.entries
            .read()
            .await
            .values()
            .filter(|e| {
                agent_id.map_or(true, |id| &e.approval.agent_id == id)
                    && status_filter.map_or(true, |s| &e.approval.status == s)
            })
            .map(|e| e.approval.clone())
            .collect()
    }

    /// Remove resolved (non-pending) approvals older than the given duration.
    pub async fn purge_resolved(&self, older_than: chrono::Duration) {
        let cutoff = Utc::now() - older_than;
        let mut entries = self.entries.write().await;
        entries.retain(|_, e| {
            e.approval.status == ApprovalStatus::Pending || e.approval.created_at > cutoff
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_resolve_approve() {
        let registry = ApprovalRegistry::new(300);
        let agent_id = Uuid::new_v4();

        let (approval, rx) = registry
            .register(
                agent_id,
                "req-1".to_string(),
                "Bash".to_string(),
                serde_json::json!({"command": "ls"}),
            )
            .await;

        assert_eq!(approval.status, ApprovalStatus::Pending);
        assert_eq!(approval.tool_name, "Bash");

        // Resolve it
        let resolved = registry.resolve(&approval.id, ApprovalDecision::Approve).await.unwrap();
        assert_eq!(resolved.status, ApprovalStatus::Approved);

        // Receiver should get the decision
        let decision = rx.await.unwrap();
        assert!(matches!(decision, ApprovalDecision::Approve));
    }

    #[tokio::test]
    async fn test_register_and_resolve_deny() {
        let registry = ApprovalRegistry::new(300);
        let (approval, rx) = registry
            .register(
                Uuid::new_v4(),
                "req-2".to_string(),
                "Write".to_string(),
                serde_json::json!({}),
            )
            .await;

        let resolved = registry.resolve(&approval.id, ApprovalDecision::Deny).await.unwrap();
        assert_eq!(resolved.status, ApprovalStatus::Denied);

        let decision = rx.await.unwrap();
        assert!(matches!(decision, ApprovalDecision::Deny));
    }

    #[tokio::test]
    async fn test_double_resolve_fails() {
        let registry = ApprovalRegistry::new(300);
        let (approval, _rx) = registry
            .register(
                Uuid::new_v4(),
                "req-3".to_string(),
                "Read".to_string(),
                serde_json::json!({}),
            )
            .await;

        registry.resolve(&approval.id, ApprovalDecision::Approve).await.unwrap();

        let result = registry.resolve(&approval.id, ApprovalDecision::Deny).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already been resolved"));
    }

    #[tokio::test]
    async fn test_resolve_not_found() {
        let registry = ApprovalRegistry::new(300);
        let result = registry.resolve(&Uuid::new_v4(), ApprovalDecision::Approve).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_mark_timed_out() {
        let registry = ApprovalRegistry::new(300);
        let (approval, _rx) = registry
            .register(
                Uuid::new_v4(),
                "req-4".to_string(),
                "Bash".to_string(),
                serde_json::json!({}),
            )
            .await;

        registry.mark_timed_out(&approval.id).await;

        let fetched = registry.get(&approval.id).await.unwrap();
        assert_eq!(fetched.status, ApprovalStatus::TimedOut);
    }

    #[tokio::test]
    async fn test_list_with_filters() {
        let registry = ApprovalRegistry::new(300);
        let agent1 = Uuid::new_v4();
        let agent2 = Uuid::new_v4();

        let (a1, _) =
            registry.register(agent1, "r1".into(), "Bash".into(), serde_json::json!({})).await;
        let (_a2, _) =
            registry.register(agent2, "r2".into(), "Read".into(), serde_json::json!({})).await;

        // Resolve the first one
        registry.resolve(&a1.id, ApprovalDecision::Approve).await.unwrap();

        // List all
        assert_eq!(registry.list(None, None).await.len(), 2);

        // List pending only
        let pending = registry.list(None, Some(&ApprovalStatus::Pending)).await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].agent_id, agent2);

        // List by agent
        let agent1_approvals = registry.list(Some(&agent1), None).await;
        assert_eq!(agent1_approvals.len(), 1);
    }
}
