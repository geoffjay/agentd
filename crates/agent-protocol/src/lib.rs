//! agentd Agent Protocol (AAP) — typed message definitions.
//!
//! AAP is the vendor-neutral wire protocol between the agentd orchestrator (the
//! **host**) and an AI coding agent, driven through an **adapter** process. The
//! full specification lives in `docs/spec/agent-protocol-v1.md`.
//!
//! Messages are exchanged as newline-delimited JSON (NDJSON): one JSON object
//! per line, discriminated by a top-level `type` field. This crate provides the
//! [`HostMessage`] and [`AgentMessage`] enums that model both directions of the
//! protocol, shared by the orchestrator and every in-tree adapter.
//!
//! Forward compatibility: deserialization ignores unknown fields, and callers
//! should treat an unknown message `type` (a serde error) as a line to log and
//! skip rather than a fatal condition.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The AAP protocol version implemented by this crate.
pub const PROTOCOL_VERSION: u32 = 1;

/// Capability tokens advertised by an adapter in [`AgentMessage::Ready`].
///
/// Capabilities are carried on the wire as free-form strings so that unknown
/// tokens are ignored rather than rejected; these constants name the tokens
/// this protocol version defines.
pub mod capability {
    /// Incremental `message` frames during a turn.
    pub const STREAMING: &str = "streaming";
    /// Emits `thinking` content blocks.
    pub const THINKING: &str = "thinking";
    /// Supports the `approval_request` / `approval_response` exchange.
    pub const TOOL_APPROVAL: &str = "tool_approval";
    /// Populates `usage` token counts on `turn_complete`.
    pub const USAGE_REPORTING: &str = "usage_reporting";
    /// Populates `usage.total_cost_usd`.
    pub const COST_REPORTING: &str = "cost_reporting";
    /// Handles `clear_context`.
    pub const CONTEXT_CLEAR: &str = "context_clear";
    /// Handles `cancel`.
    pub const CANCEL: &str = "cancel";
    /// Honors `tools.mcp_servers`.
    pub const MCP: &str = "mcp";
    /// Supports `system_prompt.mode = "append"`.
    pub const SYSTEM_PROMPT_APPEND: &str = "system_prompt_append";
    /// Emits and consumes `resume_token`.
    pub const RESUME: &str = "resume";
}

/// Environment variable naming the transport binding the adapter should use.
///
/// Value is either [`TRANSPORT_STDIO`] or [`TRANSPORT_WEBSOCKET`].
pub const ENV_TRANSPORT: &str = "AGENTD_AAP_TRANSPORT";
/// Environment variable carrying the WebSocket URL for the websocket binding.
pub const ENV_WS_URL: &str = "AGENTD_AAP_WS_URL";
/// Value of [`ENV_TRANSPORT`] selecting the stdio binding.
pub const TRANSPORT_STDIO: &str = "stdio";
/// Value of [`ENV_TRANSPORT`] selecting the websocket binding.
pub const TRANSPORT_WEBSOCKET: &str = "websocket";

/// Messages sent by the host (orchestrator) to the agent adapter.
///
/// Serialized with an internal `type` tag in `snake_case`, matching the wire
/// format in the specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMessage {
    /// First message on the connection; carries all agent configuration.
    Initialize(InitializeParams),
    /// A user prompt beginning a turn.
    Prompt { turn_id: String, content: PromptContent },
    /// Request interruption of a turn (capability `cancel`).
    Cancel {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
    },
    /// Discard conversation history and start fresh (capability `context_clear`).
    ClearContext,
    /// Request graceful termination.
    Shutdown,
    /// The host's decision on a pending tool-use approval.
    ApprovalResponse {
        request_id: String,
        decision: ApprovalDecision,
        /// Opaque replacement input applied by the adapter when present.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_input: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}

/// Parameters of the [`HostMessage::Initialize`] message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    /// AAP version the host speaks. Adapters refuse versions they cannot serve.
    pub protocol_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<SystemPrompt>,
    pub workspace: Workspace,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Tools>,
    /// Opaque token from a prior `turn_complete` to resume (capability `resume`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_token: Option<String>,
}

impl Default for InitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            model: None,
            system_prompt: None,
            workspace: Workspace::default(),
            tools: None,
            resume_token: None,
        }
    }
}

/// System prompt configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemPrompt {
    pub mode: SystemPromptMode,
    /// Inline prompt text. Exactly one of `text` or `path` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Path to a prompt file. Exactly one of `text` or `path` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Whether a system prompt replaces or appends to the agent's default.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemPromptMode {
    Replace,
    Append,
}

/// Workspace configuration for the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Workspace {
    /// Working directory.
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_dirs: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub worktree: bool,
}

/// Tool provisioning for the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Tools {
    /// MCP servers keyed by name (capability `mcp`).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mcp_servers: HashMap<String, McpServer>,
}

/// A single MCP server definition (stdio transport).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Content of a [`HostMessage::Prompt`]: either a plain string or content blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PromptContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl PromptContent {
    /// Flatten the prompt into a single plain-text string.
    pub fn as_text(&self) -> String {
        match self {
            PromptContent::Text(s) => s.clone(),
            PromptContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    ContentBlock::Thinking { .. } => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        }
    }
}

/// Messages sent by the agent adapter to the host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentMessage {
    /// Sent once the adapter is ready to accept prompts.
    Ready {
        protocol_version: u32,
        agent: AgentInfo,
        #[serde(default)]
        capabilities: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        models: Option<Vec<String>>,
    },
    /// Assistant output blocks for a turn.
    Message { turn_id: String, content: Vec<ContentBlock> },
    /// Announcement of a tool invocation.
    ToolCall {
        turn_id: String,
        call_id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    /// End of a turn, with optional usage accounting.
    TurnComplete {
        turn_id: String,
        #[serde(default)]
        is_error: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result_text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume_token: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage: Option<Usage>,
    },
    /// Activity transition.
    Status { state: ActivityState },
    /// Structured diagnostic log line.
    Log { level: LogLevel, message: String },
    /// A request for the host to approve a tool call (capability `tool_approval`).
    ApprovalRequest {
        request_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        tool_name: String,
        #[serde(default)]
        input: Value,
    },
    /// An error from the adapter. `fatal` means the adapter is exiting.
    Error {
        #[serde(default)]
        fatal: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        message: String,
    },
}

/// Identifies the agent behind an adapter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// A block of assistant content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    Thinking { text: String },
}

/// Agent activity state reported via [`AgentMessage::Status`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    Busy,
    Idle,
}

/// Severity of a [`AgentMessage::Log`] line.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// The host's decision on a tool-use approval.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Allow,
    Deny,
}

/// Token/cost/timing accounting for a completed turn.
///
/// Field shape matches the orchestrator's `UsageSnapshot` so the host can map
/// it across without translation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub total_cost_usd: f64,
    #[serde(default)]
    pub num_turns: u64,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub duration_api_ms: u64,
}

impl HostMessage {
    /// Serialize to a single NDJSON line (no trailing newline).
    pub fn to_ndjson(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }
}

impl AgentMessage {
    /// Serialize to a single NDJSON line (no trailing newline).
    pub fn to_ndjson(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn initialize_roundtrip() {
        let raw = json!({
            "type": "initialize",
            "protocol_version": 1,
            "model": "claude-sonnet-5",
            "system_prompt": { "mode": "append", "text": "be terse", "path": null },
            "workspace": { "cwd": "/repo", "additional_dirs": ["/other"], "worktree": true },
            "tools": { "mcp_servers": { "agentd": { "command": "agent", "args": ["mcp"], "env": {} } } },
            "resume_token": null
        });
        let msg: HostMessage = serde_json::from_value(raw).unwrap();
        match &msg {
            HostMessage::Initialize(p) => {
                assert_eq!(p.protocol_version, 1);
                assert_eq!(p.model.as_deref(), Some("claude-sonnet-5"));
                assert_eq!(p.workspace.cwd, "/repo");
                assert!(p.workspace.worktree);
                assert!(p.tools.as_ref().unwrap().mcp_servers.contains_key("agentd"));
            }
            _ => panic!("wrong variant"),
        }
        // Re-parse from serialized form to confirm the tag survives.
        let s = msg.to_ndjson().unwrap();
        let back: HostMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn prompt_content_string_or_blocks() {
        let s: HostMessage =
            serde_json::from_value(json!({"type":"prompt","turn_id":"t1","content":"hi"})).unwrap();
        match s {
            HostMessage::Prompt { content, .. } => assert_eq!(content.as_text(), "hi"),
            _ => panic!(),
        }
        let b: HostMessage = serde_json::from_value(json!({
            "type":"prompt","turn_id":"t1",
            "content":[{"type":"text","text":"a"},{"type":"thinking","text":"z"},{"type":"text","text":"b"}]
        }))
        .unwrap();
        match b {
            HostMessage::Prompt { content, .. } => assert_eq!(content.as_text(), "ab"),
            _ => panic!(),
        }
    }

    #[test]
    fn control_variants_tag_names() {
        assert_eq!(
            serde_json::to_value(HostMessage::ClearContext).unwrap(),
            json!({"type":"clear_context"})
        );
        assert_eq!(
            serde_json::to_value(HostMessage::Shutdown).unwrap(),
            json!({"type":"shutdown"})
        );
        let ar = HostMessage::ApprovalResponse {
            request_id: "r1".into(),
            decision: ApprovalDecision::Allow,
            updated_input: Some(json!({"command":"ls"})),
            message: None,
        };
        assert_eq!(
            serde_json::to_value(&ar).unwrap(),
            json!({"type":"approval_response","request_id":"r1","decision":"allow","updated_input":{"command":"ls"}})
        );
    }

    #[test]
    fn ready_and_turn_complete_roundtrip() {
        let ready: AgentMessage = serde_json::from_value(json!({
            "type":"ready","protocol_version":1,
            "agent":{"name":"claude-code","version":"2.1.x"},
            "capabilities":["streaming","tool_approval"],
            "models":["claude-sonnet-5"]
        }))
        .unwrap();
        match &ready {
            AgentMessage::Ready { capabilities, agent, .. } => {
                assert!(capabilities.iter().any(|c| c == capability::TOOL_APPROVAL));
                assert_eq!(agent.name, "claude-code");
            }
            _ => panic!(),
        }

        let tc = AgentMessage::TurnComplete {
            turn_id: "t1".into(),
            is_error: false,
            stop_reason: Some("end_turn".into()),
            result_text: Some("done".into()),
            resume_token: None,
            usage: Some(Usage {
                input_tokens: 10,
                output_tokens: 5,
                total_cost_usd: 0.01,
                ..Default::default()
            }),
        };
        let back: AgentMessage = serde_json::from_str(&tc.to_ndjson().unwrap()).unwrap();
        assert_eq!(tc, back);
    }

    #[test]
    fn tool_call_and_approval_request() {
        let tc: AgentMessage = serde_json::from_value(json!({
            "type":"tool_call","turn_id":"t1","call_id":"c1","name":"Bash","input":{"command":"ls"}
        }))
        .unwrap();
        assert!(matches!(tc, AgentMessage::ToolCall { .. }));

        let ar: AgentMessage = serde_json::from_value(json!({
            "type":"approval_request","request_id":"r1","call_id":"c1","tool_name":"Bash","input":{"command":"rm"}
        }))
        .unwrap();
        match ar {
            AgentMessage::ApprovalRequest { tool_name, .. } => assert_eq!(tool_name, "Bash"),
            _ => panic!(),
        }
    }

    #[test]
    fn unknown_fields_are_ignored() {
        // Forward compat: an extra field on a known message must not break parsing.
        let msg: AgentMessage = serde_json::from_value(json!({
            "type":"status","state":"busy","future_field":123
        }))
        .unwrap();
        assert_eq!(msg, AgentMessage::Status { state: ActivityState::Busy });
    }
}
