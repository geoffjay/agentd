//! Prompt command implementation.
//!
//! Provides the `agent prompt` subcommand, which lets users send a natural-language
//! message to a named recipient using the `@recipient message` syntax familiar from
//! chat applications.
//!
//! # Syntax
//!
//! ```text
//! agent prompt "@worker-agent please summarise the last PR"
//! agent prompt "@engineering hello team"
//! agent prompt "no recipient — falls back to interactive picker"
//! ```
//!
//! # Recipient resolution
//!
//! 1. Parse the `@name` token from the front of the input string.
//! 2. Look up the name against known agents (via orchestrator) and rooms (via communicate).
//! 3. Route the message to the appropriate service.
//!
//! Routing integration tests are gated behind issue-734 (message routing
//! implementation) and are marked `#[ignore]` until that work lands.

use anyhow::Result;
use clap::Subcommand;

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
    /// interactive recipient picker (not yet implemented).
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent prompt send "@worker-agent summarise the last PR"
    /// agent prompt send "@engineering deploy is done"
    /// ```
    Send {
        /// Full prompt string, e.g. `"@worker-agent hello"`
        input: String,

        /// Output raw JSON instead of formatted text
        #[arg(long)]
        json: bool,
    },
}

impl PromptCommand {
    /// Dispatch to the appropriate handler.
    pub async fn execute(&self) -> Result<()> {
        match self {
            PromptCommand::Send { input, json } => send_prompt(input, *json).await,
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
// Routing (stub — full implementation pending issue-734)
// ---------------------------------------------------------------------------

/// Destination for a routed message.
#[derive(Debug, PartialEq, Eq)]
pub enum RouteTarget {
    /// Send via orchestrator agent API.
    Agent(String),
    /// Send via communicate room API.
    Room(String),
}

/// Resolve a recipient name to a [`RouteTarget`].
///
/// This is a stub that will be wired to live service calls in issue-734.
/// Once implemented it will query the orchestrator for the agent list and the
/// communicate service for the room list, then call [`match_recipient`] against
/// each to find the best match.
pub async fn resolve_route(recipient: &str) -> Option<RouteTarget> {
    // TODO(issue-734): replace stubs with live orchestrator + communicate lookups.
    let agents: Vec<&str> = vec![];
    let rooms: Vec<&str> = vec![];

    if let Some(name) = match_recipient(recipient, &agents) {
        return Some(RouteTarget::Agent(name.to_string()));
    }

    if let Some(name) = match_recipient(recipient, &rooms) {
        return Some(RouteTarget::Room(name.to_string()));
    }

    None
}

// ---------------------------------------------------------------------------
// Command handler
// ---------------------------------------------------------------------------

async fn send_prompt(input: &str, _json: bool) -> Result<()> {
    let parsed = parse_prompt(input);

    match &parsed.recipient {
        Some(name) => {
            // Route via live services once issue-734 lands.
            match resolve_route(name).await {
                Some(RouteTarget::Agent(agent)) => {
                    println!("routing to agent: {agent}");
                    println!("message:          {}", parsed.message);
                }
                Some(RouteTarget::Room(room)) => {
                    println!("routing to room: {room}");
                    println!("message:         {}", parsed.message);
                }
                None => {
                    // Recipient named but not yet resolvable — print stub output.
                    println!("recipient: {name} (unresolved — routing pending issue-734)");
                    println!("message:   {}", parsed.message);
                }
            }
        }
        None => {
            println!("No recipient specified. Interactive picker not yet implemented.");
            println!("message: {}", parsed.message);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
        // "@" with no name and no message
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
        // First token is not @-prefixed — all words go into the message.
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
        // Leading spaces on the rest are trimmed; interior spaces are kept.
        assert_eq!(p.message, "spaces   preserved in message");
    }

    #[test]
    fn parse_recipient_only_no_message() {
        // "@name" with nothing after it — recipient is set, message is empty.
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
        // Query uses a space; candidate uses a hyphen.
        assert_eq!(match_recipient("worker agent", &names), Some("worker-agent"));
    }

    #[test]
    fn match_space_to_hyphen() {
        // Candidate uses spaces; query uses a hyphen.
        let names = vec!["my agent"];
        assert_eq!(match_recipient("my-agent", &names), Some("my agent"));
    }

    #[test]
    fn match_exact_preferred() {
        // Both "Worker" (case-insensitive match) and "worker" (exact match)
        // are in the list — the exact match must be returned.
        let names = vec!["Worker", "worker"];
        assert_eq!(match_recipient("worker", &names), Some("worker"));
    }

    #[test]
    fn match_case_insensitive_preferred_over_normalised() {
        // "TRIAGE" is a case-insensitive match for "triage"; also a normalised
        // match.  The case-insensitive path (rule 2) must fire before
        // hyphen-normalisation (rule 3).
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
    // Routing integration tests (blocked by issue-734)
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "blocked by issue-734: message routing not yet implemented"]
    async fn send_to_agent_uses_orchestrator_api() {
        // TODO(issue-734): mock orchestrator, assert POST /agents/{name}/message
        todo!()
    }

    #[tokio::test]
    #[ignore = "blocked by issue-734: message routing not yet implemented"]
    async fn send_to_room_uses_communicate_api() {
        // TODO(issue-734): mock communicate service, assert POST /rooms/{name}/messages
        todo!()
    }

    #[tokio::test]
    #[ignore = "blocked by issue-734: message routing not yet implemented"]
    async fn agent_not_running_returns_error() {
        // TODO(issue-734): verify graceful error when agent is not running
        todo!()
    }

    #[tokio::test]
    #[ignore = "blocked by issue-734: message routing not yet implemented"]
    async fn unknown_recipient_triggers_picker() {
        // TODO(issue-734): verify fallback to interactive picker
        todo!()
    }
}
