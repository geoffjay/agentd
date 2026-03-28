//! Prompt command implementation.
//!
//! Provides the `agent prompt` subcommand, which lets users send a natural-language
//! message to a named recipient using the `@recipient message` syntax familiar from
//! chat applications.
//!
//! # Syntax
//!
//! ```text
//! agent prompt send "@worker-agent please summarise the last PR"
//! agent prompt send "@engineering hello team"
//! agent prompt send "no recipient — falls back to interactive picker"
//! ```
//!
//! # Recipient resolution (issue-733)
//!
//! 1. Parse the `@name` token from the front of the input string.
//! 2. Fetch running agents from the orchestrator and all rooms from the
//!    communicate service.
//! 3. Match the name against agents first (priority), then rooms.
//! 4. If no match, fall back to the interactive picker.
//! 5. If no `@` is given, show the full picker immediately.
//!
//! # Message routing (issue-734)
//!
//! Once a target is resolved the message is forwarded to the appropriate API.
//! Routing is implemented in issue-734; until then the command prints a
//! confirmation stub.

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use colored::Colorize;
use communicate::client::CommunicateClient;
use communicate::types::{CreateMessageRequest, ParticipantKind};
use orchestrator::client::OrchestratorClient;
use orchestrator::types::SendMessageRequest;
use uuid::Uuid;

use crate::picker::{select_recipient, RecipientKind, RecipientOption};

// ---------------------------------------------------------------------------
// Clap command definition
// ---------------------------------------------------------------------------

/// Prompt subcommands.
#[derive(Debug, Subcommand)]
pub enum PromptCommand {
    /// Send a message using @ notation.
    ///
    /// The input string is expected to begin with `@recipient` followed by the
    /// message body.  If no `@` prefix is found the command falls back to an
    /// interactive recipient picker.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent prompt send "@worker-agent summarise the last PR"
    /// agent prompt send "@engineering deploy is done"
    /// agent prompt send "open picker for all agents and rooms"
    /// ```
    Send {
        /// Full prompt string, e.g. `"@worker-agent hello"`
        input: String,

        /// Sender identity for room messages (default: "user")
        #[arg(long, default_value = "user")]
        from: String,

        /// Output raw JSON instead of formatted text
        #[arg(long)]
        json: bool,
    },
}

impl PromptCommand {
    /// Dispatch to the appropriate handler.
    pub async fn execute(
        &self,
        orch: &OrchestratorClient,
        comm: &CommunicateClient,
        json: bool,
    ) -> Result<()> {
        match self {
            PromptCommand::Send { input, from, json: cmd_json } => {
                send_prompt(input, from, orch, comm, json || *cmd_json).await
            }
        }
    }
}

// ---------------------------------------------------------------------------
// @ parsing
// ---------------------------------------------------------------------------

/// Result of parsing an `@recipient message` string.
#[derive(Debug, PartialEq, Eq)]
pub struct ParsedPrompt {
    /// The recipient name extracted from the `@name` token, or `None` if the
    /// input did not start with `@`.
    pub recipient: Option<String>,
    /// The message body that follows the optional `@name` token.
    pub message: String,
}

/// Parse a prompt string that may begin with `@recipient`.
///
/// The first whitespace-delimited token is checked for a leading `@`.  If
/// present the token (minus the `@`) becomes the recipient and the remainder
/// of the string (trimmed) becomes the message.  If no `@` is found the
/// entire input is treated as the message with no recipient.
///
/// # Examples
///
/// ```
/// use cli::commands::prompt::parse_prompt;
///
/// let p = parse_prompt("@worker hello there");
/// assert_eq!(p.recipient.as_deref(), Some("worker"));
/// assert_eq!(p.message, "hello there");
///
/// let p = parse_prompt("no recipient here");
/// assert!(p.recipient.is_none());
/// assert_eq!(p.message, "no recipient here");
/// ```
pub fn parse_prompt(input: &str) -> ParsedPrompt {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return ParsedPrompt { recipient: None, message: String::new() };
    }

    // Split on the first whitespace to isolate the potential @token.
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    if let Some(name) = first.strip_prefix('@') {
        if name.is_empty() {
            // Input was "@" with no name — treat as no recipient.
            ParsedPrompt { recipient: None, message: rest.to_string() }
        } else {
            ParsedPrompt { recipient: Some(name.to_string()), message: rest.to_string() }
        }
    } else {
        // No @ prefix — whole string is the message.
        ParsedPrompt { recipient: None, message: trimmed.to_string() }
    }
}

// ---------------------------------------------------------------------------
// Name matching
// ---------------------------------------------------------------------------

/// Attempt to match a parsed recipient name against a list of known names.
///
/// Matching rules (applied in priority order):
///
/// 1. **Exact match** — case-sensitive equality.
/// 2. **Case-insensitive match** — ASCII-lowercase comparison.
/// 3. **Hyphen/space equivalence** — hyphens in the candidate are treated as
///    spaces (and vice-versa) before case-insensitive comparison.
///
/// Returns `Some(&str)` pointing at the matching entry in `candidates`, or
/// `None` if no match is found.
///
/// # Examples
///
/// ```
/// use cli::commands::prompt::match_recipient;
///
/// let names = vec!["worker-agent", "engineering", "MyAgent"];
///
/// assert_eq!(match_recipient("worker-agent", &names), Some("worker-agent"));
/// assert_eq!(match_recipient("WORKER-AGENT", &names), Some("worker-agent"));
/// assert_eq!(match_recipient("worker agent", &names), Some("worker-agent"));
/// assert_eq!(match_recipient("myagent",       &names), Some("MyAgent"));
/// assert_eq!(match_recipient("unknown",        &names), None);
/// ```
pub fn match_recipient<'a>(query: &str, candidates: &[&'a str]) -> Option<&'a str> {
    // 1. Exact match.
    if let Some(&c) = candidates.iter().find(|&&c| c == query) {
        return Some(c);
    }

    let query_lower = query.to_ascii_lowercase();

    // 2. Case-insensitive match (no normalisation).
    if let Some(&c) = candidates.iter().find(|&&c| c.to_ascii_lowercase() == query_lower) {
        return Some(c);
    }

    // 3. Hyphen ↔ space equivalence (case-insensitive).
    let query_norm = query_lower.replace('-', " ");
    candidates.iter().find(|&&c| c.to_ascii_lowercase().replace('-', " ") == query_norm).copied()
}

// ---------------------------------------------------------------------------
// Recipient resolution
// ---------------------------------------------------------------------------

/// Resolved routing target carrying the service UUID needed to send a message.
#[derive(Debug, PartialEq, Eq)]
pub enum RouteTarget {
    /// Send via `POST /agents/{id}/message` on the orchestrator.
    Agent {
        /// Orchestrator-assigned UUID.
        id: Uuid,
        /// Human-readable agent name (for display and logging).
        name: String,
    },
    /// Send via `POST /rooms/{id}/messages` on the communicate service.
    Room {
        /// Communicate-service UUID.
        id: Uuid,
        /// Human-readable room name (for display and logging).
        name: String,
    },
}

/// Fetch all running agents and available rooms and build a list of
/// [`RecipientOption`]s for use with the interactive picker.
///
/// Agents are listed before rooms so the picker default selection favours
/// agents.
pub async fn build_picker_options(
    orch: &OrchestratorClient,
    comm: &CommunicateClient,
) -> Result<Vec<RecipientOption>> {
    let mut options: Vec<RecipientOption> = Vec::new();

    // --- Running agents ---
    match orch.list_agents(Some("running")).await {
        Ok(page) => {
            for agent in page.items {
                options.push(RecipientOption::agent(
                    &agent.name,
                    agent.id,
                    agent.status,
                    agent.activity,
                ));
            }
        }
        Err(e) => {
            // Non-fatal: warn but continue so rooms can still be shown.
            eprintln!("warning: could not fetch agent list: {e}");
        }
    }

    // --- Rooms ---
    match comm.list_rooms(100, 0).await {
        Ok(page) => {
            for room in page.items {
                options.push(RecipientOption::room(
                    &room.name,
                    room.id,
                    &room.room_type.to_string(),
                ));
            }
        }
        Err(e) => {
            eprintln!("warning: could not fetch room list: {e}");
        }
    }

    Ok(options)
}

/// Resolve a recipient name to a [`RouteTarget`].
///
/// Resolution flow:
/// 1. Fetch running agents from the orchestrator.
/// 2. Check agents for a name match (agents take priority over rooms).
/// 3. Fetch rooms from the communicate service.
/// 4. Check rooms for a name match.
/// 5. Return `None` if no match found (caller should invoke the picker).
///
/// Matching uses [`match_recipient`]: exact → case-insensitive → hyphen/space
/// equivalence.
pub async fn resolve_route(
    recipient: &str,
    orch: &OrchestratorClient,
    comm: &CommunicateClient,
) -> Result<Option<RouteTarget>> {
    // --- 1. Check running agents (priority) ---
    let agents_page =
        orch.list_agents(Some("running")).await.context("Failed to list running agents")?;

    let agent_names: Vec<&str> = agents_page.items.iter().map(|a| a.name.as_str()).collect();

    if let Some(matched_name) = match_recipient(recipient, &agent_names) {
        // Find the full agent record to get the UUID.
        if let Some(agent) = agents_page.items.iter().find(|a| a.name == matched_name) {
            return Ok(Some(RouteTarget::Agent { id: agent.id, name: agent.name.clone() }));
        }
    }

    // --- 2. Check rooms ---
    let rooms_page = comm.list_rooms(100, 0).await.context("Failed to list rooms")?;

    let room_names: Vec<&str> = rooms_page.items.iter().map(|r| r.name.as_str()).collect();

    if let Some(matched_name) = match_recipient(recipient, &room_names) {
        if let Some(room) = rooms_page.items.iter().find(|r| r.name == matched_name) {
            return Ok(Some(RouteTarget::Room { id: room.id, name: room.name.clone() }));
        }
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// Command handler
// ---------------------------------------------------------------------------

async fn send_prompt(
    input: &str,
    from: &str,
    orch: &OrchestratorClient,
    comm: &CommunicateClient,
    json: bool,
) -> Result<()> {
    let parsed = parse_prompt(input);

    if parsed.message.is_empty() {
        bail!("No message provided. Usage: agent prompt send \"@recipient <message>\"");
    }

    // Resolve the target — either from @recipient or via the interactive picker.
    let target = match &parsed.recipient {
        Some(name) => match resolve_route(name, orch, comm).await? {
            Some(t) => t,
            None => {
                eprintln!("No agent or room named '{name}' found. Select from available:");
                let options = build_picker_options(orch, comm).await?;
                let chosen = select_recipient(&options, "Select recipient:")?;
                recipient_option_to_target(chosen)?
            }
        },
        None => {
            let options = build_picker_options(orch, comm).await?;
            if options.is_empty() {
                bail!("No running agents or rooms available");
            }
            let chosen = select_recipient(&options, "Select recipient:")?;
            recipient_option_to_target(chosen)?
        }
    };

    // Route the message to the resolved target.
    route_message(&target, &parsed.message, from, orch, comm, json).await
}

/// Send the message to the resolved [`RouteTarget`] and print a confirmation.
async fn route_message(
    target: &RouteTarget,
    message: &str,
    from: &str,
    orch: &OrchestratorClient,
    comm: &CommunicateClient,
    json: bool,
) -> Result<()> {
    match target {
        RouteTarget::Agent { id, name } => {
            let request = SendMessageRequest { content: message.to_string() };
            let response = orch
                .send_message(id, &request)
                .await
                .with_context(|| format!("Failed to send message to agent '{name}'"))?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "sent",
                        "target": name,
                        "target_type": "agent",
                        "agent_id": id,
                        "response_status": response.status,
                    })
                );
            } else {
                println!("{} Prompt sent to agent '{}'", "✓".green().bold(), name.bold());
                println!("  status: {}", response.status.cyan());
            }
        }

        RouteTarget::Room { id, name } => {
            let request = CreateMessageRequest {
                sender_id: from.to_string(),
                sender_name: from.to_string(),
                sender_kind: ParticipantKind::Human,
                content: message.to_string(),
                metadata: std::collections::HashMap::new(),
                reply_to: None,
            };
            let response = comm
                .send_message(*id, &request)
                .await
                .with_context(|| format!("Failed to send message to room '{name}'"))?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "sent",
                        "target": name,
                        "target_type": "room",
                        "room_id": id,
                        "message_id": response.id,
                    })
                );
            } else {
                println!("{} Message sent to room '{}'", "✓".green().bold(), name.bold());
                println!("  message id: {}", response.id.to_string().cyan());
            }
        }
    }

    Ok(())
}

/// Convert a chosen [`RecipientOption`] from the picker into a [`RouteTarget`].
fn recipient_option_to_target(opt: &RecipientOption) -> Result<RouteTarget> {
    match &opt.kind {
        RecipientKind::Agent { id, .. } => {
            Ok(RouteTarget::Agent { id: *id, name: opt.name.clone() })
        }
        RecipientKind::Room { id, .. } => Ok(RouteTarget::Room { id: *id, name: opt.name.clone() }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    // -----------------------------------------------------------------------
    // @ parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_at_recipient_and_message() {
        let p = parse_prompt("@worker hello there");
        assert_eq!(p.recipient.as_deref(), Some("worker"));
        assert_eq!(p.message, "hello there");
    }

    #[test]
    fn parse_no_recipient() {
        let p = parse_prompt("just a plain message");
        assert!(p.recipient.is_none());
        assert_eq!(p.message, "just a plain message");
    }

    #[test]
    fn parse_at_only_no_message() {
        let p = parse_prompt("@");
        assert!(p.recipient.is_none());
        assert_eq!(p.message, "");
    }

    #[test]
    fn parse_hyphenated_recipient() {
        let p = parse_prompt("@worker-agent please do the thing");
        assert_eq!(p.recipient.as_deref(), Some("worker-agent"));
        assert_eq!(p.message, "please do the thing");
    }

    #[test]
    fn parse_multiple_words_no_quotes() {
        let p = parse_prompt("send this to nobody in particular");
        assert!(p.recipient.is_none());
        assert_eq!(p.message, "send this to nobody in particular");
    }

    #[test]
    fn parse_empty_input() {
        let p = parse_prompt("");
        assert!(p.recipient.is_none());
        assert_eq!(p.message, "");
    }

    #[test]
    fn parse_at_with_spaces_in_message() {
        let p = parse_prompt("@bot   spaces   preserved in message");
        assert_eq!(p.recipient.as_deref(), Some("bot"));
        assert_eq!(p.message, "spaces   preserved in message");
    }

    #[test]
    fn parse_recipient_only_no_message() {
        let p = parse_prompt("@triage");
        assert_eq!(p.recipient.as_deref(), Some("triage"));
        assert_eq!(p.message, "");
    }

    #[test]
    fn parse_whitespace_only_input() {
        let p = parse_prompt("   ");
        assert!(p.recipient.is_none());
        assert_eq!(p.message, "");
    }

    // -----------------------------------------------------------------------
    // Name matching
    // -----------------------------------------------------------------------

    #[test]
    fn match_case_insensitive() {
        let names = vec!["worker-agent", "engineering"];
        assert_eq!(match_recipient("WORKER-AGENT", &names), Some("worker-agent"));
        assert_eq!(match_recipient("Engineering", &names), Some("engineering"));
    }

    #[test]
    fn match_hyphen_to_space() {
        let names = vec!["worker-agent"];
        assert_eq!(match_recipient("worker agent", &names), Some("worker-agent"));
    }

    #[test]
    fn match_space_to_hyphen() {
        let names = vec!["my agent"];
        assert_eq!(match_recipient("my-agent", &names), Some("my agent"));
    }

    #[test]
    fn match_exact_preferred() {
        let names = vec!["Worker", "worker"];
        assert_eq!(match_recipient("worker", &names), Some("worker"));
    }

    #[test]
    fn match_case_insensitive_preferred_over_normalised() {
        let names = vec!["triage", "triage-bot"];
        assert_eq!(match_recipient("TRIAGE", &names), Some("triage"));
    }

    #[test]
    fn no_match_returns_none() {
        let names = vec!["worker-agent", "engineering"];
        assert_eq!(match_recipient("unknown-bot", &names), None);
    }

    #[test]
    fn match_empty_candidates() {
        let names: Vec<&str> = vec![];
        assert_eq!(match_recipient("worker", &names), None);
    }

    // -----------------------------------------------------------------------
    // Recipient resolution — unit tests with mock servers
    // -----------------------------------------------------------------------

    fn agent_json(id: &str, name: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "status": "running",
            "activity": "idle",
            "config": {
                "working_dir": "/tmp",
                "shell": "bash"
            },
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
        })
    }

    fn room_json(id: &str, name: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "topic": null,
            "description": null,
            "room_type": "group",
            "created_by": "test",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
        })
    }

    fn paginated<T: serde::Serialize>(items: Vec<T>) -> serde_json::Value {
        let len = items.len() as u64;
        json!({ "items": items, "total": len, "limit": 100, "offset": 0 })
    }

    #[tokio::test]
    async fn resolve_route_matches_running_agent_by_name() {
        let agent_id = "550e8400-e29b-41d4-a716-446655440001";
        let mut orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _orch_mock = orch_server
            .mock("GET", "/agents?status=running")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(vec![agent_json(agent_id, "planner")]).to_string())
            .create_async()
            .await;

        // communicate mock not reached when agent matches first
        let _comm_mock = comm_server
            .mock("GET", mockito::Matcher::Regex(r"/rooms".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(Vec::<serde_json::Value>::new()).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let result = resolve_route("planner", &orch, &comm).await.unwrap();

        assert!(matches!(
            result,
            Some(RouteTarget::Agent { ref name, .. }) if name == "planner"
        ));
        if let Some(RouteTarget::Agent { id, .. }) = result {
            assert_eq!(id.to_string(), agent_id);
        }
    }

    #[tokio::test]
    async fn resolve_route_matches_agent_case_insensitively() {
        let agent_id = "550e8400-e29b-41d4-a716-446655440002";
        let mut orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _m = orch_server
            .mock("GET", "/agents?status=running")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(vec![agent_json(agent_id, "planner")]).to_string())
            .create_async()
            .await;

        let _mc = comm_server
            .mock("GET", mockito::Matcher::Regex(r"/rooms".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(Vec::<serde_json::Value>::new()).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let result = resolve_route("PLANNER", &orch, &comm).await.unwrap();
        assert!(matches!(result, Some(RouteTarget::Agent { .. })));
    }

    #[tokio::test]
    async fn resolve_route_matches_agent_with_hyphen_query() {
        let agent_id = "550e8400-e29b-41d4-a716-446655440003";
        let mut orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _m = orch_server
            .mock("GET", "/agents?status=running")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(vec![agent_json(agent_id, "worker-agent")]).to_string())
            .create_async()
            .await;

        let _mc = comm_server
            .mock("GET", mockito::Matcher::Regex(r"/rooms".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(Vec::<serde_json::Value>::new()).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        // "worker agent" (space) should match "worker-agent" (hyphen)
        let result = resolve_route("worker agent", &orch, &comm).await.unwrap();
        assert!(matches!(result, Some(RouteTarget::Agent { .. })));
    }

    #[tokio::test]
    async fn resolve_route_matches_room_when_no_agent_match() {
        let room_id = "660e8400-e29b-41d4-a716-446655440001";
        let mut orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _m = orch_server
            .mock("GET", "/agents?status=running")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(Vec::<serde_json::Value>::new()).to_string())
            .create_async()
            .await;

        let _mc = comm_server
            .mock("GET", mockito::Matcher::Regex(r"/rooms".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(vec![room_json(room_id, "engineering")]).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let result = resolve_route("engineering", &orch, &comm).await.unwrap();
        assert!(matches!(
            result,
            Some(RouteTarget::Room { ref name, .. }) if name == "engineering"
        ));
        if let Some(RouteTarget::Room { id, .. }) = result {
            assert_eq!(id.to_string(), room_id);
        }
    }

    #[tokio::test]
    async fn resolve_route_agents_take_priority_over_rooms() {
        // Same name exists as both an agent and a room — agent wins.
        let agent_id = "550e8400-e29b-41d4-a716-446655440010";
        let room_id = "660e8400-e29b-41d4-a716-446655440010";
        let mut orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _m = orch_server
            .mock("GET", "/agents?status=running")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(vec![agent_json(agent_id, "shared-name")]).to_string())
            .create_async()
            .await;

        let _mc = comm_server
            .mock("GET", mockito::Matcher::Regex(r"/rooms".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(vec![room_json(room_id, "shared-name")]).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let result = resolve_route("shared-name", &orch, &comm).await.unwrap();
        // Must be the agent, not the room.
        assert!(matches!(result, Some(RouteTarget::Agent { .. })));
    }

    #[tokio::test]
    async fn resolve_route_returns_none_for_unknown_recipient() {
        let mut orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _m = orch_server
            .mock("GET", "/agents?status=running")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(Vec::<serde_json::Value>::new()).to_string())
            .create_async()
            .await;

        let _mc = comm_server
            .mock("GET", mockito::Matcher::Regex(r"/rooms".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(paginated(Vec::<serde_json::Value>::new()).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let result = resolve_route("nonexistent-bot", &orch, &comm).await.unwrap();
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // route_message — routing integration tests
    // -----------------------------------------------------------------------

    fn message_response_json(room_id: &str, msg_id: &str) -> serde_json::Value {
        json!({
            "id": msg_id,
            "room_id": room_id,
            "sender_id": "user",
            "sender_name": "user",
            "sender_kind": "human",
            "content": "hello",
            "metadata": {},
            "reply_to": null,
            "status": "delivered",
            "created_at": "2024-01-01T00:00:00Z",
        })
    }

    fn send_message_response_json(agent_id: &str) -> serde_json::Value {
        json!({
            "status": "sent",
            "agent_id": agent_id,
        })
    }

    #[tokio::test]
    async fn send_to_agent_uses_orchestrator_api() {
        let agent_id = "550e8400-e29b-41d4-a716-446655440020";
        let mut orch_server = Server::new_async().await;
        let comm_server = Server::new_async().await;

        let _mock = orch_server
            .mock("POST", format!("/agents/{agent_id}/message").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(send_message_response_json(agent_id).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let target = RouteTarget::Agent {
            id: Uuid::parse_str(agent_id).unwrap(),
            name: "planner".to_string(),
        };

        let result =
            route_message(&target, "summarise the last PR", "user", &orch, &comm, false).await;

        assert!(result.is_ok(), "expected Ok, got: {result:?}");
        _mock.assert_async().await;
    }

    #[tokio::test]
    async fn send_to_agent_json_output() {
        let agent_id = "550e8400-e29b-41d4-a716-446655440021";
        let mut orch_server = Server::new_async().await;
        let comm_server = Server::new_async().await;

        let _mock = orch_server
            .mock("POST", format!("/agents/{agent_id}/message").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(send_message_response_json(agent_id).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let target = RouteTarget::Agent {
            id: Uuid::parse_str(agent_id).unwrap(),
            name: "planner".to_string(),
        };

        // json=true path must not panic
        let result = route_message(&target, "hello", "user", &orch, &comm, true).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn send_to_room_uses_communicate_api() {
        let room_id = "660e8400-e29b-41d4-a716-446655440020";
        let msg_id = "770e8400-e29b-41d4-a716-446655440020";
        let orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _mock = comm_server
            .mock("POST", format!("/rooms/{room_id}/messages").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(message_response_json(room_id, msg_id).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let target = RouteTarget::Room {
            id: Uuid::parse_str(room_id).unwrap(),
            name: "engineering".to_string(),
        };

        let result = route_message(&target, "deploy is done", "user", &orch, &comm, false).await;

        assert!(result.is_ok(), "expected Ok, got: {result:?}");
        _mock.assert_async().await;
    }

    #[tokio::test]
    async fn send_to_room_json_output() {
        let room_id = "660e8400-e29b-41d4-a716-446655440021";
        let msg_id = "770e8400-e29b-41d4-a716-446655440021";
        let orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _mock = comm_server
            .mock("POST", format!("/rooms/{room_id}/messages").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(message_response_json(room_id, msg_id).to_string())
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let target = RouteTarget::Room {
            id: Uuid::parse_str(room_id).unwrap(),
            name: "engineering".to_string(),
        };

        let result = route_message(&target, "hello", "bot-sender", &orch, &comm, true).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn agent_not_running_returns_error() {
        let agent_id = "550e8400-e29b-41d4-a716-446655440030";
        let mut orch_server = Server::new_async().await;
        let comm_server = Server::new_async().await;

        // Simulate orchestrator returning 404 (agent stopped between resolution
        // and routing — a race condition the router must handle gracefully).
        let _mock = orch_server
            .mock("POST", format!("/agents/{agent_id}/message").as_str())
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":"agent not found"}"#)
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let target = RouteTarget::Agent {
            id: Uuid::parse_str(agent_id).unwrap(),
            name: "planner".to_string(),
        };

        let result = route_message(&target, "hello", "user", &orch, &comm, false).await;

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("planner"), "error should mention agent name: {msg}");
    }

    #[tokio::test]
    async fn room_not_found_returns_error() {
        let room_id = "660e8400-e29b-41d4-a716-446655440030";
        let orch_server = Server::new_async().await;
        let mut comm_server = Server::new_async().await;

        let _mock = comm_server
            .mock("POST", format!("/rooms/{room_id}/messages").as_str())
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":"room not found"}"#)
            .create_async()
            .await;

        let orch = OrchestratorClient::new(orch_server.url());
        let comm = CommunicateClient::new(&comm_server.url());

        let target = RouteTarget::Room {
            id: Uuid::parse_str(room_id).unwrap(),
            name: "engineering".to_string(),
        };

        let result = route_message(&target, "hello", "user", &orch, &comm, false).await;

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("engineering"), "error should mention room name: {msg}");
    }
}
