//! Tmux session management.
//!
//! This module provides functionality for creating and managing tmux sessions
//! for agent workflows. It handles session creation, command execution within
//! sessions, and session lifecycle management.
//!
//! # Examples
//!
//! ```no_run
//! use wrap::tmux::TmuxManager;
//!
//! let tmux = TmuxManager::new("agentd");
//!
//! // Create a session
//! tmux.create_session("my-session", "/path/to/project", None)?;
//!
//! // Send a command to the session
//! tmux.send_command("my-session", "echo 'Hello, world!'")?;
//!
//! // Check if session exists
//! if tmux.session_exists("my-session")? {
//!     println!("Session is running");
//! }
//!
//! // Kill the session when done
//! tmux.kill_session("my-session")?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use crate::types::TmuxLayout;
use std::path::Path;
use std::process::Command;
use tracing::{debug, warn};

/// Get the tmux binary path.
///
/// Checks common installation locations and falls back to PATH lookup.
/// This is necessary because Agent.app doesn't have Homebrew paths in its environment.
fn get_tmux_command() -> &'static str {
    // Check common tmux installation locations
    const COMMON_PATHS: &[&str] = &[
        "/opt/homebrew/bin/tmux", // Homebrew on Apple Silicon
        "/usr/local/bin/tmux",    // Homebrew on Intel / manual install
        "/usr/bin/tmux",          // System install
        "tmux",                   // Fallback to PATH
    ];

    for path in COMMON_PATHS {
        if *path == "tmux" || Path::new(path).exists() {
            return path;
        }
    }

    "tmux" // Final fallback
}

/// Tmux session manager.
///
/// Provides methods for creating and managing tmux sessions for agent workflows.
#[derive(Debug, Clone)]
pub struct TmuxManager {
    /// Prefix for session names
    prefix: String,
}

impl TmuxManager {
    /// Creates a new tmux manager with the specified prefix.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Prefix to use for session names (e.g., "agentd")
    ///
    /// # Examples
    ///
    /// ```
    /// use wrap::tmux::TmuxManager;
    ///
    /// let tmux = TmuxManager::new("agentd");
    /// ```
    pub fn new(prefix: impl Into<String>) -> Self {
        Self { prefix: prefix.into() }
    }

    /// Returns the session name prefix.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrap::tmux::TmuxManager;
    ///
    /// let tmux = TmuxManager::new("agentd");
    /// assert_eq!(tmux.prefix(), "agentd");
    /// ```
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Creates a new tmux session.
    ///
    /// Creates a detached tmux session with the specified name and working directory.
    /// If a layout is provided, the session will be configured with multiple panes
    /// according to the layout specification.
    ///
    /// # Arguments
    ///
    /// * `session_name` - Name for the tmux session
    /// * `working_dir` - Working directory for the session
    /// * `layout` - Optional layout configuration
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the session was created successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - tmux is not installed or not in PATH
    /// - The working directory does not exist
    /// - A session with the same name already exists
    /// - The tmux command fails for any other reason
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wrap::tmux::TmuxManager;
    ///
    /// let tmux = TmuxManager::new("agentd");
    /// tmux.create_session("my-session", "/path/to/project", None)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn create_session(
        &self,
        session_name: &str,
        working_dir: &str,
        layout: Option<&TmuxLayout>,
    ) -> anyhow::Result<()> {
        debug!("Creating tmux session: {} in {}", session_name, working_dir);

        // Create the base session
        let output = Command::new(get_tmux_command())
            .args([
                "new-session",
                "-d", // Detached
                "-s",
                session_name,
                "-c",
                working_dir,
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create tmux session: {}", stderr));
        }

        // Apply layout if specified
        if let Some(layout) = layout {
            self.apply_layout(session_name, layout)?;
        }

        Ok(())
    }

    /// Applies a layout to a tmux session.
    ///
    /// Configures the session with multiple panes according to the layout specification.
    ///
    /// # Arguments
    ///
    /// * `session_name` - Name of the tmux session
    /// * `layout` - Layout configuration to apply
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the layout was applied successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if the tmux command fails.
    fn apply_layout(&self, session_name: &str, layout: &TmuxLayout) -> anyhow::Result<()> {
        debug!("Applying layout to session {}: {:?}", session_name, layout);

        let panes = layout.panes.unwrap_or(1);
        if panes <= 1 {
            return Ok(()); // Single pane, nothing to do
        }

        // Create additional panes based on layout type
        let split_flag = match layout.layout_type.as_str() {
            "horizontal" => "-h",
            _ => "-v", // Default to vertical
        };

        // Create panes (one less than total, since we start with one)
        for _ in 1..panes {
            let output = Command::new(get_tmux_command())
                .args(["split-window", split_flag, "-t", session_name])
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to split window: {}", stderr);
            }
        }

        // Apply tiled layout if requested
        if layout.layout_type == "tiled" {
            let output = Command::new(get_tmux_command())
                .args(["select-layout", "-t", session_name, "tiled"])
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to apply tiled layout: {}", stderr);
            }
        }

        Ok(())
    }

    /// Sends a command to a tmux session.
    ///
    /// Executes the specified command in the first pane of the tmux session.
    /// The command is sent as if typed by the user, followed by Enter.
    ///
    /// # Arguments
    ///
    /// * `session_name` - Name of the tmux session
    /// * `command` - Command to execute
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the command was sent successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session does not exist
    /// - The tmux command fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wrap::tmux::TmuxManager;
    ///
    /// let tmux = TmuxManager::new("agentd");
    /// tmux.send_command("my-session", "echo 'Hello, world!'")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn send_command(&self, session_name: &str, command: &str) -> anyhow::Result<()> {
        debug!("Sending command to session {}: {}", session_name, command);

        let output = Command::new(get_tmux_command())
            .args(["send-keys", "-t", session_name, command, "Enter"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to send command: {}", stderr));
        }

        Ok(())
    }

    /// Checks if a tmux session exists.
    ///
    /// # Arguments
    ///
    /// * `session_name` - Name of the tmux session to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the session exists, `Ok(false)` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the tmux command fails for reasons other than
    /// the session not existing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wrap::tmux::TmuxManager;
    ///
    /// let tmux = TmuxManager::new("agentd");
    /// if tmux.session_exists("my-session")? {
    ///     println!("Session exists");
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn session_exists(&self, session_name: &str) -> anyhow::Result<bool> {
        let output =
            Command::new(get_tmux_command()).args(["has-session", "-t", session_name]).output()?;

        Ok(output.status.success())
    }

    /// Kills a tmux session.
    ///
    /// Terminates the specified tmux session and all processes running within it.
    ///
    /// # Arguments
    ///
    /// * `session_name` - Name of the tmux session to kill
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the session was killed successfully or if the
    /// session doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the tmux command fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wrap::tmux::TmuxManager;
    ///
    /// let tmux = TmuxManager::new("agentd");
    /// tmux.kill_session("my-session")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn kill_session(&self, session_name: &str) -> anyhow::Result<()> {
        debug!("Killing tmux session: {}", session_name);

        let output =
            Command::new(get_tmux_command()).args(["kill-session", "-t", session_name]).output()?;

        // Don't error if session doesn't exist
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("can't find session") {
                return Err(anyhow::anyhow!("Failed to kill session: {}", stderr));
            }
        }

        Ok(())
    }

    /// Lists all active tmux sessions.
    ///
    /// Returns a list of all currently running tmux sessions.
    ///
    /// # Returns
    ///
    /// Returns a vector of session names.
    ///
    /// # Errors
    ///
    /// Returns an error if the tmux command fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wrap::tmux::TmuxManager;
    ///
    /// let tmux = TmuxManager::new("agentd");
    /// let sessions = tmux.list_sessions()?;
    /// for session in sessions {
    ///     println!("Session: {}", session);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
        let output = Command::new(get_tmux_command())
            .args(["list-sessions", "-F", "#{session_name}"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // No sessions is not an error
            if stderr.contains("no server running") {
                return Ok(Vec::new());
            }
            return Err(anyhow::anyhow!("Failed to list sessions: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let sessions: Vec<String> = stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmux_manager_new() {
        let tmux = TmuxManager::new("test");
        assert_eq!(tmux.prefix(), "test");
    }

    #[test]
    fn test_session_naming() {
        let tmux = TmuxManager::new("agentd");
        let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let session_name = format!("{}-my-project-{}", tmux.prefix(), timestamp);
        assert!(session_name.starts_with("agentd-my-project-"));
    }
}
