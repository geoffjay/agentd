//! Notification service command implementations.
//!
//! This module implements all subcommands for managing notifications via the REST API.
//! Commands include creating, listing, viewing, responding to, and deleting notifications.
//!
//! # Available Commands
//!
//! - **create**: Create a new notification with specified properties
//! - **list**: List all notifications, optionally filtered by status
//! - **get**: Retrieve detailed information about a specific notification
//! - **respond**: Provide a response to a notification
//! - **delete**: Remove a notification from the system
//!
//! # Examples
//!
//! ## Create a high-priority notification
//!
//! ```bash
//! agentd notify create \
//!   --title "Build Failed" \
//!   --message "Tests failed on main branch" \
//!   --priority high \
//!   --requires-response
//! ```
//!
//! ## List actionable notifications
//!
//! ```bash
//! agentd notify list --actionable
//! ```
//!
//! ## Respond to a notification
//!
//! ```bash
//! agentd notify respond <notification-id> "I've fixed the failing tests"
//! ```
//!
//! # Output Formatting
//!
//! - **list**: Displays a formatted table with colored priority and status
//! - **get/create/respond**: Shows detailed notification information with separators
//! - **delete**: Confirms deletion with the notification ID
//!
//! All output uses colored terminal formatting for better readability.

use crate::types::*;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Subcommand;
use colored::*;
use notify::client::NotifyClient;
use prettytable::{format, Cell, Row, Table};
use uuid::Uuid;

/// Notification management subcommands.
///
/// Each variant corresponds to a specific operation on notifications. All commands
/// communicate with the notification service REST API on port 7004.
#[derive(Subcommand)]
pub enum NotifyCommand {
    /// Create a new notification via the REST API.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Create a system notification with default settings
    /// agentd notify create \
    ///   --title "Build Complete" \
    ///   --message "All tests passed"
    ///
    /// # Create an urgent, ephemeral notification requiring response
    /// agentd notify create \
    ///   --title "Production Alert" \
    ///   --message "Service is down" \
    ///   --priority urgent \
    ///   --lifetime ephemeral \
    ///   --expires-in 600 \
    ///   --requires-response
    /// ```
    Create {
        /// Source type: system, ask, monitor, or hook
        #[arg(short, long, default_value = "system")]
        source: String,

        /// Lifetime type: ephemeral (expires) or persistent (until dismissed)
        #[arg(short, long, default_value = "persistent")]
        lifetime: String,

        /// Expiration time in seconds (only used with --lifetime ephemeral)
        #[arg(short, long, default_value = "3600")]
        expires_in: i64,

        /// Priority level: low, normal, high, or urgent
        #[arg(short, long, default_value = "normal")]
        priority: String,

        /// Short, descriptive notification title
        #[arg(short, long)]
        title: String,

        /// Detailed message content
        #[arg(short, long)]
        message: String,

        /// Whether the notification requires a user response
        #[arg(short, long, default_value = "false")]
        requires_response: bool,
    },

    /// List all notifications with optional filtering.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # List all notifications
    /// agentd notify list
    ///
    /// # List only pending notifications
    /// agentd notify list --status pending
    ///
    /// # List only actionable notifications (pending with required response)
    /// agentd notify list --actionable
    /// ```
    List {
        /// Filter by status: pending, viewed, dismissed, responded, or expired
        #[arg(short, long)]
        status: Option<String>,

        /// Show only actionable notifications (pending and require response)
        #[arg(short, long, default_value = "false")]
        actionable: bool,
    },

    /// Get the total count of notifications.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Get total count
    /// agent notify count
    ///
    /// # Shows total and breakdown by status
    /// ```
    Count,

    /// Delete a notification by its UUID.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd notify delete 550e8400-e29b-41d4-a716-446655440000
    /// ```
    Delete {
        /// Notification UUID (can use short form like first 8 characters)
        id: String,
    },

    /// Respond to a notification with text.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd notify respond 550e8400-e29b-41d4-a716-446655440000 \
    ///   "I've deployed the fix to production"
    /// ```
    Respond {
        /// Notification UUID to respond to
        id: String,

        /// Response text (can be multiple words)
        response: String,
    },

    /// Get detailed information about a specific notification.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd notify get 550e8400-e29b-41d4-a716-446655440000
    /// ```
    Get {
        /// Notification UUID to retrieve
        id: String,
    },
}

impl NotifyCommand {
    /// Execute the notification command by dispatching to the appropriate handler.
    ///
    /// # Arguments
    ///
    /// * `client` - The notification service client
    /// * `json` - If true, output raw JSON instead of formatted text
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the command fails.
    pub async fn execute(&self, client: &NotifyClient, json: bool) -> Result<()> {
        match self {
            NotifyCommand::Create {
                source,
                lifetime,
                expires_in,
                priority,
                title,
                message,
                requires_response,
            } => {
                create_notification(
                    client,
                    source,
                    lifetime,
                    *expires_in,
                    priority,
                    title,
                    message,
                    *requires_response,
                    json,
                )
                .await
            }
            NotifyCommand::List { status, actionable } => {
                list_notifications(client, status.as_deref(), *actionable, json).await
            }
            NotifyCommand::Count => count_notifications(client, json).await,
            NotifyCommand::Delete { id } => delete_notification(client, id, json).await,
            NotifyCommand::Respond { id, response } => {
                respond_to_notification(client, id, response, json).await
            }
            NotifyCommand::Get { id } => get_notification(client, id, json).await,
        }
    }
}

/// Create a new notification via POST request to the API.
///
/// This function parses command-line string arguments into typed values,
/// constructs a `CreateNotificationRequest`, and sends it to the API.
///
/// # Arguments
///
/// * `client` - Notification service client
/// * `source` - Source type as string ("system", "ask", "monitor", "hook")
/// * `lifetime` - Lifetime type as string ("ephemeral", "persistent")
/// * `expires_in` - Seconds until expiration (for ephemeral notifications)
/// * `priority` - Priority level as string ("low", "normal", "high", "urgent")
/// * `title` - Notification title
/// * `message` - Notification message content
/// * `requires_response` - Whether notification needs a response
///
/// # Returns
///
/// Returns `Ok(())` on success after displaying the created notification.
///
/// # Errors
///
/// Returns an error if:
/// - Invalid source, lifetime, or priority string
/// - Network request fails
/// - API returns an error
#[allow(clippy::too_many_arguments)]
async fn create_notification(
    client: &NotifyClient,
    source: &str,
    lifetime: &str,
    expires_in: i64,
    priority: &str,
    title: &str,
    message: &str,
    requires_response: bool,
    json: bool,
) -> Result<()> {
    // Parse source
    let source = match source.to_lowercase().as_str() {
        "system" => NotificationSource::System,
        "ask" => NotificationSource::AskService { request_id: Uuid::new_v4() },
        "monitor" => NotificationSource::MonitorService { alert_type: "cli".to_string() },
        "hook" => NotificationSource::AgentHook {
            agent_id: "cli".to_string(),
            hook_type: "manual".to_string(),
        },
        _ => anyhow::bail!("Invalid source type: {source}"),
    };

    // Parse lifetime
    let lifetime = match lifetime.to_lowercase().as_str() {
        "ephemeral" => NotificationLifetime::Ephemeral {
            expires_at: Utc::now() + Duration::seconds(expires_in),
        },
        "persistent" => NotificationLifetime::Persistent,
        _ => anyhow::bail!("Invalid lifetime type: {lifetime}"),
    };

    // Parse priority
    let priority: NotificationPriority = priority.parse().context("Invalid priority level")?;

    let request = CreateNotificationRequest {
        source,
        lifetime,
        priority,
        title: title.to_string(),
        message: message.to_string(),
        requires_response,
    };

    let notification =
        client.create_notification(&request).await.context("Failed to create notification")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&notification)?);
    } else {
        println!("{}", "Notification created successfully!".green().bold());
        println!();
        display_notification(&notification);
    }

    Ok(())
}

/// List notifications with optional filtering.
///
/// Fetches notifications from the API and displays them in a formatted table.
/// Supports filtering by status or showing only actionable notifications.
///
/// # Arguments
///
/// * `client` - Notification service client
/// * `status` - Optional status filter ("pending", "viewed", etc.)
/// * `actionable` - If true, only show actionable notifications (pending + requires response)
///
/// # Returns
///
/// Returns `Ok(())` on success after displaying the notification list.
///
/// # Errors
///
/// Returns an error if the network request fails or API returns an error.
async fn list_notifications(
    client: &NotifyClient,
    status: Option<&str>,
    actionable: bool,
    json: bool,
) -> Result<()> {
    let notifications = if actionable {
        client
            .list_actionable_notifications()
            .await
            .context("Failed to fetch actionable notifications")?
    } else if let Some(status) = status {
        let status_enum =
            status.parse::<NotificationStatus>().context(format!("Invalid status: {status}"))?;
        client
            .list_notifications_by_status(status_enum)
            .await
            .context("Failed to fetch notifications")?
    } else {
        client.list_notifications().await.context("Failed to fetch notifications")?
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&notifications)?);
        return Ok(());
    }

    if notifications.is_empty() {
        println!("{}", "No notifications found.".yellow());
        return Ok(());
    }

    println!("{}", format!("Found {} notification(s)", notifications.len()).cyan().bold());
    println!();

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_BOX_CHARS);
    table.set_titles(Row::new(vec![
        Cell::new("ID").style_spec("Fb"),
        Cell::new("Priority").style_spec("Fb"),
        Cell::new("Status").style_spec("Fb"),
        Cell::new("Title").style_spec("Fb"),
        Cell::new("Created").style_spec("Fb"),
        Cell::new("Requires Response").style_spec("Fb"),
    ]));

    for notification in notifications {
        let priority_text = format_priority_plain(notification.priority);
        let priority_style = get_priority_style(notification.priority);

        let status_text = format_status_plain(notification.status);
        let status_style = get_status_style(notification.status);

        let created = notification.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let response_text = if notification.requires_response { "Yes" } else { "No" };
        let response_style = if notification.requires_response { "Fg" } else { "" };

        table.add_row(Row::new(vec![
            Cell::new(&notification.id.to_string()),
            Cell::new(priority_text).style_spec(priority_style),
            Cell::new(status_text).style_spec(status_style),
            Cell::new(&notification.title),
            Cell::new(&created),
            Cell::new(response_text).style_spec(response_style),
        ]));
    }

    table.printstd();

    Ok(())
}

/// Get and display notification counts grouped by status.
///
/// Fetches notification statistics from the API and displays the total count
/// along with a breakdown by status.
///
/// # Arguments
///
/// * `client` - Notification service client
///
/// # Returns
///
/// Returns `Ok(())` on success after displaying the counts.
///
/// # Errors
///
/// Returns an error if the network request fails or API returns an error.
async fn count_notifications(client: &NotifyClient, json: bool) -> Result<()> {
    let counts =
        client.count_notifications().await.context("Failed to fetch notification counts")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&counts)?);
        return Ok(());
    }

    println!("{}", format!("Total: {}", counts.total).cyan().bold());
    println!();

    if !counts.by_status.is_empty() {
        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_BOX_CHARS);
        table.set_titles(Row::new(vec![
            Cell::new("Status").style_spec("Fb"),
            Cell::new("Count").style_spec("Fb"),
        ]));

        for status_count in counts.by_status {
            let status_style = match status_count.status.as_str() {
                "pending" => "Fy",   // yellow
                "viewed" => "Fc",    // cyan
                "responded" => "Fg", // green
                "dismissed" => "Fd", // dim
                "expired" => "Fr",   // red
                _ => "",
            };

            table.add_row(Row::new(vec![
                Cell::new(&status_count.status).style_spec(status_style),
                Cell::new(&status_count.count.to_string()),
            ]));
        }

        table.printstd();
    }

    Ok(())
}

/// Get and display a specific notification by ID.
///
/// # Arguments
///
/// * `client` - Notification service client
/// * `id` - Notification UUID as string
///
/// # Returns
///
/// Returns `Ok(())` on success after displaying the notification details.
///
/// # Errors
///
/// Returns an error if:
/// - The ID is not a valid UUID
/// - The network request fails
/// - The notification is not found
async fn get_notification(client: &NotifyClient, id: &str, json: bool) -> Result<()> {
    let uuid = Uuid::parse_str(id).context("Invalid UUID format")?;
    let notification =
        client.get_notification(uuid).await.context("Failed to fetch notification")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&notification)?);
    } else {
        display_notification(&notification);
    }

    Ok(())
}

/// Delete a notification by ID.
///
/// # Arguments
///
/// * `client` - Notification service client
/// * `id` - Notification UUID as string
///
/// # Returns
///
/// Returns `Ok(())` on success after confirming deletion.
///
/// # Errors
///
/// Returns an error if:
/// - The ID is not a valid UUID
/// - The network request fails
/// - The notification is not found
async fn delete_notification(client: &NotifyClient, id: &str, json: bool) -> Result<()> {
    let uuid = Uuid::parse_str(id).context("Invalid UUID format")?;

    client.delete_notification(uuid).await.context("Failed to delete notification")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "message": format!("Notification {uuid} deleted successfully"),
                "id": uuid
            }))?
        );
    } else {
        println!("{}", format!("Notification {uuid} deleted successfully!").green().bold());
    }

    Ok(())
}

/// Submit a response to a notification.
///
/// Sends a PUT request with the response text, which updates the notification
/// status to `Responded` and stores the response.
///
/// # Arguments
///
/// * `client` - Notification service client
/// * `id` - Notification UUID as string
/// * `response` - User's response text
///
/// # Returns
///
/// Returns `Ok(())` on success after displaying the updated notification.
///
/// # Errors
///
/// Returns an error if:
/// - The ID is not a valid UUID
/// - The network request fails
/// - The notification is not found
/// - The notification doesn't accept responses
async fn respond_to_notification(
    client: &NotifyClient,
    id: &str,
    response: &str,
    json: bool,
) -> Result<()> {
    let uuid = Uuid::parse_str(id).context("Invalid UUID format")?;

    let request = UpdateNotificationRequest { status: None, response: Some(response.to_string()) };

    let notification = client
        .update_notification(uuid, &request)
        .await
        .context("Failed to respond to notification")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&notification)?);
    } else {
        println!("{}", "Response submitted successfully!".green().bold());
        println!();
        display_notification(&notification);
    }

    Ok(())
}

/// Display a notification with formatted, colored output.
///
/// Shows all notification fields with visual separators and color-coded
/// priority and status indicators.
///
/// # Arguments
///
/// * `notification` - The notification to display
fn display_notification(notification: &Notification) {
    println!("{}", "═".repeat(80).cyan());
    println!("{}: {}", "ID".bold(), notification.id.to_string().bright_black());
    println!("{}: {}", "Title".bold(), notification.title.bright_white().bold());
    println!("{}: {}", "Message".bold(), notification.message);
    println!("{}: {}", "Priority".bold(), format_priority(notification.priority));
    println!("{}: {}", "Status".bold(), format_status(notification.status));
    println!("{}: {}", "Created".bold(), notification.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("{}: {}", "Updated".bold(), notification.updated_at.format("%Y-%m-%d %H:%M:%S"));
    println!(
        "{}: {}",
        "Requires Response".bold(),
        if notification.requires_response { "Yes".green() } else { "No".yellow() }
    );

    if let Some(ref response) = notification.response {
        println!("{}: {}", "Response".bold(), response.bright_cyan());
    }

    match &notification.lifetime {
        NotificationLifetime::Ephemeral { expires_at } => {
            println!(
                "{}: {} ({})",
                "Lifetime".bold(),
                "Ephemeral".yellow(),
                expires_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
        NotificationLifetime::Persistent => {
            println!("{}: {}", "Lifetime".bold(), "Persistent".green());
        }
    }

    println!("{}", "═".repeat(80).cyan());
}

/// Format a notification priority with appropriate color coding.
///
/// Returns a colored string representation of the priority level:
/// - Low: gray
/// - Normal: white
/// - High: yellow
/// - Urgent: bold red
///
/// # Arguments
///
/// * `priority` - The priority level to format
///
/// # Returns
///
/// Returns a colored string ready for terminal output.
fn format_priority(priority: NotificationPriority) -> String {
    match priority {
        NotificationPriority::Low => "Low".bright_black().to_string(),
        NotificationPriority::Normal => "Normal".white().to_string(),
        NotificationPriority::High => "High".yellow().to_string(),
        NotificationPriority::Urgent => "Urgent".red().bold().to_string(),
    }
}

/// Format a notification status with appropriate color coding.
///
/// Returns a colored string representation of the status:
/// - Pending: yellow
/// - Viewed: cyan
/// - Responded: green
/// - Dismissed: gray
/// - Expired: red
///
/// # Arguments
///
/// * `status` - The notification status to format
///
/// # Returns
///
/// Returns a colored string ready for terminal output.
fn format_status(status: NotificationStatus) -> String {
    match status {
        NotificationStatus::Pending => "Pending".yellow().to_string(),
        NotificationStatus::Viewed => "Viewed".cyan().to_string(),
        NotificationStatus::Responded => "Responded".green().to_string(),
        NotificationStatus::Dismissed => "Dismissed".bright_black().to_string(),
        NotificationStatus::Expired => "Expired".red().to_string(),
    }
}

/// Format a notification priority as plain text (no colors) for table display.
///
/// Returns an unformatted string representation of the priority level.
/// Used in table cells to avoid ANSI escape code width calculation issues.
///
/// # Arguments
///
/// * `priority` - The priority level to format
///
/// # Returns
///
/// Returns a plain string representation of the priority.
fn format_priority_plain(priority: NotificationPriority) -> &'static str {
    match priority {
        NotificationPriority::Low => "Low",
        NotificationPriority::Normal => "Normal",
        NotificationPriority::High => "High",
        NotificationPriority::Urgent => "Urgent",
    }
}

/// Format a notification status as plain text (no colors) for table display.
///
/// Returns an unformatted string representation of the status.
/// Used in table cells to avoid ANSI escape code width calculation issues.
///
/// # Arguments
///
/// * `status` - The notification status to format
///
/// # Returns
///
/// Returns a plain string representation of the status.
fn format_status_plain(status: NotificationStatus) -> &'static str {
    match status {
        NotificationStatus::Pending => "Pending",
        NotificationStatus::Viewed => "Viewed",
        NotificationStatus::Responded => "Responded",
        NotificationStatus::Dismissed => "Dismissed",
        NotificationStatus::Expired => "Expired",
    }
}

/// Get the prettytable style spec for a notification priority.
///
/// Returns a style specification string that prettytable uses to apply colors:
/// - Low: dim (Fd)
/// - Normal: default (empty string)
/// - High: yellow (Fy)
/// - Urgent: bold red (Frb)
///
/// # Arguments
///
/// * `priority` - The priority level
///
/// # Returns
///
/// Returns a style spec string for prettytable.
fn get_priority_style(priority: NotificationPriority) -> &'static str {
    match priority {
        NotificationPriority::Low => "Fd",     // dim/dark
        NotificationPriority::Normal => "",    // default
        NotificationPriority::High => "Fy",    // yellow
        NotificationPriority::Urgent => "Frb", // red bold
    }
}

/// Get the prettytable style spec for a notification status.
///
/// Returns a style specification string that prettytable uses to apply colors:
/// - Pending: yellow (Fy)
/// - Viewed: cyan (Fc)
/// - Responded: green (Fg)
/// - Dismissed: dim (Fd)
/// - Expired: red (Fr)
///
/// # Arguments
///
/// * `status` - The notification status
///
/// # Returns
///
/// Returns a style spec string for prettytable.
fn get_status_style(status: NotificationStatus) -> &'static str {
    match status {
        NotificationStatus::Pending => "Fy",   // yellow
        NotificationStatus::Viewed => "Fc",    // cyan
        NotificationStatus::Responded => "Fg", // green
        NotificationStatus::Dismissed => "Fd", // dim/dark
        NotificationStatus::Expired => "Fr",   // red
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_priority() {
        // Test that formatting functions don't panic and return non-empty strings
        let low = format_priority(NotificationPriority::Low);
        let normal = format_priority(NotificationPriority::Normal);
        let high = format_priority(NotificationPriority::High);
        let urgent = format_priority(NotificationPriority::Urgent);

        assert!(!low.is_empty());
        assert!(!normal.is_empty());
        assert!(!high.is_empty());
        assert!(!urgent.is_empty());

        // Verify the strings contain the expected text (ignoring color codes)
        assert!(low.contains("Low"));
        assert!(normal.contains("Normal"));
        assert!(high.contains("High"));
        assert!(urgent.contains("Urgent"));
    }

    #[test]
    fn test_format_status() {
        // Test that formatting functions don't panic and return non-empty strings
        let pending = format_status(NotificationStatus::Pending);
        let viewed = format_status(NotificationStatus::Viewed);
        let responded = format_status(NotificationStatus::Responded);
        let dismissed = format_status(NotificationStatus::Dismissed);
        let expired = format_status(NotificationStatus::Expired);

        assert!(!pending.is_empty());
        assert!(!viewed.is_empty());
        assert!(!responded.is_empty());
        assert!(!dismissed.is_empty());
        assert!(!expired.is_empty());

        // Verify the strings contain the expected text (ignoring color codes)
        assert!(pending.contains("Pending"));
        assert!(viewed.contains("Viewed"));
        assert!(responded.contains("Responded"));
        assert!(dismissed.contains("Dismissed"));
        assert!(expired.contains("Expired"));
    }

    #[test]
    fn test_display_notification_doesnt_panic() {
        // Create a test notification and verify display_notification doesn't panic
        let notification = Notification {
            id: Uuid::new_v4(),
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            status: NotificationStatus::Pending,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: false,
            response: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // This should not panic
        display_notification(&notification);
    }

    #[test]
    fn test_display_notification_with_response() {
        let notification = Notification {
            id: Uuid::new_v4(),
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::High,
            status: NotificationStatus::Responded,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: true,
            response: Some("My response".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // This should not panic
        display_notification(&notification);
    }

    #[test]
    fn test_display_notification_ephemeral() {
        let notification = Notification {
            id: Uuid::new_v4(),
            source: NotificationSource::AskService { request_id: Uuid::new_v4() },
            lifetime: NotificationLifetime::Ephemeral {
                expires_at: Utc::now() + Duration::hours(1),
            },
            priority: NotificationPriority::Urgent,
            status: NotificationStatus::Pending,
            title: "Urgent Test".to_string(),
            message: "This is urgent".to_string(),
            requires_response: true,
            response: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // This should not panic
        display_notification(&notification);
    }
}
