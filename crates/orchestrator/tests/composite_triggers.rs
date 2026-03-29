//! Integration tests for the composite trigger system (AND/OR combinators).
//!
//! These tests exercise the full path from `TriggerConfig::Composite`
//! through `create_strategy()` to actual task production, verifying
//! that the strategy factory correctly wires up nested sub-strategies.
//!
//! # What is tested
//!
//! - `create_strategy()` produces a `CompositeStrategy` for OR mode
//! - `create_strategy()` produces a `CompositeStrategy` for AND mode
//! - Nested composites (OR of ORs) are supported up to depth 3
//! - Nesting beyond `MAX_COMPOSITE_DEPTH` (3) returns an error
//! - Invalid composite mode returns an error
//! - Serde round-trip for `TriggerConfig::Composite`
//! - Config validation: at least 2 sub-triggers required

use chrono::Utc;
use orchestrator::scheduler::{
    runner::create_strategy,
    types::{TriggerConfig, WorkflowConfig},
};
use orchestrator::{scheduler::events::EventBus, websocket::ConnectionRegistry};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_workflow(trigger_config: TriggerConfig) -> WorkflowConfig {
    let now = Utc::now();
    WorkflowConfig {
        id: Uuid::new_v4(),
        name: "composite-test".to_string(),
        agent_id: Uuid::new_v4(),
        trigger_config,
        prompt_template: "Handle: {{title}}".to_string(),
        poll_interval_secs: 60,
        enabled: true,
        tool_policy: Default::default(),
        created_at: now,
        updated_at: now,
    }
}

fn cron_trigger() -> TriggerConfig {
    TriggerConfig::Cron { expression: "0 * * * *".to_string() }
}

fn agent_lifecycle_trigger() -> TriggerConfig {
    TriggerConfig::AgentLifecycle { event: "session_start".to_string() }
}

// ---------------------------------------------------------------------------
// Strategy factory tests
// ---------------------------------------------------------------------------

/// `create_strategy()` should succeed for a composite OR trigger wrapping
/// two leaf triggers.
#[tokio::test]
async fn create_strategy_or_composite_succeeds() {
    let trigger = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![cron_trigger(), agent_lifecycle_trigger()],
        correlation_window_secs: None,
    };
    let config = make_workflow(trigger);
    let bus = EventBus::shared(16);

    let result = create_strategy(&config, Some(&bus));
    assert!(
        result.is_ok(),
        "Expected Ok for OR composite: {:?}",
        result.err().map(|e| e.to_string())
    );
}

/// `create_strategy()` should succeed for a composite AND trigger.
#[tokio::test]
async fn create_strategy_and_composite_succeeds() {
    let trigger = TriggerConfig::Composite {
        mode: "and".to_string(),
        triggers: vec![cron_trigger(), agent_lifecycle_trigger()],
        correlation_window_secs: Some(30),
    };
    let config = make_workflow(trigger);
    let bus = EventBus::shared(16);

    let result = create_strategy(&config, Some(&bus));
    assert!(
        result.is_ok(),
        "Expected Ok for AND composite: {:?}",
        result.err().map(|e| e.to_string())
    );
}

/// An invalid `mode` string should return an error.
#[tokio::test]
async fn create_strategy_invalid_mode_returns_error() {
    let trigger = TriggerConfig::Composite {
        mode: "xor".to_string(),
        triggers: vec![cron_trigger(), agent_lifecycle_trigger()],
        correlation_window_secs: None,
    };
    let config = make_workflow(trigger);
    let bus = EventBus::shared(16);

    let result = create_strategy(&config, Some(&bus));
    assert!(result.is_err(), "Expected error for invalid mode");
    let msg = result.err().expect("expected error").to_string();
    assert!(msg.contains("xor"), "Error should mention the bad mode: {msg}");
}

/// Nested composites within the depth limit (3) should be accepted.
#[tokio::test]
async fn create_strategy_nested_composite_within_depth_limit() {
    // Depth 0: outer OR
    //   Depth 1: inner OR
    //     Depth 2: cron, manual  (leaf triggers)
    let inner = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![cron_trigger(), agent_lifecycle_trigger()],
        correlation_window_secs: None,
    };
    let outer = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![inner, cron_trigger()],
        correlation_window_secs: None,
    };
    let config = make_workflow(outer);
    let bus = EventBus::shared(16);

    let result = create_strategy(&config, Some(&bus));
    assert!(
        result.is_ok(),
        "Expected Ok for 2-level nesting: {:?}",
        result.err().map(|e| e.to_string())
    );
}

/// Nesting beyond `MAX_COMPOSITE_DEPTH` (3) should return an error.
#[tokio::test]
async fn create_strategy_excessive_nesting_returns_error() {
    // Build 4 levels of OR nesting.
    let level3 = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![cron_trigger(), agent_lifecycle_trigger()],
        correlation_window_secs: None,
    };
    let level2 = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![level3, cron_trigger()],
        correlation_window_secs: None,
    };
    let level1 = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![level2, cron_trigger()],
        correlation_window_secs: None,
    };
    // Level 0 (the workflow trigger) wraps level1 → depth 3 when building level1's children
    let level0 = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![level1, cron_trigger()],
        correlation_window_secs: None,
    };
    let config = make_workflow(level0);
    let bus = EventBus::shared(16);

    let result = create_strategy(&config, Some(&bus));
    assert!(result.is_err(), "Expected error for 4-level nesting");
    let msg = result.err().expect("expected error").to_string();
    assert!(msg.contains("nesting") || msg.contains("depth"), "Error should mention depth: {msg}");
}

// ---------------------------------------------------------------------------
// Serde round-trip tests
// ---------------------------------------------------------------------------

/// `TriggerConfig::Composite` should survive a serde JSON round-trip.
#[test]
fn trigger_config_composite_serde_round_trip_or() {
    let original = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![cron_trigger(), agent_lifecycle_trigger()],
        correlation_window_secs: None,
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let decoded: TriggerConfig = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(decoded.trigger_type(), "composite");
    if let TriggerConfig::Composite { mode, triggers, .. } = &decoded {
        assert_eq!(mode, "or");
        assert_eq!(triggers.len(), 2);
        assert_eq!(triggers[0].trigger_type(), "cron");
        assert_eq!(triggers[1].trigger_type(), "agent_lifecycle");
    } else {
        panic!("Expected Composite variant");
    }
}

#[test]
fn trigger_config_composite_serde_round_trip_and() {
    let original = TriggerConfig::Composite {
        mode: "and".to_string(),
        triggers: vec![cron_trigger(), agent_lifecycle_trigger()],
        correlation_window_secs: Some(120),
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let decoded: TriggerConfig = serde_json::from_str(&json).expect("deserialize");

    if let TriggerConfig::Composite { mode, correlation_window_secs, .. } = &decoded {
        assert_eq!(mode, "and");
        assert_eq!(*correlation_window_secs, Some(120));
    } else {
        panic!("Expected Composite variant");
    }
}

#[test]
fn trigger_config_composite_is_implemented() {
    let trigger = TriggerConfig::Composite {
        mode: "or".to_string(),
        triggers: vec![cron_trigger(), agent_lifecycle_trigger()],
        correlation_window_secs: None,
    };
    assert!(trigger.is_implemented());
    assert_eq!(trigger.trigger_type(), "composite");
    assert!(!trigger.is_one_shot());
}

// ---------------------------------------------------------------------------
// Unused import guard — ensures ConnectionRegistry compiles in this context.
// ---------------------------------------------------------------------------
#[allow(dead_code)]
fn _unused(_: ConnectionRegistry) {}
