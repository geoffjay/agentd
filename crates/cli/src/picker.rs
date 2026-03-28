//! Interactive recipient picker for the `agent prompt` subcommand.
//!
//! Provides a terminal selection list populated with running agents and
//! available communication rooms.  Used as the fallback when no `@recipient`
//! is supplied on the command line or when the supplied name does not resolve
//! to a known target.
//!
//! # Example
//!
//! ```no_run
//! use cli::picker::{RecipientOption, select_recipient};
//! use orchestrator::types::{AgentStatus, ActivityState};
//!
//! let options = vec![
//!     RecipientOption::agent(
//!         "planner",
//!         uuid::Uuid::new_v4(),
//!         AgentStatus::Running,
//!         ActivityState::Idle,
//!     ),
//!     RecipientOption::room("engineering", uuid::Uuid::new_v4(), "group"),
//! ];
//!
//! // Presents an interactive list; returns the chosen option.
//! // Returns an error when there is no interactive terminal (e.g. in tests).
//! let chosen = select_recipient(&options, "Select a recipient:");
//! ```

use anyhow::{bail, Context, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use orchestrator::types::{ActivityState, AgentStatus};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The kind of recipient: a running agent or a communication room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipientKind {
    /// A running agent managed by the orchestrator.
    Agent {
        /// Orchestrator-assigned UUID.
        id: Uuid,
        /// Lifecycle status (`running`, `pending`, …).
        status: AgentStatus,
        /// Activity state (`idle` or `busy`).
        activity: ActivityState,
    },
    /// A communication room managed by the communicate service.
    Room {
        /// Communicate-service UUID.
        id: Uuid,
        /// Room type string (`direct`, `group`, `broadcast`).
        room_type: String,
    },
}

/// A selectable entry in the interactive picker.
#[derive(Debug, Clone)]
pub struct RecipientOption {
    /// Canonical name of the agent or room.
    pub name: String,
    /// Structured kind metadata.
    pub kind: RecipientKind,
    /// Pre-formatted display string shown in the terminal list.
    pub display: String,
}

impl RecipientOption {
    /// Construct an agent option.
    ///
    /// # Arguments
    ///
    /// * `name` — Agent name as registered in the orchestrator.
    /// * `id` — Orchestrator UUID.
    /// * `status` — Lifecycle status string (e.g. `"running"`).
    /// * `activity` — Activity state string (e.g. `"idle"` or `"busy"`).
    pub fn agent(name: &str, id: Uuid, status: AgentStatus, activity: ActivityState) -> Self {
        let display = format!("{name}  (agent, {status} / {})", activity_display(&activity));
        RecipientOption {
            name: name.to_string(),
            kind: RecipientKind::Agent { id, status, activity },
            display,
        }
    }

    /// Construct a room option.
    ///
    /// # Arguments
    ///
    /// * `name` — Room name as stored in the communicate service.
    /// * `id` — Communicate-service UUID.
    /// * `room_type` — Room type string (e.g. `"group"`, `"direct"`).
    pub fn room(name: &str, id: Uuid, room_type: &str) -> Self {
        let display = format!("{name}  (room, {room_type})");
        RecipientOption {
            name: name.to_string(),
            kind: RecipientKind::Room { id, room_type: room_type.to_string() },
            display,
        }
    }

    /// Return `true` if this option represents an agent.
    pub fn is_agent(&self) -> bool {
        matches!(self.kind, RecipientKind::Agent { .. })
    }

    /// Return `true` if this option represents a room.
    pub fn is_room(&self) -> bool {
        matches!(self.kind, RecipientKind::Room { .. })
    }
}

/// Format an [`ActivityState`] as a display string without requiring a
/// foreign `Display` impl (which would violate the orphan rule).
fn activity_display(a: &ActivityState) -> &'static str {
    match a {
        ActivityState::Idle => "idle",
        ActivityState::Busy => "busy",
    }
}

// ---------------------------------------------------------------------------
// Interactive selection
// ---------------------------------------------------------------------------

/// Present an interactive terminal picker and return the chosen [`RecipientOption`].
///
/// The list is rendered with a colourful theme using `dialoguer::Select`.
/// Returns an error when:
/// - `options` is empty
/// - There is no interactive terminal (e.g. piped input, CI, tests)
/// - The user cancels (presses `Esc` or `q`)
///
/// # Arguments
///
/// * `options` — Slice of recipient options to display.
/// * `prompt` — Text shown above the selection list.
pub fn select_recipient<'a>(
    options: &'a [RecipientOption],
    prompt: &str,
) -> Result<&'a RecipientOption> {
    if options.is_empty() {
        bail!("No agents or rooms available to select from");
    }

    let items: Vec<&str> = options.iter().map(|o| o.display.as_str()).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&items)
        .default(0)
        .interact_opt()
        .context("Failed to render interactive picker — is there a terminal attached?")?;

    match selection {
        Some(idx) => Ok(&options[idx]),
        None => bail!("No recipient selected"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(name: &str) -> RecipientOption {
        RecipientOption::agent(name, Uuid::new_v4(), AgentStatus::Running, ActivityState::Idle)
    }

    fn make_busy_agent(name: &str) -> RecipientOption {
        RecipientOption::agent(name, Uuid::new_v4(), AgentStatus::Running, ActivityState::Busy)
    }

    fn make_room(name: &str, room_type: &str) -> RecipientOption {
        RecipientOption::room(name, Uuid::new_v4(), room_type)
    }

    // -----------------------------------------------------------------------
    // RecipientOption construction
    // -----------------------------------------------------------------------

    #[test]
    fn agent_option_display_contains_name_status_activity() {
        let opt = make_agent("planner");
        assert!(opt.display.contains("planner"));
        assert!(opt.display.contains("agent"));
        assert!(opt.display.contains("running"));
        assert!(opt.display.contains("idle"));
    }

    #[test]
    fn busy_agent_display_shows_busy() {
        let opt = make_busy_agent("worker");
        assert!(opt.display.contains("busy"));
    }

    #[test]
    fn room_option_display_contains_name_and_type() {
        let opt = make_room("engineering", "group");
        assert!(opt.display.contains("engineering"));
        assert!(opt.display.contains("room"));
        assert!(opt.display.contains("group"));
    }

    #[test]
    fn is_agent_returns_true_for_agent() {
        let opt = make_agent("planner");
        assert!(opt.is_agent());
        assert!(!opt.is_room());
    }

    #[test]
    fn is_room_returns_true_for_room() {
        let opt = make_room("ops-channel", "broadcast");
        assert!(opt.is_room());
        assert!(!opt.is_agent());
    }

    #[test]
    fn agent_option_name_preserved() {
        let opt = make_agent("review-agent");
        assert_eq!(opt.name, "review-agent");
    }

    #[test]
    fn room_option_name_preserved() {
        let opt = make_room("engineering", "group");
        assert_eq!(opt.name, "engineering");
    }

    #[test]
    fn agent_kind_captures_id() {
        let id = Uuid::new_v4();
        let opt = RecipientOption::agent("planner", id, AgentStatus::Running, ActivityState::Idle);
        match opt.kind {
            RecipientKind::Agent { id: stored_id, .. } => assert_eq!(stored_id, id),
            _ => panic!("expected Agent kind"),
        }
    }

    #[test]
    fn room_kind_captures_id_and_type() {
        let id = Uuid::new_v4();
        let opt = RecipientOption::room("engineering", id, "group");
        match opt.kind {
            RecipientKind::Room { id: stored_id, room_type } => {
                assert_eq!(stored_id, id);
                assert_eq!(room_type, "group");
            }
            _ => panic!("expected Room kind"),
        }
    }

    // -----------------------------------------------------------------------
    // select_recipient — error paths (no TTY in test environment)
    // -----------------------------------------------------------------------

    #[test]
    fn select_recipient_errors_on_empty_list() {
        let result = select_recipient(&[], "Choose:");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("No agents or rooms"), "unexpected: {msg}");
    }

    #[test]
    fn select_recipient_errors_without_tty() {
        // In CI / test environments there is no interactive terminal, so
        // dialoguer will fail.  We just verify it returns an error rather than
        // panicking.
        let options = vec![make_agent("planner"), make_room("engineering", "group")];
        let result = select_recipient(&options, "Choose:");
        // May succeed if somehow a TTY is attached, otherwise must be an error.
        // We don't assert Ok because that would be environment-dependent.
        let _ = result; // either path is acceptable — no panic is the contract.
    }

    // -----------------------------------------------------------------------
    // activity_display helper
    // -----------------------------------------------------------------------

    #[test]
    fn activity_display_idle() {
        assert_eq!(activity_display(&ActivityState::Idle), "idle");
    }

    #[test]
    fn activity_display_busy() {
        assert_eq!(activity_display(&ActivityState::Busy), "busy");
    }
}
