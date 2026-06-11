//! Shared ToolPolicy JSON construction for MCP tools.
//!
//! The orchestrator's `ToolPolicy` enum is internally tagged
//! (`#[serde(tag = "mode", rename_all = "snake_case")]`), so the wire shape is
//! `{"mode": "allow_list", "tools": [...]}`. Centralizing the construction
//! here keeps every tool that accepts a (mode, tools) pair consistent with
//! that shape.

use serde_json::{json, Value};

/// All policy modes accepted by the orchestrator.
pub const POLICY_MODES: &[&str] =
    &["allow_all", "deny_all", "require_approval", "allow_list", "deny_list"];

/// Build internally-tagged ToolPolicy JSON from a mode string and optional
/// tool list.
///
/// Returns `Err` with a user-facing message for unknown modes or when a
/// list mode is missing its tools.
pub fn build_tool_policy(mode: &str, tools: Option<&[String]>) -> Result<Value, String> {
    match mode {
        "allow_all" | "deny_all" | "require_approval" => Ok(json!({ "mode": mode })),
        "allow_list" | "deny_list" => {
            let list = tools.unwrap_or_default();
            if list.is_empty() {
                return Err(format!(
                    "🔴 `{mode}` mode requires at least one tool name in the `tools` parameter."
                ));
            }
            Ok(json!({ "mode": mode, "tools": list }))
        }
        other => Err(format!(
            "🔴 Unknown policy mode `{other}`.\n Valid modes: {}",
            POLICY_MODES.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_modes_build_internally_tagged_json() {
        for mode in ["allow_all", "deny_all", "require_approval"] {
            let policy = build_tool_policy(mode, None).unwrap();
            assert_eq!(policy, json!({ "mode": mode }), "mode {mode}");
        }
    }

    #[test]
    fn list_modes_include_tools() {
        let tools = vec!["Read".to_string(), "Grep".to_string()];
        let policy = build_tool_policy("allow_list", Some(&tools)).unwrap();
        assert_eq!(policy, json!({ "mode": "allow_list", "tools": ["Read", "Grep"] }));

        let policy = build_tool_policy("deny_list", Some(&tools)).unwrap();
        assert_eq!(policy["mode"], "deny_list");
    }

    #[test]
    fn list_modes_require_tools() {
        assert!(build_tool_policy("allow_list", None).is_err());
        assert!(build_tool_policy("deny_list", Some(&[])).is_err());
    }

    #[test]
    fn unknown_mode_lists_valid_modes() {
        let err = build_tool_policy("bogus", None).unwrap_err();
        assert!(err.contains("allow_list"), "error should list valid modes: {err}");
    }

    #[test]
    fn output_deserializes_as_orchestrator_tool_policy() {
        // Round-trip against the real enum to lock the wire shape.
        let tools = vec!["Read".to_string()];
        let policy = build_tool_policy("allow_list", Some(&tools)).unwrap();
        let parsed: orchestrator::types::ToolPolicy = serde_json::from_value(policy).unwrap();
        assert!(matches!(parsed, orchestrator::types::ToolPolicy::AllowList { .. }));
    }
}
