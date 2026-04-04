//! Application state management for the ask service.
//!
//! Provides thread-safe access to question storage.
//! The [`AppState`] struct can be cloned cheaply as it wraps an `Arc`.

use crate::storage::QuestionStorage;
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

/// Thread-safe application state for the ask service.
///
/// Wraps the persistent [`QuestionStorage`] in an `Arc` for cheap cloning
/// and sharing across Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<QuestionStorage>,
}

impl AppState {
    /// Creates a new application state with the given storage backend.
    pub fn new_with_storage(storage: QuestionStorage) -> Self {
        Self { storage: Arc::new(storage) }
    }

    /// Expires questions whose TTL has elapsed.
    ///
    /// Called periodically by the background task in main.rs.
    pub async fn expire_questions(&self) -> Result<()> {
        let count = self.storage.expire_old().await?;
        if count > 0 {
            info!("Expired {} question(s)", count);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::QuestionStorage;
    use crate::types::{CreateQuestionRequest, QuestionPriority, QuestionStatus};

    async fn make_state() -> AppState {
        let storage = QuestionStorage::in_memory().await.unwrap();
        AppState::new_with_storage(storage)
    }

    fn make_request() -> CreateQuestionRequest {
        CreateQuestionRequest {
            agent_id: "test-agent".to_string(),
            workflow_id: None,
            dispatch_id: None,
            category: Some("health".to_string()),
            question: "What did you eat?".to_string(),
            context: None,
            priority: Some(QuestionPriority::Normal),
            expires_in_seconds: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_question() {
        let state = make_state().await;
        let req = make_request();
        let q = state.storage.create(&req).await.unwrap();

        assert_eq!(q.status, QuestionStatus::Pending);
        assert_eq!(q.agent_id, "test-agent");

        let retrieved = state.storage.get(&q.id).await.unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_expire_questions() {
        let state = make_state().await;
        // No expired questions — should succeed with 0 expired.
        state.expire_questions().await.unwrap();
    }
}
