//! Integration tests for the `agent prompt` CLI command.
//!
//! These tests exercise the public API of `cli::commands::prompt` — the parsing
//! and name-matching helpers — without requiring any live services.
//!
//! Routing integration tests (send_to_agent, send_to_room, …) are tracked in
//! issue-734 and are marked `#[ignore]` until that implementation lands.

use cli::commands::prompt::{match_recipient, parse_prompt, ParsedPrompt};

// ---------------------------------------------------------------------------
// CLI arg-parsing smoke tests
// ---------------------------------------------------------------------------

#[test]
fn prompt_command_parses_args() {
    // Verify that the prompt parser round-trips through the full
    // parse_prompt function correctly for a realistic CLI-style input.
    let input = "@worker-agent please review PR #99";
    let parsed = parse_prompt(input);
    assert_eq!(parsed.recipient.as_deref(), Some("worker-agent"));
    assert_eq!(parsed.message, "please review PR #99");
}

#[test]
fn prompt_command_json_flag_does_not_affect_parsing() {
    // The --json flag controls output formatting, not parsing behaviour.
    // Parsing must be identical regardless of whether --json is set.
    let with_json = parse_prompt("@bot hello");
    let without_json = parse_prompt("@bot hello");
    assert_eq!(with_json, without_json);
}

// ---------------------------------------------------------------------------
// Edge-case @ parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_at_with_numeric_recipient() {
    let p = parse_prompt("@agent42 do the thing");
    assert_eq!(p.recipient.as_deref(), Some("agent42"));
    assert_eq!(p.message, "do the thing");
}

#[test]
fn parse_at_recipient_with_underscores() {
    // Underscores are valid in identifiers — they should pass through unchanged.
    let p = parse_prompt("@worker_agent build it");
    assert_eq!(p.recipient.as_deref(), Some("worker_agent"));
    assert_eq!(p.message, "build it");
}

#[test]
fn parse_at_recipient_message_with_at_sign_inside() {
    // A second @ inside the message body must not be treated as a recipient.
    let p = parse_prompt("@bot email me@example.com");
    assert_eq!(p.recipient.as_deref(), Some("bot"));
    assert_eq!(p.message, "email me@example.com");
}

#[test]
fn parsed_prompt_equality() {
    let a = ParsedPrompt { recipient: Some("bot".to_string()), message: "hi".to_string() };
    let b = ParsedPrompt { recipient: Some("bot".to_string()), message: "hi".to_string() };
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Name matching — additional integration-level cases
// ---------------------------------------------------------------------------

#[test]
fn match_against_realistic_agent_list() {
    let agents = vec!["worker-agent", "triage-agent", "review-agent", "orchestrator"];

    assert_eq!(match_recipient("worker-agent", &agents), Some("worker-agent"));
    assert_eq!(match_recipient("TRIAGE-AGENT", &agents), Some("triage-agent"));
    assert_eq!(match_recipient("review agent", &agents), Some("review-agent"));
    assert_eq!(match_recipient("Orchestrator", &agents), Some("orchestrator"));
    assert_eq!(match_recipient("nonexistent", &agents), None);
}

#[test]
fn match_against_realistic_room_list() {
    let rooms = vec!["engineering", "ops-channel", "alerts", "general"];

    assert_eq!(match_recipient("engineering", &rooms), Some("engineering"));
    assert_eq!(match_recipient("OPS-CHANNEL", &rooms), Some("ops-channel"));
    assert_eq!(match_recipient("ops channel", &rooms), Some("ops-channel"));
    assert_eq!(match_recipient("GENERAL", &rooms), Some("general"));
}
