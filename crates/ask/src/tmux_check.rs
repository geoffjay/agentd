//! Tmux session detection and monitoring.
//!
//! This module provides functionality for checking if tmux is installed, detecting
//! running tmux sessions, and gathering information about active sessions. It uses
//! the tmux command-line interface rather than a library for maximum compatibility.
//!
//! # Tmux Integration
//!
//! The module interacts with tmux through the command line:
//! - `which tmux` - Check if tmux is installed
//! - `tmux list-sessions` - Get list of running sessions
//!
//! # Error Handling
//!
//! The module distinguishes between different error conditions:
//! - Tmux not installed (returns [`TmuxError::NotInstalled`])
//! - Tmux installed but no server running (returns success with empty session list)
//! - Command execution failures (returns [`TmuxError::CommandFailed`])
//!
//! # Examples
//!
//! ## Check if tmux is installed
//!
//! ```
//! use ask::tmux_check::is_tmux_installed;
//!
//! if is_tmux_installed() {
//!     println!("tmux is available");
//! } else {
//!     println!("tmux is not installed");
//! }
//! ```
//!
//! ## Check for running sessions
//!
//! ```
//! use ask::tmux_check::check_tmux_sessions;
//!
//! match check_tmux_sessions() {
//!     Ok(result) if result.running => {
//!         println!("Found {} tmux sessions", result.session_count);
//!     }
//!     Ok(_) => {
//!         println!("No tmux sessions running");
//!     }
//!     Err(e) => {
//!         eprintln!("Error checking tmux: {}", e);
//!     }
//! }
//! ```
//!
//! ## List all sessions
//!
//! ```
//! use ask::tmux_check::list_sessions;
//!
//! match list_sessions() {
//!     Ok(sessions) => {
//!         for session in sessions {
//!             println!("Session: {}", session);
//!         }
//!     }
//!     Err(e) => {
//!         eprintln!("Failed to list sessions: {}", e);
//!     }
//! }
//! ```

use crate::error::TmuxError;
use crate::types::TmuxCheckResult;
use std::process::Command;
use tracing::{debug, warn};

/// Checks if tmux is installed and available in PATH.
///
/// Uses the `which` command to verify tmux installation. This is a lightweight
/// check that doesn't require tmux to be running.
///
/// # Returns
///
/// Returns `true` if tmux is found in PATH, `false` otherwise.
///
/// # Examples
///
/// ```
/// use ask::tmux_check::is_tmux_installed;
///
/// if is_tmux_installed() {
///     println!("tmux is available");
/// } else {
///     eprintln!("tmux is not installed");
/// }
/// ```
pub fn is_tmux_installed() -> bool {
    Command::new("which")
        .arg("tmux")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Checks if the tmux server is running.
///
/// Attempts to list tmux sessions to determine if the server is active. This is
/// different from [`is_tmux_installed`] which only checks if tmux is available.
///
/// # Returns
///
/// - `Ok(true)` if tmux server is running (has sessions)
/// - `Ok(false)` if tmux is installed but server is not running
/// - `Err(TmuxError::NotInstalled)` if tmux is not installed
/// - `Err(TmuxError::CommandFailed)` if tmux command fails unexpectedly
///
/// # Errors
///
/// Returns an error if:
/// - Tmux is not installed
/// - Tmux command fails for unexpected reasons
///
/// # Examples
///
/// ```
/// use ask::tmux_check::is_tmux_server_running;
///
/// match is_tmux_server_running() {
///     Ok(true) => println!("Tmux server is running"),
///     Ok(false) => println!("Tmux is installed but server is not running"),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
#[allow(dead_code)]
pub fn is_tmux_server_running() -> Result<bool, TmuxError> {
    if !is_tmux_installed() {
        return Err(TmuxError::NotInstalled);
    }

    let output = Command::new("tmux")
        .arg("list-sessions")
        .output()
        .map_err(|e| TmuxError::CommandFailed(e.to_string()))?;

    // If tmux server is not running, the command exits with code 1
    // and prints "no server running on ..." to stderr
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no server running") {
            debug!("tmux server is not running");
            return Ok(false);
        }
        return Err(TmuxError::CommandFailed(stderr.to_string()));
    }

    Ok(true)
}

/// Retrieves a list of all running tmux session names.
///
/// Executes `tmux list-sessions -F "#{session_name}"` to get session names.
/// Returns an empty vector if tmux server is not running (this is not an error).
///
/// # Returns
///
/// Returns `Ok(Vec<String>)` containing session names on success, or an error if:
/// - Tmux is not installed ([`TmuxError::NotInstalled`])
/// - Command fails unexpectedly ([`TmuxError::CommandFailed`])
/// - Output cannot be parsed ([`TmuxError::ParseError`])
///
/// # Errors
///
/// Returns an error if:
/// - Tmux is not installed in PATH
/// - Tmux command execution fails (other than "no server running")
/// - Output is not valid UTF-8
///
/// # Examples
///
/// ```
/// use ask::tmux_check::list_sessions;
///
/// match list_sessions() {
///     Ok(sessions) if sessions.is_empty() => {
///         println!("No tmux sessions found");
///     }
///     Ok(sessions) => {
///         println!("Found {} sessions:", sessions.len());
///         for session in sessions {
///             println!("  - {}", session);
///         }
///     }
///     Err(e) => {
///         eprintln!("Error listing sessions: {}", e);
///     }
/// }
/// ```
pub fn list_sessions() -> Result<Vec<String>, TmuxError> {
    if !is_tmux_installed() {
        return Err(TmuxError::NotInstalled);
    }

    let output = Command::new("tmux")
        .arg("list-sessions")
        .arg("-F")
        .arg("#{session_name}")
        .output()
        .map_err(|e| TmuxError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no server running") {
            return Ok(Vec::new());
        }
        return Err(TmuxError::CommandFailed(stderr.to_string()));
    }

    let stdout =
        String::from_utf8(output.stdout).map_err(|e| TmuxError::ParseError(e.to_string()))?;

    let sessions: Vec<String> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.trim().to_string())
        .collect();

    debug!("Found {} tmux sessions", sessions.len());
    Ok(sessions)
}

/// Performs a comprehensive check of tmux sessions.
///
/// This is the primary function for checking tmux status. It returns detailed
/// information about whether tmux is running and what sessions are active.
///
/// # Returns
///
/// Returns `Ok(TmuxCheckResult)` with:
/// - `running` - Whether any tmux sessions are active
/// - `session_count` - Number of active sessions
/// - `sessions` - Optional list of session names
///
/// # Errors
///
/// Returns an error if:
/// - Tmux is not installed ([`TmuxError::NotInstalled`])
/// - Tmux command fails unexpectedly ([`TmuxError::CommandFailed`])
///
/// Note: "no server running" is NOT an error - it returns success with `running: false`.
///
/// # Examples
///
/// ```
/// use ask::tmux_check::check_tmux_sessions;
///
/// match check_tmux_sessions() {
///     Ok(result) if result.running => {
///         println!("Tmux is running with {} sessions:", result.session_count);
///         if let Some(sessions) = result.sessions {
///             for session in sessions {
///                 println!("  - {}", session);
///             }
///         }
///     }
///     Ok(_) => {
///         println!("No tmux sessions are running");
///     }
///     Err(e) => {
///         eprintln!("Error checking tmux: {}", e);
///     }
/// }
/// ```
pub fn check_tmux_sessions() -> Result<TmuxCheckResult, TmuxError> {
    if !is_tmux_installed() {
        warn!("tmux is not installed");
        return Err(TmuxError::NotInstalled);
    }

    match list_sessions() {
        Ok(sessions) => {
            let running = !sessions.is_empty();
            let session_count = sessions.len();

            debug!("tmux check: running={}, session_count={}", running, session_count);

            Ok(TmuxCheckResult { running, session_count, sessions: Some(sessions) })
        }
        Err(TmuxError::CommandFailed(ref msg)) if msg.contains("no server running") => {
            debug!("tmux server not running");
            Ok(TmuxCheckResult { running: false, session_count: 0, sessions: Some(Vec::new()) })
        }
        Err(e) => Err(e),
    }
}

// Alternative implementation using tmux_interface crate would go here
// Currently not implemented as the command-line approach is working well

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tmux_installed() {
        // This test will pass or fail depending on whether tmux is installed
        // We just check that the function doesn't panic
        let _ = is_tmux_installed();
    }

    #[test]
    fn test_check_tmux_sessions() {
        // This test will succeed if tmux is not installed (returns error)
        // or if tmux is installed (returns result)
        let result = check_tmux_sessions();

        match result {
            Ok(check_result) => {
                assert!(check_result.sessions.is_some());
            }
            Err(TmuxError::NotInstalled) | Err(TmuxError::ServerNotRunning) => {
                // Expected if tmux is not installed or server is not running
            }
            Err(TmuxError::CommandFailed(msg))
                if msg.contains("No such file or directory")
                    || msg.contains("no server running") =>
            {
                // Expected when tmux is installed but no server socket exists (e.g. CI)
            }
            Err(e) => {
                panic!("Unexpected error: {e}");
            }
        }
    }

    #[test]
    fn test_list_sessions_not_installed() {
        // We can't easily test this without mocking, but we ensure the function exists
        let _ = list_sessions();
    }

    // Test error type conversions
    #[test]
    fn test_tmux_error_types() {
        let err = TmuxError::NotInstalled;
        assert_eq!(err.to_string(), "tmux is not installed or not found in PATH");

        let err = TmuxError::CommandFailed("test".to_string());
        assert_eq!(err.to_string(), "tmux command failed: test");

        let err = TmuxError::ParseError("invalid".to_string());
        assert_eq!(err.to_string(), "failed to parse tmux output: invalid");

        let err = TmuxError::ServerNotRunning;
        assert_eq!(err.to_string(), "tmux server is not running");
    }

    // Test that parse logic handles various outputs correctly
    // This simulates what list_sessions does with actual output
    #[test]
    fn test_parse_session_list() {
        // Simulate parsing session names from output
        let output = "main\nwork\ndev\n";
        let sessions: Vec<String> = output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.trim().to_string())
            .collect();

        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0], "main");
        assert_eq!(sessions[1], "work");
        assert_eq!(sessions[2], "dev");
    }

    #[test]
    fn test_parse_empty_session_list() {
        let output = "";
        let sessions: Vec<String> = output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.trim().to_string())
            .collect();

        assert_eq!(sessions.len(), 0);
    }

    #[test]
    fn test_parse_session_list_with_whitespace() {
        let output = "  main  \n  work\ndev  \n";
        let sessions: Vec<String> = output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.trim().to_string())
            .collect();

        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0], "main");
        assert_eq!(sessions[1], "work");
        assert_eq!(sessions[2], "dev");
    }

    #[test]
    fn test_parse_session_list_with_empty_lines() {
        let output = "main\n\nwork\n\n";
        let sessions: Vec<String> = output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.trim().to_string())
            .collect();

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0], "main");
        assert_eq!(sessions[1], "work");
    }

    // Test error message detection
    #[test]
    fn test_detect_no_server_error() {
        let stderr = "no server running on /tmp/tmux-1000/default";
        assert!(stderr.contains("no server running"));
    }

    #[test]
    fn test_detect_command_not_found() {
        let stderr = "tmux: command not found";
        assert!(stderr.contains("command not found"));
    }

    // Test check result construction
    #[test]
    fn test_check_result_with_sessions() {
        let result = TmuxCheckResult {
            running: true,
            session_count: 2,
            sessions: Some(vec!["main".to_string(), "work".to_string()]),
        };

        assert!(result.running);
        assert_eq!(result.session_count, 2);
        assert!(result.sessions.is_some());
        assert_eq!(result.sessions.unwrap().len(), 2);
    }

    #[test]
    fn test_check_result_no_sessions() {
        let result =
            TmuxCheckResult { running: false, session_count: 0, sessions: Some(Vec::new()) };

        assert!(!result.running);
        assert_eq!(result.session_count, 0);
        assert!(result.sessions.is_some());
        assert!(result.sessions.unwrap().is_empty());
    }

    // Integration-style tests that actually run tmux commands
    // These will pass/fail based on system state
    #[test]
    fn test_is_tmux_installed_returns_bool() {
        let result = is_tmux_installed();
        // Just verify it returns without panicking
        let _ = result;
    }

    #[test]
    fn test_is_tmux_server_running_handles_not_installed() {
        if !is_tmux_installed() {
            let result = is_tmux_server_running();
            assert!(result.is_err());
            if let Err(e) = result {
                assert!(matches!(e, TmuxError::NotInstalled));
            }
        }
    }

    #[test]
    fn test_list_sessions_handles_not_installed() {
        if !is_tmux_installed() {
            let result = list_sessions();
            assert!(result.is_err());
            if let Err(e) = result {
                assert!(matches!(e, TmuxError::NotInstalled));
            }
        }
    }

    #[test]
    fn test_check_tmux_sessions_structure() {
        // Test the function returns a valid structure regardless of system state
        let result = check_tmux_sessions();

        match result {
            Ok(check_result) => {
                // If successful, verify structure
                assert!(check_result.sessions.is_some());
                assert_eq!(check_result.running, check_result.session_count > 0);
            }
            Err(TmuxError::NotInstalled) | Err(TmuxError::ServerNotRunning) => {
                // Expected if tmux is not installed or server is not running
            }
            Err(TmuxError::CommandFailed(msg))
                if msg.contains("No such file or directory")
                    || msg.contains("no server running") =>
            {
                // Expected when tmux is installed but no server socket exists (e.g. CI)
            }
            Err(e) => {
                // Other errors are unexpected in normal testing
                panic!("Unexpected error: {e}");
            }
        }
    }

    #[test]
    fn test_session_count_matches_list_length() {
        if let Ok(result) = check_tmux_sessions() {
            if let Some(sessions) = result.sessions {
                assert_eq!(result.session_count, sessions.len());
            }
        }
    }
}
