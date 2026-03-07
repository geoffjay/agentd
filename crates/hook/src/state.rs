//! Thread-safe application state for the hook service.
//!
//! Maintains a ring buffer of recorded hook events and exposes methods for
//! appending and querying events. The [`AppState`] struct is cheaply cloneable.

use crate::{config::HookConfig, types::RecordedEvent};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Thread-safe application state container.
///
/// Wraps a `RwLock` protected inner state. Cloning is cheap — it only clones
/// the `Arc` pointer.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<AppStateInner>>,
}

struct AppStateInner {
    /// Ring buffer of recorded events (oldest first)
    events: Vec<RecordedEvent>,
    /// Maximum number of events to retain
    history_size: usize,
    /// Service configuration
    config: HookConfig,
}

impl AppState {
    /// Create a new state container with the given configuration.
    pub fn new(config: HookConfig) -> Self {
        let history_size = config.history_size;
        Self {
            inner: Arc::new(RwLock::new(AppStateInner {
                events: Vec::with_capacity(history_size.min(1024)),
                history_size,
                config,
            })),
        }
    }

    /// Push a new event into the ring buffer, evicting the oldest if at capacity.
    pub async fn push_event(&self, event: RecordedEvent) {
        let mut state = self.inner.write().await;
        if state.events.len() >= state.history_size {
            state.events.remove(0);
        }
        state.events.push(event);
    }

    /// Return all retained events (oldest first).
    pub async fn all_events(&self) -> Vec<RecordedEvent> {
        let state = self.inner.read().await;
        state.events.clone()
    }

    /// Return the most recent `limit` events (newest first).
    pub async fn recent_events(&self, limit: usize) -> Vec<RecordedEvent> {
        let state = self.inner.read().await;
        state.events.iter().rev().take(limit).cloned().collect()
    }

    /// Find a single event by its UUID.
    pub async fn get_event(&self, id: &Uuid) -> Option<RecordedEvent> {
        let state = self.inner.read().await;
        state.events.iter().find(|e| &e.id == id).cloned()
    }

    /// Return the total number of stored events.
    pub async fn event_count(&self) -> usize {
        let state = self.inner.read().await;
        state.events.len()
    }

    /// Return a clone of the current configuration.
    pub async fn config(&self) -> HookConfig {
        let state = self.inner.read().await;
        state.config.clone()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(HookConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{HookEvent, HookKind};
    use chrono::Utc;

    fn make_event(command: &str, exit_code: i32) -> RecordedEvent {
        RecordedEvent {
            id: Uuid::new_v4(),
            received_at: Utc::now(),
            event: HookEvent {
                kind: HookKind::Shell,
                command: command.to_string(),
                exit_code,
                duration_ms: 100,
                output: None,
                metadata: Default::default(),
            },
        }
    }

    #[tokio::test]
    async fn test_initial_state_is_empty() {
        let state = AppState::default();
        assert_eq!(state.event_count().await, 0);
        assert!(state.all_events().await.is_empty());
    }

    #[tokio::test]
    async fn test_push_and_retrieve() {
        let state = AppState::default();
        let ev = make_event("cargo build", 0);
        let id = ev.id;
        state.push_event(ev).await;
        assert_eq!(state.event_count().await, 1);
        assert!(state.get_event(&id).await.is_some());
    }

    #[tokio::test]
    async fn test_ring_buffer_evicts_oldest() {
        let mut config = HookConfig::default();
        config.history_size = 3;
        let state = AppState::new(config);
        let ids: Vec<_> = (0..5).map(|i| {
            let ev = make_event(&format!("cmd-{i}"), 0);
            let id = ev.id;
            (ev, id)
        }).collect();

        for (ev, _) in &ids {
            state.push_event(ev.clone()).await;
        }
        assert_eq!(state.event_count().await, 3);
        // Oldest two should be gone
        assert!(state.get_event(&ids[0].1).await.is_none());
        assert!(state.get_event(&ids[1].1).await.is_none());
        // Newest three should be present
        assert!(state.get_event(&ids[2].1).await.is_some());
        assert!(state.get_event(&ids[4].1).await.is_some());
    }

    #[tokio::test]
    async fn test_recent_events_newest_first() {
        let state = AppState::default();
        for i in 0..5u32 {
            let mut ev = make_event(&format!("cmd-{i}"), 0);
            ev.event.duration_ms = i as u64;
            state.push_event(ev).await;
        }
        let recent = state.recent_events(3).await;
        assert_eq!(recent.len(), 3);
        // Newest should be first
        assert_eq!(recent[0].event.duration_ms, 4);
    }

    #[tokio::test]
    async fn test_get_nonexistent_event_returns_none() {
        let state = AppState::default();
        assert!(state.get_event(&Uuid::new_v4()).await.is_none());
    }
}
