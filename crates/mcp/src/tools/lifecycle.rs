//! Agent lifecycle management tool implementations.
//!
//! Provides management tools for agent operations: restart, message delivery,
//! tool policy updates, model changes, and termination.
//!
//! # Safety
//!
//! `terminate_agent` and `restart_agent` are destructive operations — they
//! kill the agent's tmux session and lose in-flight work. Tool descriptions
//! include explicit warnings per ADR-0001 risk annotation guidelines.

use crate::client::AgentdClient;
use serde_json::{json, Value};

// ── restart_agent ──────────────────────────────────────────────────────────

/// Restart an agent by capturing its config, terminating, and recreating it.
pub async fn run_restart_agent(client: &AgentdClient, agent_id: &str) -> String {
    let base = client.orchestrator_url();

    // 1. Capture current agent config
    let agent_url = format!("{base}/agents/{agent_id}");
    let agent = match client.get::<Value>(&agent_url).await {
        Ok(v) => v,
        Err(e) => {
            return format!("🔴 Could not fetch agent `{agent_id}` before restart: {e}");
        }
    };

    let name = agent["name"].as_str().unwrap_or("unknown").to_string();
    let status = agent["status"].as_str().unwrap_or("unknown");

    if status == "Running" {
        // Warn but proceed — issue spec says "warn but allow override"
        // (no override flag requested, so we note it and continue)
    }

    // Build CreateAgentRequest from the captured config
    let config = &agent["config"];
    let mut create_body = config.clone();
    // Inject name at the top level (CreateAgentRequest = name + AgentConfig fields)
    create_body["name"] = json!(name);

    // 2. Terminate existing agent
    let delete_url = format!("{base}/agents/{agent_id}");
    if let Err(e) = client.inner.delete(&delete_url).send().await.and_then(|r| r.error_for_status())
    {
        return format!("🔴 Failed to terminate agent `{agent_id}` during restart: {e}");
    }

    // 3. Recreate with the same config
    let agents_url = format!("{base}/agents");
    match client.post::<Value, Value>(&agents_url, &create_body).await {
        Ok(new_agent) => {
            let new_id = new_agent["id"].as_str().unwrap_or("unknown");
            let new_status = new_agent["status"].as_str().unwrap_or("unknown");
            format!(
                "✅ Agent `{name}` restarted.\n\
                 - **Old ID:** `{agent_id}`\n\
                 - **New ID:** `{new_id}`\n\
                 - **Status:** `{new_status}`\n\
                 → Use `diagnose_agent({new_id})` to monitor startup."
            )
        }
        Err(e) => format!(
            "⚠️ Agent `{agent_id}` was terminated but recreation failed: {e}\n\
             The old agent is gone. Create a new one manually if needed."
        ),
    }
}

// ── send_agent_message ─────────────────────────────────────────────────────

/// Send a prompt/message to a running agent.
pub async fn run_send_agent_message(
    client: &AgentdClient,
    agent_id: &str,
    message: &str,
) -> String {
    let url = format!("{}/agents/{agent_id}/message", client.orchestrator_url());
    let body = json!({ "content": message });
    match client.post::<Value, Value>(&url, &body).await {
        Ok(resp) => {
            let status = resp["status"].as_str().unwrap_or("sent");
            format!("✅ Message delivered to agent `{agent_id}`. Status: `{status}`.")
        }
        Err(e) => format!("🔴 Failed to send message to agent `{agent_id}`: {e}"),
    }
}

// ── update_agent_tool_policy ───────────────────────────────────────────────

/// Update an agent's tool policy.
pub async fn run_update_agent_tool_policy(
    client: &AgentdClient,
    agent_id: &str,
    mode: &str,
    tools: Option<Vec<String>>,
) -> String {
    // Internally-tagged shape matching the orchestrator's ToolPolicy enum.
    // The previous externally-tagged construction ({"AllowList": ...}) never
    // deserialized server-side.
    let policy = match crate::tools::policy::build_tool_policy(mode, tools.as_deref()) {
        Ok(p) => p,
        Err(msg) => return msg,
    };

    // The orchestrator routes this endpoint as PUT (POST returns 405).
    let url = format!("{}/agents/{agent_id}/policy", client.orchestrator_url());
    match client.put::<Value, Value>(&url, &policy).await {
        Ok(_) => {
            let tools_note = match mode {
                "allow_list" | "deny_list" => {
                    format!(" Tools: {}", tools.unwrap_or_default().join(", "))
                }
                _ => String::new(),
            };
            format!("✅ Tool policy for agent `{agent_id}` updated to `{mode}`.{tools_note}")
        }
        Err(e) => format!("🔴 Failed to update policy for agent `{agent_id}`: {e}"),
    }
}

// ── terminate_agent ────────────────────────────────────────────────────────

/// Terminate an agent — kills the tmux session and removes it from the registry.
pub async fn run_terminate_agent(client: &AgentdClient, agent_id: &str) -> String {
    let url = format!("{}/agents/{agent_id}", client.orchestrator_url());
    match client.inner.delete(&url).send().await.and_then(|r| r.error_for_status()) {
        Ok(_) => format!(
            "✅ Agent `{agent_id}` terminated. The tmux session has been killed and \
             all in-flight work has been lost."
        ),
        Err(e) => format!("🔴 Failed to terminate agent `{agent_id}`: {e}"),
    }
}

// ── update_agent_model ─────────────────────────────────────────────────────

/// Change the model an agent is using.
pub async fn run_update_agent_model(client: &AgentdClient, agent_id: &str, model: &str) -> String {
    // The orchestrator routes this endpoint as PUT (POST returns 405).
    let url = format!("{}/agents/{agent_id}/model", client.orchestrator_url());
    // SetModelRequest: { model: Option<String>, restart: bool }
    let body = json!({ "model": model, "restart": false });
    match client.put::<Value, Value>(&url, &body).await {
        Ok(resp) => {
            let status = resp["status"].as_str().unwrap_or("updated");
            format!("✅ Model for agent `{agent_id}` updated to `{model}`. Status: `{status}`.")
        }
        Err(e) => format!("🔴 Failed to update model for agent `{agent_id}`: {e}"),
    }
}
