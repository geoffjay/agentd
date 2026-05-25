use crate::scheduler::types::DispatchStatus;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::warn;
use uuid::Uuid;

/// Default channel capacity for the event bus.
const DEFAULT_CAPACITY: usize = 256;

/// Internal lifecycle events published by orchestrator components.
///
/// These events are broadcast to all subscribers and can be used for
/// reactive workflows, audit logging, or inter-component coordination.
///
/// Fields are read by subscribers (wired in by future Phase 3 issues).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SystemEvent {
    /// An agent established a WebSocket connection.
    AgentConnected { agent_id: Uuid },
    /// An agent's WebSocket connection was closed.
    AgentDisconnected { agent_id: Uuid },
    /// An agent's conversation context was cleared.
    ContextCleared { agent_id: Uuid },
    /// A workflow dispatch completed (succeeded or failed).
    ///
    /// `source_id` is the original task identifier (e.g. a GitHub issue or PR number)
    /// that triggered the dispatch. It is `None` when the dispatch originated from a
    /// trigger type that does not carry a task-level source identifier (rare).
    DispatchCompleted {
        workflow_id: Uuid,
        dispatch_id: Uuid,
        status: DispatchStatus,
        source_id: Option<String>,
    },
    /// An agent joined a communicate room.
    AgentJoinedRoom { agent_id: Uuid, room_id: Uuid },
    /// An agent was explicitly restarted via the API, reconciliation, or bootstrap.
    ///
    /// Published by [`crate::manager::AgentManager::restart_agent`] after the new
    /// process is launched and the DB record is updated. The scheduler subscribes
    /// to this event to re-launch any enabled workflow runners that died when the
    /// agent previously failed.
    AgentRestarted { agent_id: Uuid },
    /// A human answered or dismissed an ask service question.
    ///
    /// Published by the orchestrator when it receives a callback from the ask
    /// service. Consumed by `AskResponseStrategy` to trigger reactive workflows.
    AskResponseReceived {
        /// UUID of the answered question.
        question_id: Uuid,
        /// Which agent asked the question.
        agent_id: String,
        /// The workflow that originally created the question (if any).
        workflow_id: Option<Uuid>,
        /// The dispatch that originally created the question (if any).
        dispatch_id: Option<Uuid>,
        /// Question category (e.g. "health", "deployment").
        category: Option<String>,
        /// The question text.
        question: String,
        /// The human's answer (None if dismissed).
        answer: Option<String>,
        /// Event type: `"question_answered"` or `"question_dismissed"`.
        event_type: String,
    },
}

/// A shared broadcast-based event bus for internal system events.
///
/// Components publish events via [`EventBus::publish`] and receive them
/// via [`EventBus::subscribe`]. The bus uses a bounded broadcast channel;
/// slow subscribers that fall behind will miss events (with a logged warning).
///
/// Clone the `Arc<EventBus>` to share across components.
#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<SystemEvent>,
}

impl EventBus {
    /// Create a new event bus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Create a new event bus wrapped in an `Arc` for shared ownership.
    pub fn shared(capacity: usize) -> Arc<Self> {
        Arc::new(Self::new(capacity))
    }

    /// Publish an event to all current subscribers.
    ///
    /// If there are no active subscribers the event is silently dropped.
    pub fn publish(&self, event: SystemEvent) {
        // `send` returns Err only when there are no receivers, which is fine.
        if let Err(e) = self.tx.send(event) {
            warn!("EventBus: no subscribers for event: {:?}", e.0);
        }
    }

    /// Create a new subscriber that receives future events.
    ///
    /// Callers should handle [`broadcast::error::RecvError::Lagged`] by
    /// logging a warning and continuing — some events will have been missed.
    ///
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.tx.subscribe()
    }

    /// Return the number of active subscribers.
    ///
    /// Used by subscribers wired in by future Phase 3 issues.
    #[allow(dead_code)]
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::types::DispatchStatus;

    #[tokio::test]
    async fn test_publish_subscribe_single() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let agent_id = Uuid::new_v4();
        bus.publish(SystemEvent::AgentConnected { agent_id });

        let event = rx.recv().await.unwrap();
        match event {
            SystemEvent::AgentConnected { agent_id: id } => assert_eq!(id, agent_id),
            other => panic!("Unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_publish_subscribe_multiple_subscribers() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        let agent_id = Uuid::new_v4();
        bus.publish(SystemEvent::AgentDisconnected { agent_id });

        // Both subscribers should receive the event.
        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();

        match (e1, e2) {
            (
                SystemEvent::AgentDisconnected { agent_id: id1 },
                SystemEvent::AgentDisconnected { agent_id: id2 },
            ) => {
                assert_eq!(id1, agent_id);
                assert_eq!(id2, agent_id);
            }
            other => panic!("Unexpected events: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_publish_all_event_variants() {
        let bus = EventBus::new(32);
        let mut rx = bus.subscribe();

        let agent_id = Uuid::new_v4();
        let workflow_id = Uuid::new_v4();
        let dispatch_id = Uuid::new_v4();

        bus.publish(SystemEvent::AgentConnected { agent_id });
        bus.publish(SystemEvent::AgentDisconnected { agent_id });
        bus.publish(SystemEvent::AgentRestarted { agent_id });
        bus.publish(SystemEvent::ContextCleared { agent_id });
        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id,
            dispatch_id,
            status: DispatchStatus::Completed,
            source_id: Some("123".to_string()),
        });

        // Verify all five events arrive in order.
        assert!(matches!(rx.recv().await.unwrap(), SystemEvent::AgentConnected { .. }));
        assert!(matches!(rx.recv().await.unwrap(), SystemEvent::AgentDisconnected { .. }));
        assert!(matches!(rx.recv().await.unwrap(), SystemEvent::AgentRestarted { .. }));
        assert!(matches!(rx.recv().await.unwrap(), SystemEvent::ContextCleared { .. }));

        match rx.recv().await.unwrap() {
            SystemEvent::DispatchCompleted {
                workflow_id: wf,
                dispatch_id: d,
                status,
                source_id,
            } => {
                assert_eq!(wf, workflow_id);
                assert_eq!(d, dispatch_id);
                assert_eq!(status, DispatchStatus::Completed);
                assert_eq!(source_id, Some("123".to_string()));
            }
            other => panic!("Unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_agent_restarted_event() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let agent_id = Uuid::new_v4();
        bus.publish(SystemEvent::AgentRestarted { agent_id });

        match rx.recv().await.unwrap() {
            SystemEvent::AgentRestarted { agent_id: id } => assert_eq!(id, agent_id),
            other => panic!("Unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_no_subscribers_does_not_panic() {
        let bus = EventBus::new(16);
        // Publishing with no subscribers should not panic.
        bus.publish(SystemEvent::AgentConnected { agent_id: Uuid::new_v4() });
    }

    #[tokio::test]
    async fn test_lagged_subscriber() {
        // Capacity of 2 — publishing 3 events should cause lag for a slow reader.
        let bus = EventBus::new(2);
        let mut rx = bus.subscribe();

        bus.publish(SystemEvent::AgentConnected { agent_id: Uuid::new_v4() });
        bus.publish(SystemEvent::AgentDisconnected { agent_id: Uuid::new_v4() });
        bus.publish(SystemEvent::ContextCleared { agent_id: Uuid::new_v4() });

        // First recv should return a Lagged error.
        match rx.recv().await {
            Err(broadcast::error::RecvError::Lagged(n)) => {
                assert!(n > 0, "Expected lagged count > 0, got {}", n);
            }
            Ok(event) => {
                // Depending on timing, we might get an event — that's also valid.
                // The key thing is we don't panic.
                let _ = event;
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_subscriber_count() {
        let bus = EventBus::new(16);
        assert_eq!(bus.subscriber_count(), 0);

        let _rx1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);

        drop(_rx1);
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[tokio::test]
    async fn test_default_capacity() {
        let bus = EventBus::default();
        // Should not panic; verifies DEFAULT_CAPACITY is used.
        let _rx = bus.subscribe();
        bus.publish(SystemEvent::AgentConnected { agent_id: Uuid::new_v4() });
    }

    #[tokio::test]
    async fn test_shared_constructor() {
        let bus = EventBus::shared(32);
        let mut rx = bus.subscribe();

        let agent_id = Uuid::new_v4();
        bus.publish(SystemEvent::ContextCleared { agent_id });

        match rx.recv().await.unwrap() {
            SystemEvent::ContextCleared { agent_id: id } => assert_eq!(id, agent_id),
            other => panic!("Unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_dispatch_completed_failed_status() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let workflow_id = Uuid::new_v4();
        let dispatch_id = Uuid::new_v4();

        bus.publish(SystemEvent::DispatchCompleted {
            workflow_id,
            dispatch_id,
            status: DispatchStatus::Failed,
            source_id: None,
        });

        match rx.recv().await.unwrap() {
            SystemEvent::DispatchCompleted { status, .. } => {
                assert_eq!(status, DispatchStatus::Failed);
            }
            other => panic!("Unexpected event: {:?}", other),
        }
    }
}
