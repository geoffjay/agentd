//! Translation between Claude Code's `stream-json` protocol and AAP.
//!
//! Ported from the orchestrator's former duck-typed `handle_incoming_message`
//! / `extract_assistant_content` / `extract_usage` and the `make_*_response`
//! builders. The adapter is now the only place that knows Claude's shapes.

use agentd_agent_protocol::{ActivityState, AgentMessage, ApprovalDecision, ContentBlock, Usage};
use serde_json::{json, Value};

/// Outcome of translating one claude stdout line.
#[derive(Default)]
pub struct Translated {
    /// AAP messages to forward to the host.
    pub messages: Vec<AgentMessage>,
    /// True when this line was a `result` (the turn ended).
    pub turn_ended: bool,
}

/// Translate a single claude `stream-json` line into AAP agent messages.
///
/// `turn_id` is the host-assigned id for the active turn; claude output is
/// stamped with it.
pub fn claude_line(line: &str, turn_id: &str) -> Translated {
    let mut out = Translated::default();

    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return out, // Non-JSON noise on stdout: ignore.
    };

    match msg.get("type").and_then(|v| v.as_str()).unwrap_or("") {
        "assistant" => {
            if let Some(message) = msg.get("message") {
                let (blocks, tool_calls) = extract_assistant(message, turn_id);
                if !blocks.is_empty() {
                    out.messages.push(AgentMessage::Message {
                        turn_id: turn_id.to_string(),
                        content: blocks,
                    });
                }
                out.messages.extend(tool_calls);
            }
        }
        "result" => {
            out.turn_ended = true;
            let is_error = msg.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
            let result_text = msg.get("result").and_then(|v| v.as_str()).map(|s| s.to_string());
            out.messages.push(AgentMessage::TurnComplete {
                turn_id: turn_id.to_string(),
                is_error,
                stop_reason: msg.get("subtype").and_then(|v| v.as_str()).map(|s| s.to_string()),
                result_text,
                resume_token: None,
                usage: Some(extract_usage(&msg)),
            });
            out.messages.push(AgentMessage::Status { state: ActivityState::Idle });
        }
        "control_request" => {
            if let Some(req) = msg.get("request") {
                if req.get("subtype").and_then(|v| v.as_str()) == Some("can_use_tool") {
                    let request_id =
                        msg.get("request_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let tool_name =
                        req.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let input = req.get("input").cloned().unwrap_or(Value::Null);
                    out.messages.push(AgentMessage::ApprovalRequest {
                        request_id,
                        call_id: None,
                        tool_name,
                        input,
                    });
                }
            }
        }
        // system / keep_alive / unknown: nothing to forward.
        _ => {}
    }

    out
}

/// Extract text/thinking blocks and tool calls from a claude `assistant`
/// message payload.
fn extract_assistant(message: &Value, turn_id: &str) -> (Vec<ContentBlock>, Vec<AgentMessage>) {
    let mut blocks = Vec::new();
    let mut tool_calls = Vec::new();

    match message.get("content") {
        Some(Value::String(s)) => {
            if !s.is_empty() {
                blocks.push(ContentBlock::Text { text: s.clone() });
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                match item.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                    "text" => {
                        if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                            blocks.push(ContentBlock::Text { text: t.to_string() });
                        }
                    }
                    "thinking" => {
                        // Claude thinking blocks carry the text under "thinking";
                        // tolerate "text" as a fallback.
                        let t = item
                            .get("thinking")
                            .and_then(|v| v.as_str())
                            .or_else(|| item.get("text").and_then(|v| v.as_str()));
                        if let Some(t) = t {
                            blocks.push(ContentBlock::Thinking { text: t.to_string() });
                        }
                    }
                    "tool_use" => {
                        tool_calls.push(AgentMessage::ToolCall {
                            turn_id: turn_id.to_string(),
                            call_id: item
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            name: item
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            input: item.get("input").cloned().unwrap_or(Value::Null),
                        });
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    (blocks, tool_calls)
}

/// Extract usage/cost/timing from a claude `result` message. Token counts come
/// from the nested `usage` object; cost/turns/timing from the root with a
/// fallback to `usage`.
fn extract_usage(msg: &Value) -> Usage {
    let usage = msg.get("usage");
    let u = |k: &str| -> u64 { usage.and_then(|o| o.get(k)).and_then(|v| v.as_u64()).unwrap_or(0) };
    let root_or_usage_u64 = |k: &str| -> u64 {
        msg.get(k)
            .and_then(|v| v.as_u64())
            .or_else(|| usage.and_then(|o| o.get(k)).and_then(|v| v.as_u64()))
            .unwrap_or(0)
    };
    let root_or_usage_f64 = |k: &str| -> f64 {
        msg.get(k)
            .and_then(|v| v.as_f64())
            .or_else(|| usage.and_then(|o| o.get(k)).and_then(|v| v.as_f64()))
            .unwrap_or(0.0)
    };

    Usage {
        input_tokens: u("input_tokens"),
        output_tokens: u("output_tokens"),
        cache_read_input_tokens: u("cache_read_input_tokens"),
        cache_creation_input_tokens: u("cache_creation_input_tokens"),
        total_cost_usd: root_or_usage_f64("total_cost_usd"),
        num_turns: root_or_usage_u64("num_turns"),
        duration_ms: root_or_usage_u64("duration_ms"),
        duration_api_ms: root_or_usage_u64("duration_api_ms"),
    }
}

/// Build the claude `user` message line for an AAP prompt.
pub fn prompt_to_claude(content: &str) -> String {
    json!({ "type": "user", "message": { "role": "user", "content": content } }).to_string()
}

/// Build the claude `control_response` line for an AAP approval decision.
///
/// On allow, claude requires an `updatedInput`; `input` is the effective input
/// (the host's `updated_input` if provided, otherwise the original tool input
/// the adapter cached when it forwarded the request).
pub fn approval_to_claude(
    request_id: &str,
    decision: ApprovalDecision,
    input: &Value,
    message: Option<&str>,
) -> String {
    let response = match decision {
        ApprovalDecision::Allow => json!({
            "behavior": "allow",
            "updatedInput": input,
        }),
        ApprovalDecision::Deny => json!({
            "behavior": "deny",
            "message": message.unwrap_or("denied by policy"),
        }),
    };
    json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": response,
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_text_and_tool_use() {
        let line = json!({
            "type": "assistant",
            "message": { "content": [
                {"type":"text","text":"hello"},
                {"type":"thinking","thinking":"hmm"},
                {"type":"tool_use","id":"tu1","name":"Bash","input":{"command":"ls"}}
            ]}
        })
        .to_string();
        let t = claude_line(&line, "turn1");
        assert!(!t.turn_ended);
        // One Message (text+thinking) + one ToolCall.
        assert_eq!(t.messages.len(), 2);
        match &t.messages[0] {
            AgentMessage::Message { content, turn_id } => {
                assert_eq!(turn_id, "turn1");
                assert_eq!(content.len(), 2);
            }
            _ => panic!("expected message"),
        }
        match &t.messages[1] {
            AgentMessage::ToolCall { name, call_id, .. } => {
                assert_eq!(name, "Bash");
                assert_eq!(call_id, "tu1");
            }
            _ => panic!("expected tool_call"),
        }
    }

    #[test]
    fn result_produces_turn_complete_and_idle() {
        let line = json!({
            "type":"result","is_error":false,"result":"done",
            "usage":{"input_tokens":100,"output_tokens":20,"cache_read_input_tokens":50,"cache_creation_input_tokens":0},
            "total_cost_usd":0.02,"num_turns":2,"duration_ms":1000,"duration_api_ms":900
        })
        .to_string();
        let t = claude_line(&line, "turn1");
        assert!(t.turn_ended);
        assert_eq!(t.messages.len(), 2);
        match &t.messages[0] {
            AgentMessage::TurnComplete { usage, is_error, result_text, turn_id, .. } => {
                assert!(!is_error);
                assert_eq!(turn_id, "turn1");
                assert_eq!(result_text.as_deref(), Some("done"));
                let usage = usage.as_ref().unwrap();
                assert_eq!(usage.input_tokens, 100);
                assert_eq!(usage.output_tokens, 20);
                assert_eq!(usage.cache_read_input_tokens, 50);
                assert!((usage.total_cost_usd - 0.02).abs() < 1e-9);
                assert_eq!(usage.num_turns, 2);
            }
            _ => panic!("expected turn_complete"),
        }
        assert!(matches!(t.messages[1], AgentMessage::Status { state: ActivityState::Idle }));
    }

    #[test]
    fn can_use_tool_becomes_approval_request() {
        let line = json!({
            "type":"control_request","request_id":"req9",
            "request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"rm"}}
        })
        .to_string();
        let t = claude_line(&line, "turn1");
        match &t.messages[0] {
            AgentMessage::ApprovalRequest { request_id, tool_name, input, .. } => {
                assert_eq!(request_id, "req9");
                assert_eq!(tool_name, "Bash");
                assert_eq!(input["command"], "rm");
            }
            _ => panic!("expected approval_request"),
        }
    }

    #[test]
    fn approval_response_frames() {
        let allow =
            approval_to_claude("req9", ApprovalDecision::Allow, &json!({"command":"ls"}), None);
        let v: Value = serde_json::from_str(&allow).unwrap();
        assert_eq!(v["response"]["request_id"], "req9");
        assert_eq!(v["response"]["response"]["behavior"], "allow");
        assert_eq!(v["response"]["response"]["updatedInput"]["command"], "ls");

        let deny = approval_to_claude("req9", ApprovalDecision::Deny, &Value::Null, Some("nope"));
        let v: Value = serde_json::from_str(&deny).unwrap();
        assert_eq!(v["response"]["response"]["behavior"], "deny");
        assert_eq!(v["response"]["response"]["message"], "nope");
    }
}
