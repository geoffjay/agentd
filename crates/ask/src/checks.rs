#![allow(dead_code)]
//! Extensible check type registry for the ask service.
//!
//! This module defines the [`Check`] trait and [`CheckRegistry`] that allow
//! the ask service to support multiple, independently implemented environment
//! checks.  Each check encapsulates its own run logic, question template text,
//! and associated [`CheckType`].
//!
//! # Architecture
//!
//! - **[`Check`]** — trait implemented by every check (tmux, service health, …)
//! - **[`CheckResult`]** — structured output produced by running a check
//! - **[`QuestionTemplate`]** — title/message pair used to create a notification
//! - **[`CheckRegistry`]** — ordered list of registered checks; used by the
//!   trigger handler to iterate and run all enabled checks
//!
//! # Adding a New Check
//!
//! 1. Implement the [`Check`] trait for your struct.
//! 2. Register it with [`CheckRegistry::register`].
//! 3. The trigger handler will pick it up automatically.
//!
//! # Examples
//!
//! ```rust
//! use ask::checks::{Check, CheckRegistry, CheckResult, QuestionTemplate};
//! use ask::types::CheckType;
//!
//! struct MyCheck;
//!
//! #[async_trait::async_trait]
//! impl Check for MyCheck {
//!     fn name(&self) -> &str { "my_check" }
//!     fn check_type(&self) -> CheckType { CheckType::TmuxSessions }
//!     async fn run(&self) -> Result<CheckResult, ask::checks::CheckError> {
//!         Ok(CheckResult { needs_action: false, detail: serde_json::Value::Null })
//!     }
//!     fn question_template(&self) -> QuestionTemplate {
//!         QuestionTemplate {
//!             title: "My check".to_string(),
//!             message: "Something happened — what would you like to do?".to_string(),
//!         }
//!     }
//! }
//!
//! let mut registry = CheckRegistry::new();
//! registry.register(Box::new(MyCheck));
//! assert_eq!(registry.len(), 1);
//! ```

use crate::types::CheckType;
use async_trait::async_trait;
use thiserror::Error;

/// Error type for check execution failures.
#[derive(Debug, Error)]
pub enum CheckError {
    /// The required tool or binary is not available.
    #[error("tool not available: {0}")]
    #[allow(dead_code)]
    ToolNotAvailable(String),
    /// The check command failed to execute.
    #[error("check execution failed: {0}")]
    ExecutionFailed(String),
    /// Check produced unexpected output.
    #[error("unexpected output: {0}")]
    #[allow(dead_code)]
    UnexpectedOutput(String),
}

/// Structured output produced by running a check.
///
/// `needs_action` indicates whether the trigger handler should create a
/// notification for this result.  `detail` carries check-specific data that
/// is returned in the trigger response.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Whether this result should trigger a user notification.
    pub needs_action: bool,
    /// Check-specific detail payload (serialised as JSON in the response).
    pub detail: serde_json::Value,
}

/// Title and message text used when creating a notification for a check result.
#[derive(Debug, Clone)]
pub struct QuestionTemplate {
    /// Short notification title.
    pub title: String,
    /// Longer notification body / question text.
    pub message: String,
}

/// Trait implemented by every environment check.
///
/// Implementations must be `Send + Sync` so they can be stored in the
/// registry and called from async handlers.
#[async_trait]
pub trait Check: Send + Sync {
    /// Short identifier, e.g. `"tmux_sessions"`.
    fn name(&self) -> &str;

    /// The [`CheckType`] enum variant this check corresponds to.
    fn check_type(&self) -> CheckType;

    /// Execute the check and return a structured result.
    async fn run(&self) -> Result<CheckResult, CheckError>;

    /// Title and message to use when creating a notification for this check.
    fn question_template(&self) -> QuestionTemplate;
}

/// Registry of all registered checks.
///
/// The trigger handler iterates this registry and runs every check in order.
/// Checks are stored as boxed trait objects so any type implementing [`Check`]
/// can be registered.
///
/// # Examples
///
/// ```rust
/// use ask::checks::CheckRegistry;
///
/// let registry = CheckRegistry::default();
/// assert_eq!(registry.len(), 0);
/// ```
#[allow(dead_code)]
pub struct CheckRegistry {
    checks: Vec<Box<dyn Check>>,
}

impl CheckRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Registers a check.  Checks are run in registration order.
    pub fn register(&mut self, check: Box<dyn Check>) {
        self.checks.push(check);
    }

    /// Returns the number of registered checks.
    pub fn len(&self) -> usize {
        self.checks.len()
    }

    /// Returns `true` if no checks are registered.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    /// Returns a slice over all registered checks.
    pub fn checks(&self) -> &[Box<dyn Check>] {
        &self.checks
    }
}

impl Default for CheckRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Built-in check implementations ────────────────────────────────────────────

/// Check for running tmux sessions.
///
/// Returns `needs_action = true` when no tmux sessions are running.
pub struct TmuxSessionsCheck;

#[async_trait]
impl Check for TmuxSessionsCheck {
    fn name(&self) -> &str {
        "tmux_sessions"
    }

    fn check_type(&self) -> CheckType {
        CheckType::TmuxSessions
    }

    async fn run(&self) -> Result<CheckResult, CheckError> {
        let result = crate::tmux_check::check_tmux_sessions()
            .map_err(|e| CheckError::ExecutionFailed(e.to_string()))?;

        let detail = serde_json::json!({
            "running": result.running,
            "session_count": result.session_count,
            "sessions": result.sessions.unwrap_or_default(),
        });

        Ok(CheckResult { needs_action: !result.running, detail })
    }

    fn question_template(&self) -> QuestionTemplate {
        QuestionTemplate {
            title: "Start tmux session?".to_string(),
            message: "No tmux sessions are currently running. Would you like to start one?"
                .to_string(),
        }
    }
}

/// Example check: verify that other agentd services are reachable.
///
/// `needs_action = true` when the target URL returns a non-2xx status or is
/// unreachable.
#[allow(dead_code)]
pub struct ServiceHealthCheck {
    /// Human-readable name, e.g. `"notify_service"`.
    pub service_name: String,
    /// Base URL to GET (expected to respond with 2xx).
    pub url: String,
}

#[async_trait]
impl Check for ServiceHealthCheck {
    fn name(&self) -> &str {
        &self.service_name
    }

    fn check_type(&self) -> CheckType {
        CheckType::ServiceHealth
    }

    async fn run(&self) -> Result<CheckResult, CheckError> {
        let url = format!("{}/health", self.url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| CheckError::ExecutionFailed(e.to_string()))?;

        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(CheckResult {
                needs_action: false,
                detail: serde_json::json!({
                    "service": self.service_name,
                    "url": url,
                    "healthy": true,
                    "status": resp.status().as_u16(),
                }),
            }),
            Ok(resp) => Ok(CheckResult {
                needs_action: true,
                detail: serde_json::json!({
                    "service": self.service_name,
                    "url": url,
                    "healthy": false,
                    "status": resp.status().as_u16(),
                }),
            }),
            Err(e) => Ok(CheckResult {
                needs_action: true,
                detail: serde_json::json!({
                    "service": self.service_name,
                    "url": url,
                    "healthy": false,
                    "error": e.to_string(),
                }),
            }),
        }
    }

    fn question_template(&self) -> QuestionTemplate {
        QuestionTemplate {
            title: format!("Service {} unreachable", self.service_name),
            message: format!(
                "The {} service at {} is not responding. Would you like to restart it?",
                self.service_name, self.url
            ),
        }
    }
}

/// Build the default registry with all built-in checks.
///
/// Enabled checks are controlled by the `AGENTD_CHECKS` environment variable.
/// Set it to a comma-separated list of check names to enable only those checks,
/// or leave it unset to enable all built-in checks.
///
/// # Examples
///
/// ```bash
/// # Enable only tmux_sessions check
/// AGENTD_CHECKS=tmux_sessions cargo run
///
/// # Enable all checks (default)
/// cargo run
/// ```
pub fn default_registry() -> CheckRegistry {
    let enabled = std::env::var("AGENTD_CHECKS").ok();
    let enabled_checks: Option<Vec<&str>> =
        enabled.as_deref().map(|s| s.split(',').map(str::trim).collect());

    let mut registry = CheckRegistry::new();

    let is_enabled =
        |name: &str| -> bool { enabled_checks.as_ref().is_none_or(|list| list.contains(&name)) };

    if is_enabled("tmux_sessions") {
        registry.register(Box::new(TmuxSessionsCheck));
    }

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_empty() {
        let registry = CheckRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = CheckRegistry::new();
        registry.register(Box::new(TmuxSessionsCheck));
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_multiple() {
        let mut registry = CheckRegistry::new();
        registry.register(Box::new(TmuxSessionsCheck));
        registry.register(Box::new(ServiceHealthCheck {
            service_name: "notify".to_string(),
            url: "http://localhost:17004".to_string(),
        }));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_tmux_check_name_and_type() {
        let check = TmuxSessionsCheck;
        assert_eq!(check.name(), "tmux_sessions");
        assert_eq!(check.check_type(), CheckType::TmuxSessions);
    }

    #[test]
    fn test_tmux_question_template() {
        let check = TmuxSessionsCheck;
        let tpl = check.question_template();
        assert!(!tpl.title.is_empty());
        assert!(!tpl.message.is_empty());
    }

    #[test]
    fn test_service_health_check_name() {
        let check = ServiceHealthCheck {
            service_name: "notify".to_string(),
            url: "http://localhost:17004".to_string(),
        };
        assert_eq!(check.name(), "notify");
        assert_eq!(check.check_type(), CheckType::ServiceHealth);
    }

    #[test]
    fn test_service_health_question_template() {
        let check = ServiceHealthCheck {
            service_name: "notify".to_string(),
            url: "http://localhost:17004".to_string(),
        };
        let tpl = check.question_template();
        assert!(tpl.title.contains("notify"));
        assert!(tpl.message.contains("notify"));
    }

    #[test]
    fn test_default_registry_builds() {
        // Ensure default_registry() doesn't panic
        let registry = default_registry();
        // At minimum, TmuxSessions should be registered by default
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_default_registry_env_filter() {
        // Verify the is_enabled logic directly without touching the real env var.
        // Build a mini-registry using the same logic as default_registry but
        // with an explicit enabled list.
        let enabled: Option<Vec<&str>> = Some(vec![""]); // empty name — nothing matches
        let is_enabled =
            |name: &str| -> bool { enabled.as_ref().is_none_or(|list| list.contains(&name)) };
        assert!(!is_enabled("tmux_sessions"));
        assert!(!is_enabled("service_health"));

        // With None (no env var), everything is enabled.
        let all_enabled: Option<Vec<&str>> = None;
        let is_all =
            |name: &str| -> bool { all_enabled.as_ref().is_none_or(|list| list.contains(&name)) };
        assert!(is_all("tmux_sessions"));
        assert!(is_all("service_health"));
    }

    #[test]
    fn test_registry_checks_slice() {
        let mut registry = CheckRegistry::new();
        registry.register(Box::new(TmuxSessionsCheck));
        let checks = registry.checks();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name(), "tmux_sessions");
    }

    #[test]
    fn test_check_error_display() {
        let e = CheckError::ToolNotAvailable("tmux".to_string());
        assert!(e.to_string().contains("tmux"));

        let e = CheckError::ExecutionFailed("exit 1".to_string());
        assert!(e.to_string().contains("exit 1"));
    }
}
