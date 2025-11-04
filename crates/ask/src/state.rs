//! Thread-safe application state management for the ask service.
//!
//! This module provides the core state management functionality using `Arc<RwLock<T>>`
//! for safe concurrent access from multiple API handlers. It tracks active questions,
//! notification cooldowns, and enforces rate limiting to prevent notification spam.
//!
//! # Thread Safety
//!
//! All state access is protected by async read-write locks, allowing multiple concurrent
//! readers or a single writer. The [`AppState`] struct can be cloned cheaply as it only
//! clones the `Arc` pointer.
//!
//! # Cooldown Logic
//!
//! To prevent overwhelming users with notifications, the state tracks the last time
//! a notification was sent for each check type. New notifications are only sent if
//! the cooldown period (default 30 minutes) has elapsed.
//!
//! # Question Lifecycle
//!
//! 1. **Created**: Question is created when a check triggers a notification
//! 2. **Pending**: Question awaits user response
//! 3. **Answered**: User has provided an answer
//! 4. **Expired**: Question timeout or manual expiration
//! 5. **Cleaned Up**: Old questions are periodically removed
//!
//! # Examples
//!
//! ```
//! use agentd_ask::state::AppState;
//! use agentd_ask::types::{QuestionInfo, CheckType, QuestionStatus};
//! use chrono::Utc;
//! use uuid::Uuid;
//!
//! # async fn example() {
//! let state = AppState::new();
//!
//! // Check if we can send a notification
//! if state.can_send_notification(CheckType::TmuxSessions).await {
//!     // Send notification and record it
//!     state.record_notification(CheckType::TmuxSessions).await;
//!
//!     // Store the question
//!     let question = QuestionInfo {
//!         question_id: Uuid::new_v4(),
//!         notification_id: Uuid::new_v4(),
//!         check_type: CheckType::TmuxSessions,
//!         asked_at: Utc::now(),
//!         status: QuestionStatus::Pending,
//!         answer: None,
//!     };
//!     state.add_question(question.clone()).await;
//!
//!     // Later, answer the question
//!     state.answer_question(&question.question_id, "yes".to_string()).await.unwrap();
//! }
//! # }
//! ```

use crate::types::{CheckType, QuestionInfo, QuestionStatus};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Thread-safe application state container.
///
/// Holds all shared state for the ask service, including questions and notification
/// cooldowns. Uses `Arc<RwLock<T>>` internally for cheap cloning and safe concurrent access.
///
/// # Cloning
///
/// Cloning [`AppState`] is cheap - it only clones the `Arc` pointer, not the underlying data.
/// All clones share the same state.
///
/// # Concurrency
///
/// Multiple tasks can safely read from and write to the state concurrently. The `RwLock`
/// allows multiple concurrent readers or a single writer.
///
/// # Examples
///
/// ```
/// use agentd_ask::state::AppState;
///
/// let state = AppState::new();
/// let state_clone = state.clone(); // Cheap clone, same underlying data
/// ```
#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<AppStateInner>>,
}

/// Internal state data protected by the lock.
///
/// This struct contains the actual data. It is not exposed publicly and is only
/// accessed through the [`AppState`] methods which handle locking.
struct AppStateInner {
    /// Active questions indexed by question ID
    questions: HashMap<Uuid, QuestionInfo>,
    /// Last notification sent timestamp per check type
    last_notification: HashMap<CheckType, DateTime<Utc>>,
    /// Notification cooldown period (default: 30 minutes)
    cooldown_duration: Duration,
}

impl AppState {
    /// Creates a new application state with default cooldown period.
    ///
    /// The default cooldown is 30 minutes, meaning notifications for the same
    /// check type won't be sent more frequently than once every 30 minutes.
    ///
    /// # Returns
    ///
    /// Returns a new [`AppState`] instance ready to use.
    ///
    /// # Examples
    ///
    /// ```
    /// use agentd_ask::state::AppState;
    ///
    /// let state = AppState::new();
    /// // State has 30-minute cooldown by default
    /// ```
    pub fn new() -> Self {
        Self::with_cooldown(Duration::minutes(30))
    }

    /// Creates a new application state with custom cooldown duration.
    ///
    /// Allows configuring a custom cooldown period for testing or different
    /// notification frequencies.
    ///
    /// # Arguments
    ///
    /// - `cooldown_duration` - The minimum time between notifications for the same check type
    ///
    /// # Returns
    ///
    /// Returns a new [`AppState`] instance with the specified cooldown.
    ///
    /// # Examples
    ///
    /// ```
    /// use agentd_ask::state::AppState;
    /// use chrono::Duration;
    ///
    /// // 10-minute cooldown for testing
    /// let state = AppState::with_cooldown(Duration::minutes(10));
    /// ```
    pub fn with_cooldown(cooldown_duration: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppStateInner {
                questions: HashMap::new(),
                last_notification: HashMap::new(),
                cooldown_duration,
            })),
        }
    }

    /// Checks if a notification can be sent for the given check type.
    ///
    /// Returns `true` if either no notification has been sent for this check type,
    /// or if the cooldown period has elapsed since the last notification.
    ///
    /// # Arguments
    ///
    /// - `check_type` - The type of check to verify cooldown for
    ///
    /// # Returns
    ///
    /// Returns `true` if notification is allowed, `false` if still in cooldown.
    ///
    /// # Examples
    ///
    /// ```
    /// use agentd_ask::state::AppState;
    /// use agentd_ask::types::CheckType;
    ///
    /// # async fn example() {
    /// let state = AppState::new();
    ///
    /// if state.can_send_notification(CheckType::TmuxSessions).await {
    ///     // Send notification
    ///     state.record_notification(CheckType::TmuxSessions).await;
    /// }
    /// # }
    /// ```
    pub async fn can_send_notification(&self, check_type: CheckType) -> bool {
        let state = self.inner.read().await;
        match state.last_notification.get(&check_type) {
            None => true,
            Some(last_time) => {
                let now = Utc::now();
                let elapsed = now - *last_time;
                elapsed > state.cooldown_duration
            }
        }
    }

    /// Records that a notification was sent for the given check type.
    ///
    /// Updates the last notification timestamp to the current time, starting the
    /// cooldown period. Future calls to [`can_send_notification`](Self::can_send_notification)
    /// will return `false` until the cooldown expires.
    ///
    /// # Arguments
    ///
    /// - `check_type` - The type of check that triggered the notification
    ///
    /// # Examples
    ///
    /// ```
    /// use agentd_ask::state::AppState;
    /// use agentd_ask::types::CheckType;
    ///
    /// # async fn example() {
    /// let state = AppState::new();
    ///
    /// // After sending a notification
    /// state.record_notification(CheckType::TmuxSessions).await;
    ///
    /// // Now in cooldown
    /// assert!(!state.can_send_notification(CheckType::TmuxSessions).await);
    /// # }
    /// ```
    pub async fn record_notification(&self, check_type: CheckType) {
        let mut state = self.inner.write().await;
        state.last_notification.insert(check_type, Utc::now());
    }

    /// Adds a new question to the application state.
    ///
    /// Stores the question so it can be retrieved later when the user provides
    /// an answer or when checking question status.
    ///
    /// # Arguments
    ///
    /// - `question` - The [`QuestionInfo`] to store
    ///
    /// # Examples
    ///
    /// ```
    /// use agentd_ask::state::AppState;
    /// use agentd_ask::types::{QuestionInfo, CheckType, QuestionStatus};
    /// use chrono::Utc;
    /// use uuid::Uuid;
    ///
    /// # async fn example() {
    /// let state = AppState::new();
    ///
    /// let question = QuestionInfo {
    ///     question_id: Uuid::new_v4(),
    ///     notification_id: Uuid::new_v4(),
    ///     check_type: CheckType::TmuxSessions,
    ///     asked_at: Utc::now(),
    ///     status: QuestionStatus::Pending,
    ///     answer: None,
    /// };
    ///
    /// state.add_question(question).await;
    /// # }
    /// ```
    pub async fn add_question(&self, question: QuestionInfo) {
        let mut state = self.inner.write().await;
        state.questions.insert(question.question_id, question);
    }

    /// Retrieves a question by its ID.
    ///
    /// # Arguments
    ///
    /// - `question_id` - The UUID of the question to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Some(QuestionInfo)` if found, `None` if the question doesn't exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use agentd_ask::state::AppState;
    /// use uuid::Uuid;
    ///
    /// # async fn example() {
    /// let state = AppState::new();
    /// let question_id = Uuid::new_v4();
    ///
    /// match state.get_question(&question_id).await {
    ///     Some(question) => println!("Found question: {:?}", question),
    ///     None => println!("Question not found"),
    /// }
    /// # }
    /// ```
    pub async fn get_question(&self, question_id: &Uuid) -> Option<QuestionInfo> {
        let state = self.inner.read().await;
        state.questions.get(question_id).cloned()
    }

    /// Updates a question with a user's answer.
    ///
    /// Changes the question status to [`QuestionStatus::Answered`] and stores the
    /// provided answer. The question must be in [`QuestionStatus::Pending`] state.
    ///
    /// # Arguments
    ///
    /// - `question_id` - The UUID of the question to answer
    /// - `answer` - The user's answer as a string
    ///
    /// # Returns
    ///
    /// Returns `Ok(QuestionInfo)` with the updated question on success, or `Err(String)`
    /// if the question doesn't exist or is not pending.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Question with the given ID doesn't exist
    /// - Question is not in pending status (already answered or expired)
    ///
    /// # Examples
    ///
    /// ```
    /// use agentd_ask::state::AppState;
    /// use agentd_ask::types::{QuestionInfo, CheckType, QuestionStatus};
    /// use chrono::Utc;
    /// use uuid::Uuid;
    ///
    /// # async fn example() {
    /// let state = AppState::new();
    ///
    /// let question = QuestionInfo {
    ///     question_id: Uuid::new_v4(),
    ///     notification_id: Uuid::new_v4(),
    ///     check_type: CheckType::TmuxSessions,
    ///     asked_at: Utc::now(),
    ///     status: QuestionStatus::Pending,
    ///     answer: None,
    /// };
    ///
    /// state.add_question(question.clone()).await;
    ///
    /// match state.answer_question(&question.question_id, "yes".to_string()).await {
    ///     Ok(updated) => println!("Question answered: {:?}", updated.answer),
    ///     Err(e) => eprintln!("Failed to answer: {}", e),
    /// }
    /// # }
    /// ```
    pub async fn answer_question(
        &self,
        question_id: &Uuid,
        answer: String,
    ) -> Result<QuestionInfo, String> {
        let mut state = self.inner.write().await;

        let question = state
            .questions
            .get_mut(question_id)
            .ok_or_else(|| format!("Question {question_id} not found"))?;

        if question.status != QuestionStatus::Pending {
            return Err(format!(
                "Question {} is not pending (status: {:?})",
                question_id, question.status
            ));
        }

        question.status = QuestionStatus::Answered;
        question.answer = Some(answer);

        Ok(question.clone())
    }

    /// Mark a question as expired
    #[allow(dead_code)]
    pub async fn expire_question(&self, question_id: &Uuid) -> Result<(), String> {
        let mut state = self.inner.write().await;

        let question = state
            .questions
            .get_mut(question_id)
            .ok_or_else(|| format!("Question {question_id} not found"))?;

        question.status = QuestionStatus::Expired;
        Ok(())
    }

    /// Get all active (pending) questions
    #[allow(dead_code)]
    pub async fn get_active_questions(&self) -> Vec<QuestionInfo> {
        let state = self.inner.read().await;
        state.questions.values().filter(|q| q.status == QuestionStatus::Pending).cloned().collect()
    }

    /// Cleans up old questions from memory.
    ///
    /// Removes questions that are older than 24 hours UNLESS they are still pending.
    /// This prevents the state from growing unbounded while preserving actionable questions.
    ///
    /// # Cleanup Rules
    ///
    /// - Questions older than 24 hours AND not pending are removed
    /// - Questions older than 24 hours but still pending are kept
    /// - Recent questions are always kept
    ///
    /// This method is typically called periodically by a background task.
    ///
    /// # Examples
    ///
    /// ```
    /// use agentd_ask::state::AppState;
    ///
    /// # async fn example() {
    /// let state = AppState::new();
    ///
    /// // Periodically clean up old questions
    /// state.cleanup_old_questions().await;
    /// # }
    /// ```
    pub async fn cleanup_old_questions(&self) {
        let mut state = self.inner.write().await;
        let cutoff = Utc::now() - Duration::hours(24);

        state.questions.retain(|_, question| {
            question.asked_at > cutoff || question.status == QuestionStatus::Pending
        });
    }

    /// Get the cooldown duration
    #[allow(dead_code)]
    pub async fn get_cooldown_duration(&self) -> Duration {
        let state = self.inner.read().await;
        state.cooldown_duration
    }

    /// Get the last notification time for a check type
    #[allow(dead_code)]
    pub async fn get_last_notification_time(&self, check_type: CheckType) -> Option<DateTime<Utc>> {
        let state = self.inner.read().await;
        state.last_notification.get(&check_type).copied()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_can_send_notification_initial() {
        let state = AppState::new();
        assert!(state.can_send_notification(CheckType::TmuxSessions).await);
    }

    #[tokio::test]
    async fn test_can_send_notification_after_cooldown() {
        let state = AppState::with_cooldown(Duration::milliseconds(10));

        state.record_notification(CheckType::TmuxSessions).await;
        assert!(!state.can_send_notification(CheckType::TmuxSessions).await);

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        assert!(state.can_send_notification(CheckType::TmuxSessions).await);
    }

    #[tokio::test]
    async fn test_add_and_get_question() {
        let state = AppState::new();
        let question = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        };

        state.add_question(question.clone()).await;
        let retrieved = state.get_question(&question.question_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().question_id, question.question_id);
    }

    #[tokio::test]
    async fn test_answer_question() {
        let state = AppState::new();
        let question = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        };

        state.add_question(question.clone()).await;
        let result = state.answer_question(&question.question_id, "yes".to_string()).await;

        assert!(result.is_ok());
        let answered = result.unwrap();
        assert_eq!(answered.status, QuestionStatus::Answered);
        assert_eq!(answered.answer, Some("yes".to_string()));
    }

    #[tokio::test]
    async fn test_answer_nonexistent_question() {
        let state = AppState::new();
        let result = state.answer_question(&Uuid::new_v4(), "yes".to_string()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_answer_already_answered_question() {
        let state = AppState::new();
        let question = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        };

        state.add_question(question.clone()).await;

        // Answer once
        state.answer_question(&question.question_id, "yes".to_string()).await.unwrap();

        // Try to answer again
        let result = state.answer_question(&question.question_id, "no".to_string()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not pending"));
    }

    #[tokio::test]
    async fn test_expire_question() {
        let state = AppState::new();
        let question = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        };

        state.add_question(question.clone()).await;
        let result = state.expire_question(&question.question_id).await;
        assert!(result.is_ok());

        let expired = state.get_question(&question.question_id).await.unwrap();
        assert_eq!(expired.status, QuestionStatus::Expired);
    }

    #[tokio::test]
    async fn test_expire_nonexistent_question() {
        let state = AppState::new();
        let result = state.expire_question(&Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_active_questions() {
        let state = AppState::new();

        let pending1 = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        };

        let pending2 = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        };

        let answered = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Answered,
            answer: Some("yes".to_string()),
        };

        state.add_question(pending1.clone()).await;
        state.add_question(pending2.clone()).await;
        state.add_question(answered.clone()).await;

        let active = state.get_active_questions().await;
        assert_eq!(active.len(), 2);
        assert!(active.iter().all(|q| q.status == QuestionStatus::Pending));
    }

    #[tokio::test]
    async fn test_cleanup_old_questions() {
        let state = AppState::new();

        // Add an old answered question (25 hours ago)
        let old_answered = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now() - Duration::hours(25),
            status: QuestionStatus::Answered,
            answer: Some("yes".to_string()),
        };

        // Add a recent answered question
        let recent_answered = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now() - Duration::hours(1),
            status: QuestionStatus::Answered,
            answer: Some("no".to_string()),
        };

        // Add an old pending question (should be kept)
        let old_pending = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now() - Duration::hours(25),
            status: QuestionStatus::Pending,
            answer: None,
        };

        state.add_question(old_answered.clone()).await;
        state.add_question(recent_answered.clone()).await;
        state.add_question(old_pending.clone()).await;

        state.cleanup_old_questions().await;

        // Old answered should be removed
        assert!(state.get_question(&old_answered.question_id).await.is_none());

        // Recent answered should remain
        assert!(state.get_question(&recent_answered.question_id).await.is_some());

        // Old pending should remain (still actionable)
        assert!(state.get_question(&old_pending.question_id).await.is_some());
    }

    #[tokio::test]
    async fn test_record_and_check_notification() {
        let state = AppState::with_cooldown(Duration::seconds(60));

        // First check should allow notification
        assert!(state.can_send_notification(CheckType::TmuxSessions).await);

        // Record notification
        state.record_notification(CheckType::TmuxSessions).await;

        // Second check should block notification
        assert!(!state.can_send_notification(CheckType::TmuxSessions).await);
    }

    #[tokio::test]
    async fn test_get_cooldown_duration() {
        let state = AppState::with_cooldown(Duration::minutes(15));
        let cooldown = state.get_cooldown_duration().await;
        assert_eq!(cooldown.num_minutes(), 15);
    }

    #[tokio::test]
    async fn test_get_last_notification_time() {
        let state = AppState::new();

        // Initially no notification
        assert!(state.get_last_notification_time(CheckType::TmuxSessions).await.is_none());

        // Record a notification
        state.record_notification(CheckType::TmuxSessions).await;

        // Should have a timestamp now
        let timestamp = state.get_last_notification_time(CheckType::TmuxSessions).await;
        assert!(timestamp.is_some());

        // Timestamp should be recent
        let elapsed = Utc::now() - timestamp.unwrap();
        assert!(elapsed < Duration::seconds(1));
    }

    #[tokio::test]
    async fn test_concurrent_question_access() {
        let state = AppState::new();

        let question = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        };

        state.add_question(question.clone()).await;

        // Spawn multiple concurrent tasks to read the question
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let state_clone = state.clone();
                let question_id = question.question_id;
                tokio::spawn(async move { state_clone.get_question(&question_id).await })
            })
            .collect();

        // All tasks should successfully read the question
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some());
        }
    }

    #[tokio::test]
    async fn test_concurrent_notification_recording() {
        let state = AppState::with_cooldown(Duration::milliseconds(10));

        // Spawn multiple concurrent tasks to record notifications
        let handles: Vec<_> = (0..5)
            .map(|_| {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    state_clone.record_notification(CheckType::TmuxSessions).await;
                })
            })
            .collect();

        // All tasks should complete without panicking
        for handle in handles {
            handle.await.unwrap();
        }

        // Should have a recorded notification
        assert!(!state.can_send_notification(CheckType::TmuxSessions).await);
    }

    #[tokio::test]
    async fn test_default_app_state() {
        let state = AppState::default();
        let cooldown = state.get_cooldown_duration().await;
        assert_eq!(cooldown.num_minutes(), 30);
    }

    #[tokio::test]
    async fn test_cooldown_boundary_condition() {
        let state = AppState::with_cooldown(Duration::milliseconds(50));

        state.record_notification(CheckType::TmuxSessions).await;

        // Should not be able to send immediately
        assert!(!state.can_send_notification(CheckType::TmuxSessions).await);

        // Wait for cooldown to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;

        // Should be able to send now
        assert!(state.can_send_notification(CheckType::TmuxSessions).await);
    }

    #[tokio::test]
    async fn test_multiple_check_types_independent() {
        // This test ensures different CheckType values are tracked independently
        // Currently we only have TmuxSessions, but this demonstrates the pattern
        let state = AppState::with_cooldown(Duration::seconds(60));

        state.record_notification(CheckType::TmuxSessions).await;
        assert!(!state.can_send_notification(CheckType::TmuxSessions).await);

        // If we add more check types in the future, they should be independent
        // For now, just verify the single type works correctly
    }
}
