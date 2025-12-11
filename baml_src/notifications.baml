// Notification-related BAML functions for agentd
//
// This module provides AI-powered notification processing including:
// - Automatic categorization and prioritization
// - Notification summarization and digests
// - Smart filtering and grouping

// ============================================================================
// Data Models
// ============================================================================

/// Result of notification categorization
class NotificationCategory {
  /// Category type: "urgent", "info", "action_required", "reminder", "system", "error"
  category string

  /// Priority level: "low", "normal", "high", "urgent"
  priority string

  /// Suggested lifetime: "ephemeral" (auto-dismiss), "persistent" (keep until dismissed)
  suggested_lifetime string

  /// Brief explanation of the categorization decision
  reasoning string

  /// Recommended action for the user (if any)
  suggested_action string?
}

/// Summary of multiple notifications for digest generation
class NotificationDigest {
  /// High-level summary of notification activity
  summary string

  /// Key actions that require user attention
  key_actions string[]

  /// Count of notifications by priority
  urgent_count int
  high_count int
  normal_count int
  low_count int

  /// Distribution of notifications by category
  categories map<string, int>

  /// Observed trends or patterns in notification activity
  trends string

  /// Actionable recommendations for the user
  recommendations string[]
}

/// Notification grouping suggestion
class NotificationGroup {
  /// Group title/name
  title string

  /// IDs of notifications that should be grouped together
  notification_ids string[]

  /// Reason for grouping
  reasoning string

  /// Suggested group action (e.g., "mark all as read", "dismiss group")
  suggested_group_action string?
}

// ============================================================================
// Functions
// ============================================================================

/// Categorize a notification automatically based on its content
///
/// This function analyzes the notification title, message, and source context
/// to determine the appropriate category, priority, and lifetime.
///
/// # Arguments
/// - title: The notification title
/// - message: The notification message body
/// - source_context: Context about where the notification came from
///
/// # Returns
/// NotificationCategory with category, priority, lifetime, and reasoning
function CategorizeNotification(
  title: string,
  message: string,
  source_context: string
) -> NotificationCategory {
  client LocalOllama
  prompt #"
    Analyze this notification and determine its appropriate categorization:

    **Title:** {{ title }}

    **Message:** {{ message }}

    **Source:** {{ source_context }}

    Please categorize this notification by considering:

    **Category Guidelines:**
    - "urgent": System failures, security issues, critical errors requiring immediate attention
    - "action_required": User decisions needed, approvals, confirmations
    - "error": Non-critical errors, warnings, issues that should be addressed
    - "reminder": Time-based reminders, scheduled notifications
    - "system": System status updates, background task completions
    - "info": General information, FYI messages, status updates

    **Priority Guidelines:**
    - "urgent": Immediate attention required, system down, data loss risk
    - "high": Important but not critical, should be addressed soon
    - "normal": Standard notifications, can be addressed during normal workflow
    - "low": Informational only, no action required

    **Lifetime Guidelines:**
    - "ephemeral": Auto-dismiss notifications that are time-sensitive or informational
    - "persistent": Keep until user explicitly dismisses, for actionable items

    Provide your analysis in the structured format.

    {{ ctx.output_format }}
  "#
}

/// Generate a digest summarizing multiple notifications
///
/// Creates a comprehensive summary of notification activity over a time period,
/// highlighting key actions, trends, and recommendations.
///
/// # Arguments
/// - notifications: List of notification summaries (format: "title: message | source | priority")
/// - time_period: Description of the time period (e.g., "last 24 hours", "this week")
///
/// # Returns
/// NotificationDigest with summary, key actions, trends, and recommendations
function SummarizeNotifications(
  notifications: string[],
  time_period: string
) -> NotificationDigest {
  client AgentdPrimary
  prompt #"
    Summarize these notifications from {{ time_period }}:

    {% for notification in notifications %}
    {{ loop.index }}. {{ notification }}
    {% endfor %}

    Please provide:

    1. **Summary**: A high-level overview of notification activity during this period
    2. **Key Actions**: Specific actions the user should take (be concrete and actionable)
    3. **Counts**: Accurate counts of notifications by priority (urgent, high, normal, low)
    4. **Categories**: Distribution of notifications by type
    5. **Trends**: Any patterns or trends in the notification activity
    6. **Recommendations**: Specific suggestions for the user based on the activity

    Focus on actionable insights rather than just repeating what the notifications say.

    {{ ctx.output_format }}
  "#
}

/// Suggest intelligent grouping of related notifications
///
/// Analyzes multiple notifications to identify which ones should be grouped
/// together based on common themes, sources, or action items.
///
/// # Arguments
/// - notifications: List of notifications with their IDs and content
/// - min_group_size: Minimum number of notifications to form a group (default: 2)
///
/// # Returns
/// List of NotificationGroup objects with suggested groupings
function GroupRelatedNotifications(
  notifications: map<string, string>,
  min_group_size: int
) -> NotificationGroup[] {
  client LocalOllama
  prompt #"
    Analyze these notifications and suggest intelligent groupings:

    {{ notifications }}

    **Grouping Rules:**
    - Minimum group size: {{ min_group_size }} notifications
    - Group by common theme, source, or related actions
    - Each notification should belong to at most one group
    - Don't force groupings if notifications are unrelated

    **Example groupings:**
    - Multiple error notifications from the same service
    - Series of status updates for a long-running task
    - Related action items that can be handled together

    Return a list of suggested groups. Include only meaningful groupings.

    {{ ctx.output_format }}
  "#
}

/// Determine if a notification is still relevant
///
/// Analyzes a notification's content, age, and current system state to determine
/// if it should be automatically dismissed or archived.
///
/// # Arguments
/// - title: Notification title
/// - message: Notification message
/// - created_hours_ago: How many hours ago the notification was created
/// - current_system_state: Brief description of current system state
///
/// # Returns
/// Boolean indicating if the notification is still relevant and should be kept
function IsNotificationStillRelevant(
  title: string,
  message: string,
  created_hours_ago: int,
  current_system_state: string
) -> bool {
  client LocalOllama
  prompt #"
    Determine if this notification is still relevant:

    **Notification:**
    Title: {{ title }}
    Message: {{ message }}
    Created: {{ created_hours_ago }} hours ago

    **Current System State:**
    {{ current_system_state }}

    **Relevance Criteria:**
    - Time-sensitive notifications become irrelevant after the event/deadline passes
    - Error notifications become irrelevant if the error is resolved
    - Status update notifications become irrelevant once superseded by newer updates
    - Persistent action items remain relevant until explicitly completed

    Return true if the notification is still relevant and should be kept,
    false if it can be safely archived or dismissed.

    Answer with just 'true' or 'false'.
  "#
}

// ============================================================================
// Tests
// ============================================================================

test categorize_urgent_system_failure {
  functions [CategorizeNotification]
  args {
    title "Database Connection Lost"
    message "Unable to connect to PostgreSQL database. All services are affected."
    source_context "production monitoring system"
  }
}

test categorize_normal_task_completion {
  functions [CategorizeNotification]
  args {
    title "Backup Completed"
    message "Daily backup finished successfully. 2.3GB backed up to cloud storage."
    source_context "backup service"
  }
}

test categorize_action_required {
  functions [CategorizeNotification]
  args {
    title "Approval Needed"
    message "Please approve the deployment to production environment"
    source_context "deployment pipeline"
  }
}

test summarize_daily_notifications {
  functions [SummarizeNotifications]
  args {
    notifications [
      "Database backup completed | backup-service | normal",
      "API response time degraded | monitoring | high",
      "Deployment approved by admin | ci/cd | normal",
      "Certificate expiring in 7 days | security | high"
    ]
    time_period "today"
  }
}
