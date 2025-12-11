// Hook analysis BAML functions for agentd
//
// This module provides intelligent analysis of shell hook events,
// determining which events warrant notifications and how to present them.

// ============================================================================
// Data Models
// ============================================================================

/// Decision about whether and how to notify for a hook event
class HookAction {
  /// Whether a notification should be sent
  should_notify bool

  /// Notification title (if should_notify is true)
  notification_title string

  /// Notification message body (if should_notify is true)
  notification_message string

  /// Suggested priority: "low", "normal", "high", "urgent"
  priority string

  /// Brief explanation of the decision
  reasoning string

  /// Metadata to attach to the notification for context
  metadata map<string, string>

  /// Whether this event indicates a problem
  indicates_problem bool

  /// Suggested actions for the user (if applicable)
  suggested_actions string[]?
}

/// Pattern learned from hook event history
class HookPattern {
  /// Description of the pattern
  pattern_description string

  /// Commands or events that match this pattern
  matching_criteria string

  /// Whether events matching this pattern should notify
  should_notify bool

  /// Suggested notification priority for this pattern
  notification_priority string

  /// How many times this pattern has been observed
  occurrence_count int

  /// Confidence in this pattern (0.0 - 1.0)
  confidence float
}

/// Intelligence about command execution
class CommandInsight {
  /// What the command appears to be doing
  purpose string

  /// Whether this is a long-running command
  is_long_running bool

  /// Expected behavior (success indicators)
  expected_outcomes string[]

  /// Potential issues to watch for
  potential_issues string[]

  /// Whether user typically cares about this command's completion
  user_cares_about_completion bool
}

// ============================================================================
// Functions
// ============================================================================

/// Analyze a shell event to determine if notification is needed
///
/// Intelligently decides whether a shell command execution warrants
/// a notification, and if so, what that notification should say.
///
/// # Arguments
/// - command: The shell command that was executed
/// - exit_code: Exit code of the command (0 = success)
/// - output: stdout/stderr from the command (truncated if long)
/// - duration_ms: How long the command took to execute
/// - context: Additional context (current directory, time of day, etc.)
///
/// # Returns
/// HookAction with notification decision and details
function AnalyzeShellEvent(
  command: string,
  exit_code: int,
  output: string,
  duration_ms: int,
  context: string
) -> HookAction {
  client AgentdFast
  prompt #"
    Analyze this shell command execution:

    **Command:** `{{ command }}`

    **Exit Code:** {{ exit_code }} (0 = success, non-zero = error)

    **Duration:** {{ duration_ms }}ms

    **Output:**
    ```
    {{ output }}
    ```

    **Context:** {{ context }}

    **Notification Decision Guidelines:**

    **DO Notify For:**
    1. **Long-running commands completing** (> 30 seconds)
       - Build commands (make, cargo build, npm install)
       - Tests (cargo test, npm test, pytest)
       - Deployments
       - Large file operations

    2. **Commands with errors** (exit_code != 0)
       - Especially if non-interactive
       - Include error message in notification

    3. **User-defined patterns**
       - Commands explicitly marked for notification
       - Commands matching learned patterns

    4. **Background/async operations**
       - Downloads, uploads
       - Background jobs

    **DON'T Notify For:**
    1. **Fast interactive commands** (< 5 seconds)
       - ls, cd, pwd, etc.
       - Quick file edits
       - Status checks

    2. **Expected failures**
       - Test commands often expected to fail during development
       - Conditional commands (grep with no match)

    3. **Silent commands**
       - Commands that explicitly suppress output
       - Background maintenance tasks

    **Notification Priority:**
    - **urgent**: Critical failures affecting system/data
    - **high**: Command failures, unexpected errors
    - **normal**: Long-running commands completed successfully
    - **low**: Informational, optional completions

    **Notification Content:**
    - Title: Brief, actionable (e.g., "Build Failed" not "Command Completed")
    - Message: Include key information (exit code, timing, error preview)
    - Context: Directory, relevant output lines

    Make your decision now.

    {{ ctx.output_format }}
  "#
}

/// Learn patterns from command history
///
/// Analyzes historical command executions to identify patterns
/// and improve future notification decisions.
///
/// # Arguments
/// - command_history: Recent command executions with outcomes
/// - notification_history: Which commands generated notifications and user response
///
/// # Returns
/// Array of HookPattern objects describing learned patterns
function LearnCommandPatterns(
  command_history: string[],
  notification_history: string[]
) -> HookPattern[] {
  client AgentdPrimary
  prompt #"
    Learn patterns from command execution history:

    **Command History:**
    {% for cmd in command_history %}
    - {{ cmd }}
    {% endfor %}

    **Notification History (and user reactions):**
    {% for notif in notification_history %}
    - {{ notif }}
    {% endfor %}

    **Pattern Learning Guidelines:**

    1. **Frequency Patterns**:
       - Commands run regularly (daily backups, cron jobs)
       - Only notify if they fail, not on success

    2. **Duration Patterns**:
       - If command usually takes X seconds, notify if it takes significantly longer
       - Learn typical execution times for common commands

    3. **Outcome Patterns**:
       - Some commands regularly fail but that's expected (test-driven development)
       - Other commands should always succeed (deployments, backups)

    4. **User Response Patterns**:
       - If user dismisses notifications for certain commands without action
       - If user always acts on certain notification types

    5. **Context Patterns**:
       - Time-of-day patterns (late-night commands might be more important)
       - Directory patterns (production dirs vs test dirs)

    **Pattern Output:**
    - Describe each pattern clearly
    - Provide matching criteria (regex, command prefix, etc.)
    - Specify notification behavior
    - Include confidence level

    Return only significant patterns (occurrence_count >= 3, confidence >= 0.7).

    {{ ctx.output_format }}
  "#
}

/// Provide insights about a command before execution
///
/// Analyzes a command that's about to run and provides intelligence
/// about what to expect, useful for setting up appropriate monitoring.
///
/// # Arguments
/// - command: The command that will be executed
/// - execution_context: Where and when it's running
///
/// # Returns
/// CommandInsight with purpose, expectations, and monitoring suggestions
function AnalyzeCommandIntent(
  command: string,
  execution_context: string
) -> CommandInsight {
  client AgentdFast
  prompt #"
    Analyze this command that's about to execute:

    **Command:** `{{ command }}`

    **Context:** {{ execution_context }}

    **Analysis Goals:**

    1. **Purpose**: What is this command doing?
       - Building/compiling
       - Testing
       - Deploying
       - Data processing
       - System maintenance
       - File operations

    2. **Long-running Detection**:
       - Likely to take > 30 seconds?
       - Based on command type (build, test, download)

    3. **Expected Outcomes**:
       - What indicates success?
       - What outputs are normal?

    4. **Potential Issues**:
       - Common failure modes for this command
       - What to watch for in output

    5. **User Interest**:
       - Do users typically care when this completes?
       - Background task vs. active work

    **Examples:**

    - `cargo build --release` → Long-running, user cares about completion
    - `git status` → Fast, informational only
    - `pytest tests/` → Medium duration, user cares about results
    - `scp large-file.zip server:/path` → Long-running, user wants notification

    Provide analysis now.

    {{ ctx.output_format }}
  "#
}

/// Generate smart notification content for command completion
///
/// Creates informative, actionable notification content for a
/// completed command execution.
///
/// # Arguments
/// - command: The command that completed
/// - exit_code: Exit code (0 = success)
/// - duration_ms: Execution time
/// - output_summary: Key lines from output
/// - previous_attempts: Whether this command was retried
///
/// # Returns
/// Notification title and message as a JSON-formatted string
function GenerateCompletionNotification(
  command: string,
  exit_code: int,
  duration_ms: int,
  output_summary: string,
  previous_attempts: int
) -> string {
  client AgentdFast
  prompt #"
    Generate notification content for this completed command:

    **Command:** `{{ command }}`

    **Exit Code:** {{ exit_code }}

    **Duration:** {{ duration_ms }}ms ({{ duration_ms / 1000 }} seconds)

    **Output Summary:**
    {{ output_summary }}

    **Previous Attempts:** {{ previous_attempts }}

    **Notification Guidelines:**

    **Title:**
    - Action-oriented and clear
    - Include key result (success/failure)
    - Keep under 50 characters
    - Examples:
      - "Build Completed Successfully" (not "Command Finished")
      - "Tests Failed: 3 errors" (not "Test Run Done")
      - "Deployment Finished in 2m" (not "Deploy Command Completed")

    **Message:**
    - Key information first
    - Include timing if significant
    - Include error preview if failed
    - Include action items if failure
    - Keep concise (2-3 lines max)

    **Examples:**

    Success:
    Title: "Build Completed"
    Message: "cargo build --release finished successfully in 2m 15s. Binary ready at target/release/myapp."

    Failure:
    Title: "Build Failed"
    Message: "Compilation error in src/main.rs:42. Check output for details. Retry attempt 1 of 3."

    Return JSON: {"title": "...", "message": "..."}

    {{ ctx.output_format }}
  "#
}

/// Filter out noise from command output
///
/// Extracts the most important lines from verbose command output
/// for inclusion in notifications.
///
/// # Arguments
/// - full_output: Complete command output
/// - exit_code: Whether command succeeded
/// - max_lines: Maximum lines to include in summary
///
/// # Returns
/// String with filtered, most relevant output lines
function FilterRelevantOutput(
  full_output: string,
  exit_code: int,
  max_lines: int
) -> string {
  client AgentdFast
  prompt #"
    Extract the most relevant {{ max_lines }} lines from this command output:

    ```
    {{ full_output }}
    ```

    **Exit Code:** {{ exit_code }}

    **Filtering Guidelines:**

    **If Failed (exit_code != 0):**
    - Prioritize error messages
    - Include context around errors
    - Include the last few lines (often contain error summary)

    **If Succeeded (exit_code == 0):**
    - Prioritize summary lines
    - Include final status/completion messages
    - Skip verbose progress output

    **Always:**
    - Skip timestamp-only lines
    - Skip debug/trace level logs
    - Skip blank lines
    - Keep warning messages
    - Keep lines with numbers/statistics (e.g., "Compiled 42 files")

    Return the {{ max_lines }} most important lines, preserving their original text.
    Separate lines with newlines. If output is shorter than {{ max_lines }} lines, return all of it.

    {{ ctx.output_format }}
  "#
}

// ============================================================================
// Tests
// ============================================================================

test analyze_long_build_success {
  functions [AnalyzeShellEvent]
  args {
    command "cargo build --release"
    exit_code 0
    output "    Compiling myapp v0.1.0\n    Finished release [optimized] target(s) in 2m 15s"
    duration_ms 135000
    context "Directory: /projects/myapp, Time: 14:30"
  }
}

test analyze_quick_command {
  functions [AnalyzeShellEvent]
  args {
    command "ls -la"
    exit_code 0
    output "total 48\ndrwxr-xr-x  12 user  staff   384 Jan 15 14:00 ."
    duration_ms 15
    context "Directory: /projects, Time: 14:30"
  }
}

test analyze_failed_test {
  functions [AnalyzeShellEvent]
  args {
    command "cargo test"
    exit_code 101
    output "running 10 tests\ntest auth_test ... FAILED\ntest db_test ... ok\n\nfailures:\n    auth_test\n\ntest result: FAILED. 9 passed; 1 failed"
    duration_ms 5000
    context "Directory: /projects/myapp/tests, Time: 14:45"
  }
}

test learn_patterns_from_history {
  functions [LearnCommandPatterns]
  args {
    command_history [
      "cargo test | exit_code: 0 | duration: 4500ms | no notification",
      "cargo test | exit_code: 101 | duration: 4200ms | notified, user reviewed",
      "cargo build --release | exit_code: 0 | duration: 120000ms | notified, user acknowledged",
      "cargo build --release | exit_code: 0 | duration: 118000ms | notified, user acknowledged",
      "ls -la | exit_code: 0 | duration: 10ms | no notification",
      "git status | exit_code: 0 | duration: 50ms | no notification"
    ]
    notification_history [
      "cargo test FAILED → user immediately checked logs",
      "cargo build completed (2min) → user acknowledged but no action",
      "cargo build completed (2min) → user acknowledged but no action"
    ]
  }
}

test analyze_command_intent_build {
  functions [AnalyzeCommandIntent]
  args {
    command "npm run build"
    execution_context "Project: web-app, Directory: /projects/webapp, User actively developing"
  }
}

test analyze_command_intent_deploy {
  functions [AnalyzeCommandIntent]
  args {
    command "./deploy-production.sh"
    execution_context "Directory: /projects/myapp/scripts, Time: 22:30 (late night)"
  }
}

test generate_success_notification {
  functions [GenerateCompletionNotification]
  args {
    command "cargo test --all"
    exit_code 0
    duration_ms 8500
    output_summary "running 42 tests\ntest result: ok. 42 passed; 0 failed; 0 ignored"
    previous_attempts 0
  }
}

test generate_failure_notification {
  functions [GenerateCompletionNotification]
  args {
    command "cargo clippy --all-targets"
    exit_code 1
    duration_ms 3200
    output_summary "error: this function has too many arguments (8/7)\n  --> src/main.rs:42:1\nwarning: 5 warnings emitted\nerror: aborting due to previous error"
    previous_attempts 0
  }
}

test filter_error_output {
  functions [FilterRelevantOutput]
  args {
    full_output "Compiling myapp v0.1.0\nCompiling dep1 v1.0.0\nCompiling dep2 v2.0.0\nerror[E0425]: cannot find value `foo` in this scope\n  --> src/main.rs:42:5\n   |\n42 |     foo.bar()\n   |     ^^^ not found in this scope\n\nerror: aborting due to previous error\nFor more information about this error, try `rustc --explain E0425`"
    exit_code 1
    max_lines 5
  }
}

test filter_success_output {
  functions [FilterRelevantOutput]
  args {
    full_output "    Compiling dep1 v1.0.0\n    Compiling dep2 v2.0.0\n    Compiling dep3 v3.0.0\n    Compiling myapp v0.1.0\n    Finished release [optimized] target(s) in 2m 15s"
    exit_code 0
    max_lines 3
  }
}
