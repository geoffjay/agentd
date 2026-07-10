//! Agent and workflow creation/management tool implementations.
//!
//! These tools wrap the orchestrator's `POST /agents` and `/workflows` CRUD
//! routes so MCP clients (and the agentd-architect built-in agent) can
//! provision live resources through the API.
//!
//! # Deliberately not exposed
//!
//! `create_agent` accepts no `env` map: agent env vars commonly carry
//! `ANTHROPIC_API_KEY`-class secrets, and values passed through an MCP tool
//! transit model context and transcripts. `get_agent` redacts env values for
//! the same reason — accepting them here would undo that. `interactive`,
//! `worktree`, `shell`, and other power-user knobs are also omitted to keep
//! the tool schema small; use the CLI or API directly.

use crate::client::AgentdClient;
use crate::tools::policy::build_tool_policy;
use serde_json::{json, Value};

/// Trigger variant names accepted by `CreateWorkflowRequest.trigger_config`
/// (the internally-tagged `type` field of the orchestrator's TriggerConfig).
pub const TRIGGER_TYPES: &[&str] = &[
    "github_issues",
    "github_pull_requests",
    "cron",
    "delay",
    "agent_lifecycle",
    "dispatch_result",
    "webhook",
    "manual",
    "agent_idle",
    "linear_issues",
    "composite",
    "queue",
    "ask_response",
    "gitlab_issues",
    "gitlab_merge_requests",
];

// ── create_agent ───────────────────────────────────────────────────────────

/// Build the `POST /agents` body, validating the cheap things client-side.
#[allow(clippy::too_many_arguments)]
fn build_create_agent_body(
    name: &str,
    working_dir: &str,
    model: Option<&str>,
    system_prompt: Option<&str>,
    append_system_prompt: bool,
    prompt: Option<&str>,
    tool_policy_mode: Option<&str>,
    tool_policy_tools: Option<&[String]>,
    rooms: Option<&[String]>,
) -> Result<Value, String> {
    if name.trim().is_empty() {
        return Err("🔴 `name` must not be empty.".to_string());
    }
    if working_dir.trim().is_empty() {
        return Err("🔴 `working_dir` must not be empty.".to_string());
    }

    let mut body = json!({
        "name": name,
        "working_dir": working_dir,
    });

    if let Some(model) = model {
        body["model"] = json!(model);
    }
    if let Some(system_prompt) = system_prompt {
        body["system_prompt"] = json!(system_prompt);
        body["append_system_prompt"] = json!(append_system_prompt);
    }
    if let Some(prompt) = prompt {
        body["prompt"] = json!(prompt);
    }
    if let Some(mode) = tool_policy_mode {
        body["tool_policy"] = build_tool_policy(mode, tool_policy_tools)?;
    }
    if let Some(rooms) = rooms {
        body["rooms"] = json!(rooms);
    }

    Ok(body)
}

/// Create a new agent via `POST /agents`.
#[allow(clippy::too_many_arguments)]
pub async fn run_create_agent(
    client: &AgentdClient,
    name: &str,
    working_dir: &str,
    model: Option<&str>,
    system_prompt: Option<&str>,
    append_system_prompt: bool,
    prompt: Option<&str>,
    tool_policy_mode: Option<&str>,
    tool_policy_tools: Option<Vec<String>>,
    rooms: Option<Vec<String>>,
) -> String {
    let body = match build_create_agent_body(
        name,
        working_dir,
        model,
        system_prompt,
        append_system_prompt,
        prompt,
        tool_policy_mode,
        tool_policy_tools.as_deref(),
        rooms.as_deref(),
    ) {
        Ok(b) => b,
        Err(msg) => return msg,
    };

    let url = format!("{}/agents", client.orchestrator_url());
    match client.post::<Value, Value>(&url, &body).await {
        Ok(agent) => {
            let id = agent["id"].as_str().unwrap_or("unknown");
            let status = agent["status"].as_str().unwrap_or("unknown");
            format!(
                "✅ Agent `{name}` created.\n\
                 - **ID:** `{id}`\n\
                 - **Status:** `{status}`\n\
                 → Use `diagnose_agent({id})` to monitor startup."
            )
        }
        Err(e) => format!(
            "🔴 Failed to create agent `{name}`: {e}\n\
             Common causes: duplicate name, working_dir does not exist, \
             orchestrator unreachable."
        ),
    }
}

// ── create_workflow ────────────────────────────────────────────────────────

/// Coerce a JSON-object parameter that may have arrived as a JSON *string*.
///
/// Some MCP clients — notably the Claude Code harness — serialize object- or
/// array-typed tool parameters as a JSON string (e.g. the literal text
/// `{"type":"cron"}`) rather than a nested JSON value. Because these params are
/// declared as untyped `serde_json::Value`, the schema gives the client no
/// signal to send a real object, so the string form is common.
///
/// If `value` is a string that parses as JSON, return the parsed value;
/// otherwise return it unchanged so the caller's validation surfaces a clear
/// error against the original input.
fn coerce_json_value(value: Value) -> Value {
    match value {
        Value::String(s) => serde_json::from_str(&s).unwrap_or(Value::String(s)),
        other => other,
    }
}

/// Validate trigger_config client-side: must be an object whose `type` is a
/// known variant name. Full structural validation stays server-side.
fn validate_trigger_config(trigger_config: &Value) -> Result<(), String> {
    let Some(obj) = trigger_config.as_object() else {
        return Err(format!(
            "🔴 `trigger_config` must be a JSON object like \
             {{\"type\":\"cron\",\"expression\":\"0 9 * * MON-FRI\"}}.\n\
             Valid types: {}",
            TRIGGER_TYPES.join(", ")
        ));
    };
    let Some(trigger_type) = obj.get("type").and_then(|t| t.as_str()) else {
        return Err(format!(
            "🔴 `trigger_config` is missing the `type` field.\n Valid types: {}",
            TRIGGER_TYPES.join(", ")
        ));
    };
    if !TRIGGER_TYPES.contains(&trigger_type) {
        return Err(format!(
            "🔴 Unknown trigger type `{trigger_type}`.\n Valid types: {}",
            TRIGGER_TYPES.join(", ")
        ));
    }
    Ok(())
}

/// Build the `POST /workflows` body.
#[allow(clippy::too_many_arguments)]
fn build_create_workflow_body(
    name: &str,
    agent_id: &str,
    trigger_config: &Value,
    prompt_template: &str,
    poll_interval_secs: Option<u64>,
    enabled: Option<bool>,
    tool_policy_mode: Option<&str>,
    tool_policy_tools: Option<&[String]>,
) -> Result<Value, String> {
    if name.trim().is_empty() {
        return Err("🔴 `name` must not be empty.".to_string());
    }
    if uuid_err(agent_id) {
        return Err(format!(
            "🔴 `agent_id` must be a UUID, got `{agent_id}`. \
             Use `list_agents` to find agent IDs."
        ));
    }
    validate_trigger_config(trigger_config)?;
    if prompt_template.trim().is_empty() {
        return Err("🔴 `prompt_template` must not be empty.".to_string());
    }

    let mut body = json!({
        "name": name,
        "agent_id": agent_id,
        "trigger_config": trigger_config,
        "prompt_template": prompt_template,
    });
    if let Some(secs) = poll_interval_secs {
        body["poll_interval_secs"] = json!(secs);
    }
    if let Some(enabled) = enabled {
        body["enabled"] = json!(enabled);
    }
    if let Some(mode) = tool_policy_mode {
        body["tool_policy"] = build_tool_policy(mode, tool_policy_tools)?;
    }
    Ok(body)
}

/// Loose UUID check (8-4-4-4-12 hex groups) without a uuid dependency.
fn uuid_err(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() != 5
        || [8usize, 4, 4, 4, 12]
            .iter()
            .zip(&groups)
            .any(|(len, g)| g.len() != *len || !g.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Create a new workflow via `POST /workflows`.
#[allow(clippy::too_many_arguments)]
pub async fn run_create_workflow(
    client: &AgentdClient,
    name: &str,
    agent_id: &str,
    trigger_config: Value,
    prompt_template: &str,
    poll_interval_secs: Option<u64>,
    enabled: Option<bool>,
    tool_policy_mode: Option<&str>,
    tool_policy_tools: Option<Vec<String>>,
) -> String {
    let trigger_config = coerce_json_value(trigger_config);
    let body = match build_create_workflow_body(
        name,
        agent_id,
        &trigger_config,
        prompt_template,
        poll_interval_secs,
        enabled,
        tool_policy_mode,
        tool_policy_tools.as_deref(),
    ) {
        Ok(b) => b,
        Err(msg) => return msg,
    };

    let url = format!("{}/workflows", client.orchestrator_url());
    match client.post::<Value, Value>(&url, &body).await {
        Ok(workflow) => {
            let id = workflow["id"].as_str().unwrap_or("unknown");
            let enabled = workflow["enabled"].as_bool().unwrap_or(true);
            format!(
                "✅ Workflow `{name}` created.\n\
                 - **ID:** `{id}`\n\
                 - **Enabled:** `{enabled}`\n\
                 → Smoke-test it with `trigger_workflow({id})`, then \
                 `list_dispatches({id})` to follow execution."
            )
        }
        // The orchestrator's 400 body includes prompt-template validation
        // messages (unknown placeholders, etc.) — surface it verbatim.
        Err(e) => format!("🔴 Failed to create workflow `{name}`: {e}"),
    }
}

// ── update_workflow ────────────────────────────────────────────────────────

/// Build the `PUT /workflows/{id}` body with only the provided fields.
#[allow(clippy::too_many_arguments)]
fn build_update_workflow_body(
    name: Option<&str>,
    agent_id: Option<&str>,
    trigger_config: Option<&Value>,
    prompt_template: Option<&str>,
    poll_interval_secs: Option<u64>,
    enabled: Option<bool>,
    tool_policy_mode: Option<&str>,
    tool_policy_tools: Option<&[String]>,
) -> Result<Value, String> {
    let mut body = json!({});
    if let Some(name) = name {
        body["name"] = json!(name);
    }
    if let Some(agent_id) = agent_id {
        if uuid_err(agent_id) {
            return Err(format!("🔴 `agent_id` must be a UUID, got `{agent_id}`."));
        }
        body["agent_id"] = json!(agent_id);
    }
    if let Some(tc) = trigger_config {
        validate_trigger_config(tc)?;
        body["trigger_config"] = tc.clone();
    }
    if let Some(template) = prompt_template {
        body["prompt_template"] = json!(template);
    }
    if let Some(secs) = poll_interval_secs {
        body["poll_interval_secs"] = json!(secs);
    }
    if let Some(enabled) = enabled {
        body["enabled"] = json!(enabled);
    }
    if let Some(mode) = tool_policy_mode {
        body["tool_policy"] = build_tool_policy(mode, tool_policy_tools)?;
    }
    if body.as_object().is_some_and(|o| o.is_empty()) {
        return Err("🔴 No fields to update — provide at least one parameter.".to_string());
    }
    Ok(body)
}

/// Update a workflow via `PUT /workflows/{id}`. Only provided fields change.
#[allow(clippy::too_many_arguments)]
pub async fn run_update_workflow(
    client: &AgentdClient,
    workflow_id: &str,
    name: Option<&str>,
    agent_id: Option<&str>,
    trigger_config: Option<Value>,
    prompt_template: Option<&str>,
    poll_interval_secs: Option<u64>,
    enabled: Option<bool>,
    tool_policy_mode: Option<&str>,
    tool_policy_tools: Option<Vec<String>>,
) -> String {
    let trigger_config = trigger_config.map(coerce_json_value);
    let body = match build_update_workflow_body(
        name,
        agent_id,
        trigger_config.as_ref(),
        prompt_template,
        poll_interval_secs,
        enabled,
        tool_policy_mode,
        tool_policy_tools.as_deref(),
    ) {
        Ok(b) => b,
        Err(msg) => return msg,
    };

    let url = format!("{}/workflows/{workflow_id}", client.orchestrator_url());
    match client.put::<Value, Value>(&url, &body).await {
        Ok(workflow) => {
            let name = workflow["name"].as_str().unwrap_or("unknown");
            format!("✅ Workflow `{name}` (`{workflow_id}`) updated.")
        }
        Err(e) => format!("🔴 Failed to update workflow `{workflow_id}`: {e}"),
    }
}

// ── set_workflow_enabled ───────────────────────────────────────────────────

/// Enable or disable a workflow (PUT /workflows/{id} with just `enabled`).
pub async fn run_set_workflow_enabled(
    client: &AgentdClient,
    workflow_id: &str,
    enabled: bool,
) -> String {
    let url = format!("{}/workflows/{workflow_id}", client.orchestrator_url());
    let body = json!({ "enabled": enabled });
    match client.put::<Value, Value>(&url, &body).await {
        Ok(workflow) => {
            let name = workflow["name"].as_str().unwrap_or("unknown");
            let verb = if enabled { "enabled" } else { "disabled" };
            format!("✅ Workflow `{name}` (`{workflow_id}`) {verb}.")
        }
        Err(e) => format!("🔴 Failed to update workflow `{workflow_id}`: {e}"),
    }
}

// ── trigger_workflow ───────────────────────────────────────────────────────

/// Manually trigger a workflow via `POST /workflows/{id}/trigger`.
pub async fn run_trigger_workflow(
    client: &AgentdClient,
    workflow_id: &str,
    title: Option<&str>,
    body_text: Option<&str>,
    url_field: Option<&str>,
    labels: Option<Vec<String>>,
    metadata: Option<Value>,
) -> String {
    let mut body = json!({});
    if let Some(title) = title {
        body["title"] = json!(title);
    }
    if let Some(text) = body_text {
        body["body"] = json!(text);
    }
    if let Some(u) = url_field {
        body["url"] = json!(u);
    }
    if let Some(labels) = labels {
        body["labels"] = json!(labels);
    }
    if let Some(metadata) = metadata {
        let metadata = coerce_json_value(metadata);
        if !metadata.is_object() {
            return "🔴 `metadata` must be a JSON object (string keys/values).".to_string();
        }
        body["metadata"] = metadata;
    }

    let url = format!("{}/workflows/{workflow_id}/trigger", client.orchestrator_url());
    match client.post::<Value, Value>(&url, &body).await {
        Ok(resp) => {
            let dispatch_id = resp["dispatch_id"].as_str().or(resp["id"].as_str()).unwrap_or("?");
            format!(
                "✅ Workflow `{workflow_id}` triggered (dispatch `{dispatch_id}`).\n\
                 → Use `list_dispatches({workflow_id})` to follow execution."
            )
        }
        Err(e) => format!("🔴 Failed to trigger workflow `{workflow_id}`: {e}"),
    }
}

// ── delete_workflow ────────────────────────────────────────────────────────

/// Delete a workflow via `DELETE /workflows/{id}`.
pub async fn run_delete_workflow(client: &AgentdClient, workflow_id: &str) -> String {
    let url = format!("{}/workflows/{workflow_id}", client.orchestrator_url());
    match client.inner.delete(&url).send().await.and_then(|r| r.error_for_status()) {
        Ok(_) => format!(
            "✅ Workflow `{workflow_id}` deleted. Its trigger no longer fires; \
             past dispatch history is retained."
        ),
        Err(e) => format!("🔴 Failed to delete workflow `{workflow_id}`: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_agent_body_minimal() {
        let body =
            build_create_agent_body("worker", "/tmp", None, None, false, None, None, None, None)
                .unwrap();
        assert_eq!(body["name"], "worker");
        assert_eq!(body["working_dir"], "/tmp");
        assert!(body.get("model").is_none());
        assert!(body.get("tool_policy").is_none());
        assert!(body.get("env").is_none(), "env must never be sent");
    }

    #[test]
    fn create_agent_body_full() {
        let tools = vec!["Read".to_string()];
        let rooms = vec!["system".to_string()];
        let body = build_create_agent_body(
            "worker",
            "/tmp",
            Some("sonnet"),
            Some("be careful"),
            true,
            Some("start by listing files"),
            Some("allow_list"),
            Some(&tools),
            Some(&rooms),
        )
        .unwrap();
        assert_eq!(body["model"], "sonnet");
        assert_eq!(body["system_prompt"], "be careful");
        assert_eq!(body["append_system_prompt"], true);
        assert_eq!(body["tool_policy"]["mode"], "allow_list");
        assert_eq!(body["rooms"][0], "system");
    }

    #[test]
    fn create_agent_body_rejects_empty_name() {
        assert!(build_create_agent_body("  ", "/tmp", None, None, false, None, None, None, None)
            .is_err());
    }

    #[test]
    fn create_workflow_body_rejects_bad_uuid() {
        let err = build_create_workflow_body(
            "wf",
            "not-a-uuid",
            &serde_json::json!({"type": "manual"}),
            "do the thing",
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("UUID"), "{err}");
    }

    #[test]
    fn create_workflow_body_rejects_unknown_trigger_type() {
        let err = build_create_workflow_body(
            "wf",
            "01234567-89ab-cdef-0123-456789abcdef",
            &serde_json::json!({"type": "carrier_pigeon"}),
            "do the thing",
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("carrier_pigeon"), "{err}");
        assert!(err.contains("cron"), "error lists valid types: {err}");
    }

    #[test]
    fn create_workflow_body_accepts_all_known_trigger_types() {
        for t in TRIGGER_TYPES {
            // Only `type` validity is checked client-side; structure is
            // validated by the orchestrator.
            assert!(
                validate_trigger_config(&serde_json::json!({"type": t})).is_ok(),
                "type {t} should pass the client-side check"
            );
        }
    }

    #[test]
    fn trigger_types_match_orchestrator_enum() {
        // Round-trip each variant tag against the real enum where possible:
        // `manual` and `cron` are structurally complete with minimal fields.
        use orchestrator::scheduler::types::TriggerConfig;
        let manual: TriggerConfig =
            serde_json::from_value(serde_json::json!({"type": "manual"})).unwrap();
        assert!(matches!(manual, TriggerConfig::Manual {}));
        let cron: TriggerConfig = serde_json::from_value(
            serde_json::json!({"type": "cron", "expression": "0 9 * * MON-FRI"}),
        )
        .unwrap();
        assert!(matches!(cron, TriggerConfig::Cron { .. }));
    }

    #[test]
    fn coerce_json_value_parses_stringified_object() {
        // The Claude Code harness passes object params as JSON strings.
        let coerced = coerce_json_value(serde_json::json!("{\"type\":\"cron\"}"));
        assert!(coerced.is_object(), "stringified JSON should be parsed to an object");
        assert_eq!(coerced["type"], "cron");
    }

    #[test]
    fn coerce_json_value_passes_through_real_object() {
        let original = serde_json::json!({"type": "manual"});
        assert_eq!(coerce_json_value(original.clone()), original);
    }

    #[test]
    fn coerce_json_value_keeps_unparseable_string() {
        // A non-JSON string is returned unchanged so validation can complain
        // about the actual input rather than a parse error.
        let coerced = coerce_json_value(serde_json::json!("not json"));
        assert_eq!(coerced, serde_json::json!("not json"));
    }

    #[test]
    fn create_workflow_accepts_stringified_trigger_config() {
        // End-to-end at the body-builder level: a stringified trigger_config
        // is coerced and validated like a real object.
        let coerced = coerce_json_value(serde_json::json!(
            "{\"type\":\"cron\",\"expression\":\"0 9 * * MON-FRI\"}"
        ));
        let body = build_create_workflow_body(
            "wf",
            "01234567-89ab-cdef-0123-456789abcdef",
            &coerced,
            "do the thing",
            None,
            None,
            None,
            None,
        )
        .expect("stringified trigger_config should be accepted after coercion");
        assert_eq!(body["trigger_config"]["type"], "cron");
    }

    #[test]
    fn update_workflow_body_requires_at_least_one_field() {
        let err =
            build_update_workflow_body(None, None, None, None, None, None, None, None).unwrap_err();
        assert!(err.contains("at least one"), "{err}");
    }

    #[test]
    fn update_workflow_body_only_serializes_provided_fields() {
        let body =
            build_update_workflow_body(None, None, None, None, None, Some(false), None, None)
                .unwrap();
        let obj = body.as_object().unwrap();
        assert_eq!(obj.len(), 1, "only `enabled` should be present: {body}");
        assert_eq!(body["enabled"], false);
    }
}
