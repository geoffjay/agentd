//! Type definitions matching BAML function inputs and outputs.
//!
//! These types correspond to the BAML class definitions in baml_src/.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Notification Types (from notifications.baml)
// ============================================================================

/// Result of notification categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationCategory {
    /// Category type: "urgent", "info", "action_required", "reminder", "system", "error"
    pub category: String,
    /// Priority level: "low", "normal", "high", "urgent"
    pub priority: String,
    /// Suggested lifetime: "ephemeral" (auto-dismiss), "persistent" (keep until dismissed)
    pub suggested_lifetime: String,
    /// Brief explanation of the categorization decision
    pub reasoning: String,
    /// Recommended action for the user (if any)
    pub suggested_action: Option<String>,
}

/// Summary of multiple notifications for digest generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationDigest {
    /// High-level summary of notification activity
    pub summary: String,
    /// Key actions that require user attention
    pub key_actions: Vec<String>,
    /// Count of notifications by priority
    pub urgent_count: i32,
    pub high_count: i32,
    pub normal_count: i32,
    pub low_count: i32,
    /// Distribution of notifications by category
    pub categories: HashMap<String, i32>,
    /// Observed trends or patterns in notification activity
    pub trends: String,
    /// Actionable recommendations for the user
    pub recommendations: Vec<String>,
}

/// Notification grouping suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationGroup {
    /// Group title/name
    pub title: String,
    /// IDs of notifications that should be grouped together
    pub notification_ids: Vec<String>,
    /// Reason for grouping
    pub reasoning: String,
    /// Suggested group action (e.g., "mark all as read", "dismiss group")
    pub suggested_group_action: Option<String>,
}

// ============================================================================
// Question Types (from questions.baml)
// ============================================================================

/// Generated system question with context and suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemQuestion {
    /// The question text to present to the user
    pub question_text: String,
    /// Suggested responses (e.g., ["yes", "no", "later"], or specific options)
    pub suggested_responses: Vec<String>,
    /// Brief explanation of why this question is being asked
    pub reasoning: String,
    /// Urgency level: "low", "normal", "high", "urgent"
    pub urgency: String,
    /// Follow-up actions the system might take based on user response
    pub follow_up_actions: HashMap<String, String>,
    /// Additional context or help text for the user
    pub help_text: Option<String>,
}

/// Analysis of user's previous answer to improve future questions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerAnalysis {
    /// Interpreted meaning of the user's answer
    pub interpretation: String,
    /// Confidence in interpretation (0.0 - 1.0)
    pub confidence: f64,
    /// Suggested system action based on this answer
    pub suggested_action: String,
    /// Whether a follow-up question might be needed
    pub needs_followup: bool,
    /// Optional follow-up question if needed
    pub followup_question: Option<String>,
}

/// Question effectiveness feedback for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionFeedback {
    /// Whether the question was clear and actionable
    pub was_clear: bool,
    /// Whether the user found the question relevant
    pub was_relevant: bool,
    /// Suggestions for improving this question type
    pub improvement_suggestions: Vec<String>,
    /// Optimal time to ask this question (if timing was an issue)
    pub better_timing: Option<String>,
}

// ============================================================================
// Monitoring Types (from monitoring.baml)
// ============================================================================

/// Analysis results for log entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAnalysis {
    /// Whether errors were detected in the logs
    pub has_errors: bool,
    /// High-level summary of error patterns or issues
    pub error_summary: String,
    /// List of services or components affected by issues
    pub affected_services: Vec<String>,
    /// Specific, actionable steps to resolve issues
    pub suggested_actions: Vec<String>,
    /// Severity level: "info", "warning", "error", "critical"
    pub severity: String,
    /// Root cause analysis (if determinable from logs)
    pub root_cause: String,
    /// Whether immediate action is required
    pub requires_immediate_attention: bool,
    /// Estimated impact on system operations
    pub impact_assessment: String,
}

/// Pattern detection in logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPattern {
    /// Description of the detected pattern
    pub pattern_description: String,
    /// How many times this pattern appeared
    pub occurrence_count: i32,
    /// Time range when pattern was observed
    pub time_range: String,
    /// Whether this pattern indicates a problem
    pub is_problematic: bool,
    /// Confidence in pattern detection (0.0 - 1.0)
    pub confidence: f64,
    /// Suggested investigation steps
    pub investigation_steps: Vec<String>,
}

/// Health assessment of a service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    /// Service name
    pub service_name: String,
    /// Overall health status: "healthy", "degraded", "unhealthy", "critical"
    pub status: String,
    /// Specific health indicators and their values
    pub indicators: HashMap<String, String>,
    /// Issues detected (if any)
    pub issues: Vec<String>,
    /// Recommended remediation actions
    pub recommendations: Vec<String>,
    /// Confidence in health assessment (0.0 - 1.0)
    pub confidence: f64,
    /// Predicted impact if issues not addressed
    pub impact_if_unresolved: Option<String>,
}

/// Performance anomaly detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnomaly {
    /// Metric that shows anomalous behavior
    pub metric_name: String,
    /// Current value of the metric
    pub current_value: String,
    /// Expected/normal value range
    pub expected_range: String,
    /// Deviation from normal (percentage or description)
    pub deviation: String,
    /// Possible causes of the anomaly
    pub possible_causes: Vec<String>,
    /// Recommended investigation steps
    pub investigation_steps: Vec<String>,
    /// Severity of the anomaly
    pub severity: String,
}

// ============================================================================
// CLI Types (from cli.baml)
// ============================================================================

/// Parsed command intent from natural language input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandIntent {
    /// The identified action
    pub action: String,
    /// Extracted parameters for the command
    pub parameters: HashMap<String, String>,
    /// Confidence in the intent parsing (0.0 - 1.0)
    pub confidence: f64,
    /// Whether the command should ask for confirmation before executing
    pub requires_confirmation: bool,
    /// Human-readable explanation of what will be executed
    pub execution_summary: String,
    /// Warning messages if the command might have unintended consequences
    pub warnings: Option<Vec<String>>,
}

/// Suggestion for command completion or correction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSuggestion {
    /// Suggested command to run
    pub suggested_command: String,
    /// Explanation of what this command does
    pub explanation: String,
    /// Confidence that this is what the user intended
    pub confidence: f64,
    /// Alternative suggestions
    pub alternatives: Option<Vec<String>>,
}

/// Help information for natural language query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpResponse {
    /// Direct answer to the user's question
    pub answer: String,
    /// Relevant examples showing how to accomplish the task
    pub examples: Vec<String>,
    /// Related commands or topics
    pub related_topics: Vec<String>,
    /// Whether this requires additional context from user
    pub needs_more_info: bool,
    /// Follow-up questions to clarify user's intent
    pub followup_questions: Option<Vec<String>>,
}

// ============================================================================
// Hook Types (from hooks.baml)
// ============================================================================

/// Decision about whether and how to notify for a hook event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookAction {
    /// Whether a notification should be sent
    pub should_notify: bool,
    /// Notification title (if should_notify is true)
    pub notification_title: String,
    /// Notification message body (if should_notify is true)
    pub notification_message: String,
    /// Suggested priority: "low", "normal", "high", "urgent"
    pub priority: String,
    /// Brief explanation of the decision
    pub reasoning: String,
    /// Metadata to attach to the notification for context
    pub metadata: HashMap<String, String>,
    /// Whether this event indicates a problem
    pub indicates_problem: bool,
    /// Suggested actions for the user (if applicable)
    pub suggested_actions: Option<Vec<String>>,
}

/// Pattern learned from hook event history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPattern {
    /// Description of the pattern
    pub pattern_description: String,
    /// Commands or events that match this pattern
    pub matching_criteria: String,
    /// Whether events matching this pattern should notify
    pub should_notify: bool,
    /// Suggested notification priority for this pattern
    pub notification_priority: String,
    /// How many times this pattern has been observed
    pub occurrence_count: i32,
    /// Confidence in this pattern (0.0 - 1.0)
    pub confidence: f64,
}

/// Intelligence about command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInsight {
    /// What the command appears to be doing
    pub purpose: String,
    /// Whether this is a long-running command
    pub is_long_running: bool,
    /// Expected behavior (success indicators)
    pub expected_outcomes: Vec<String>,
    /// Potential issues to watch for
    pub potential_issues: Vec<String>,
    /// Whether user typically cares about this command's completion
    pub user_cares_about_completion: bool,
}
