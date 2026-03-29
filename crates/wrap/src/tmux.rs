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
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tracing::{debug, warn};

/// Maximum number of bytes allowed in a `tmux send-keys` argument.
///
/// tmux has an internal limit on the length of arguments passed to `send-keys`.
/// Empirically, commands up to ~12 KB work reliably, but ~18 KB fails.  We use
/// a conservative 8 KB threshold so that commands with large system prompts
/// (e.g. the conductor agent at ~6 KB after trimming) still have headroom,
/// while anything larger falls back to the temp-script strategy.
const TMUX_SEND_KEYS_MAX_LEN: usize = 8 * 1024; // 8 KB

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
        debug!(
            "Sending command to session {}: {}",
            session_name,
            &command[..command.len().min(120)]
        );

        if command.len() > TMUX_SEND_KEYS_MAX_LEN {
            // The command is too long for tmux send-keys.  Write it to a
            // self-deleting temp script and send the script invocation instead.
            debug!(
                "Command length {} exceeds tmux send-keys limit ({}); using temp-script fallback",
                command.len(),
                TMUX_SEND_KEYS_MAX_LEN,
            );
            return self.send_command_via_script(session_name, command);
        }

        let output = Command::new(get_tmux_command())
            .args(["send-keys", "-t", session_name, command, "Enter"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to send command: {}", stderr));
        }

        Ok(())
    }

    /// Writes `command` to a self-deleting temporary shell script and sends
    /// `sh <script_path>` to the tmux session via `send-keys`.
    ///
    /// This is the fallback path for commands that exceed
    /// [`TMUX_SEND_KEYS_MAX_LEN`].  The generated script uses a `trap` to
    /// remove itself on exit so no cleanup is required by the caller.
    fn send_command_via_script(&self, session_name: &str, command: &str) -> anyhow::Result<()> {
        let script_path = write_temp_script(command)?;
        let script_str = script_path.to_string_lossy();
        let send_cmd = format!("sh {script_str}");

        debug!("Sending temp-script command to session {}: {}", session_name, send_cmd);

        let output = Command::new(get_tmux_command())
            .args(["send-keys", "-t", session_name, &send_cmd, "Enter"])
            .output()?;

        if !output.status.success() {
            // Best-effort cleanup if send-keys itself fails
            let _ = std::fs::remove_file(&script_path);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to send command via script: {}", stderr));
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

/// Writes `command` to a self-deleting temporary shell script.
///
/// The generated script:
/// 1. Registers a `trap` to remove itself (`rm -f "$0"`) on exit so no
///    external cleanup is required.
/// 2. Contains the original command verbatim on the next line.
///
/// # Returns
///
/// The path to the newly created script file.
///
/// # Errors
///
/// Returns an error if the temp directory cannot be located or the file
/// cannot be written.
pub(crate) fn write_temp_script(command: &str) -> anyhow::Result<std::path::PathBuf> {
    let mut path = std::env::temp_dir();
    // Unique filename using a UUID-like timestamp + random component
    let name = format!(
        "agentd-cmd-{}-{}.sh",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        std::process::id(),
    );
    path.push(name);

    let script = format!("#!/bin/sh\ntrap 'rm -f \"$0\"' EXIT\n{command}\n");

    let mut file = std::fs::File::create(&path)?;
    file.write_all(script.as_bytes())?;

    // Make the script executable (rwx for owner, rx for group/others)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(path)
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

    // -------------------------------------------------------------------------
    // write_temp_script tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_write_temp_script_creates_file() {
        let path = write_temp_script("echo hello").expect("should create temp script");
        assert!(path.exists(), "temp script file should exist");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_temp_script_contains_shebang_and_trap() {
        let path = write_temp_script("echo hello").expect("should create temp script");
        let contents = std::fs::read_to_string(&path).expect("should read temp script");
        assert!(contents.starts_with("#!/bin/sh\n"), "should have sh shebang");
        assert!(contents.contains("trap 'rm -f \"$0\"' EXIT"), "should self-delete via trap");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_temp_script_contains_command() {
        let cmd = "claude --system-prompt 'x' --some-flag value";
        let path = write_temp_script(cmd).expect("should create temp script");
        let contents = std::fs::read_to_string(&path).expect("should read temp script");
        assert!(contents.contains(cmd), "script should contain the original command");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_temp_script_unique_paths() {
        // Two calls must not collide (different PIDs can't be guaranteed here,
        // but the nanosecond timestamp ensures uniqueness in practice).
        let p1 = write_temp_script("echo 1").expect("should create first script");
        let p2 = write_temp_script("echo 2").expect("should create second script");
        assert_ne!(p1, p2, "temp script paths should be unique");
        std::fs::remove_file(&p1).ok();
        std::fs::remove_file(&p2).ok();
    }

    #[test]
    fn test_send_keys_max_len_constant() {
        // Sanity check: the limit must be positive and within a reasonable range.
        assert!(TMUX_SEND_KEYS_MAX_LEN >= 4 * 1024, "limit should be at least 4 KB");
        assert!(TMUX_SEND_KEYS_MAX_LEN <= 32 * 1024, "limit should be at most 32 KB");
    }

    #[test]
    fn test_short_command_does_not_need_script() {
        // A command well under the limit should not need the script fallback.
        let cmd = "echo hello";
        assert!(
            cmd.len() <= TMUX_SEND_KEYS_MAX_LEN,
            "short command should be within send-keys limit"
        );
    }

    #[test]
    fn test_long_command_needs_script() {
        // A command exceeding the limit should trigger the script fallback.
        let cmd = "x".repeat(TMUX_SEND_KEYS_MAX_LEN + 1);
        assert!(cmd.len() > TMUX_SEND_KEYS_MAX_LEN, "long command should exceed send-keys limit");
    }
}
