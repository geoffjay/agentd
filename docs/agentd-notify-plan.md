# Notification Service (agentd-notify) - Detailed Plan

## Overview

The Notification Service (agentd-notify) is a daemon that manages user notifications and collects responses for the agentd service ecosystem. It provides a JSON API for other services to send notifications and handles user interaction through system notifications or a custom menu bar application.

## Core Responsibilities

- Receive notification requests via HTTP API
- Display notifications to the user via system notifications or custom UI
- Collect and store user responses
- Provide API endpoints for polling/retrieving responses
- Maintain notification history and state
- Handle notification queuing and priority

## Technology Stack

### Common Components

- **Language**: Rust
- **Runtime**: Tokio async runtime
- **Database**: SQLite (via sqlx) for notification history
- **API Framework**: Axum for HTTP server
- **Configuration**: TOML format
- **Logging**: tracing + tracing-subscriber

### Approach 1: System Notifications (notify-rust)

```toml
[dependencies]
notify-rust = "4"          # Cross-platform system notifications
# On macOS: Uses NSUserNotificationCenter
# On Linux: Uses D-Bus (libnotify)
# On Windows: Uses Windows Toast API
```

**Capabilities:**

- Display notifications with title, body, icon
- Add action buttons (limited platform support)
- Handle button clicks
- Set notification urgency/priority
- Play notification sounds

**Limitations:**

- Action button support varies by platform
- Limited input types (mostly buttons)
- macOS Notification Center requires signed app for actions
- No complex forms or multi-step interactions

### Approach 2: Custom Menu Bar Application

```toml
[dependencies]
tao = "0.16"               # Cross-platform windowing
egui = "0.27"              # Immediate mode GUI
egui-notify = "0.14"       # Notification overlays
tray-icon = "0.14"         # System tray/menu bar
rfd = "0.14"               # Native file dialogs
```

**Capabilities:**

- Rich interactive forms
- Multiple input types (text, select, checkbox, radio)
- Custom styling and branding
- Better multi-step workflows
- More control over notification persistence
- Better visual feedback

**Limitations:**

- More complex to implement
- Requires app to be always running
- Higher resource usage
- Need to handle app lifecycle (minimize, quit, etc.)

## Recommended Hybrid Approach

Start with **notify-rust** for MVP, design with **custom UI** as future upgrade path:

1. **Phase 1**: Implement with notify-rust

   - Fast time-to-value
   - Leverage existing OS notification infrastructure
   - Works across platforms immediately
   - Simpler codebase

2. **Phase 2**: Add custom menu bar UI as opt-in
   - Config option: `notification.ui_mode = "system" | "menubar"`
   - Reuse API layer and notification models
   - Enhanced UX for power users
   - Side-by-side comparison of approaches

## Architecture

### Project Structure

```
agentd-notify/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, daemon management
│   ├── lib.rs               # Library exports
│   ├── config.rs            # Configuration handling
│   ├── db/
│   │   ├── mod.rs
│   │   ├── schema.rs        # SQL schema
│   │   └── models.rs        # Notification, Response models
│   ├── api/
│   │   ├── mod.rs
│   │   ├── server.rs        # HTTP API server (axum)
│   │   └── handlers.rs      # Route handlers
│   ├── notifier/
│   │   ├── mod.rs
│   │   ├── system.rs        # System notifications (notify-rust)
│   │   ├── menubar.rs       # Custom menu bar UI (future)
│   │   └── trait.rs         # Notifier trait for abstraction
│   └── daemon/
│       ├── mod.rs
│       └── service.rs       # Service management (systemd/launchd)
```

## Data Models

### Database Schema

#### notifications

```sql
CREATE TABLE notifications (
    id TEXT PRIMARY KEY,                    -- UUID
    source_service TEXT NOT NULL,           -- e.g., "agentd-ask"
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    notification_type TEXT NOT NULL,        -- question, info, warning, error
    urgency TEXT DEFAULT 'normal',          -- low, normal, high, critical
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    displayed_at TIMESTAMP,
    responded_at TIMESTAMP,
    expires_at TIMESTAMP,
    metadata TEXT                           -- JSON string
);
```

#### notification_actions

```sql
CREATE TABLE notification_actions (
    id TEXT PRIMARY KEY,                    -- UUID
    notification_id TEXT NOT NULL,
    action_id TEXT NOT NULL,                -- "select-project", "confirm", etc.
    label TEXT NOT NULL,
    action_type TEXT NOT NULL,              -- button, select, input, checkbox
    action_data TEXT,                       -- JSON: options, defaults, validation
    sort_order INTEGER DEFAULT 0,
    FOREIGN KEY (notification_id) REFERENCES notifications(id)
);
```

#### notification_responses

```sql
CREATE TABLE notification_responses (
    id TEXT PRIMARY KEY,                    -- UUID
    notification_id TEXT NOT NULL,
    action_id TEXT NOT NULL,                -- Which action was taken
    response_value TEXT,                    -- User's response
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (notification_id) REFERENCES notifications(id)
);
```

### Rust Data Models

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    Question,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Urgency {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,                          // UUID
    pub source_service: String,
    pub title: String,
    pub message: String,
    pub notification_type: NotificationType,
    pub urgency: Urgency,
    pub actions: Vec<NotificationAction>,
    pub metadata: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    Button,        // Simple button click
    Select,        // Dropdown/list selection
    Input,         // Text input
    Checkbox,      // Boolean checkbox
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAction {
    pub id: String,
    pub action_id: String,      // Semantic ID like "select-project"
    pub label: String,
    pub action_type: ActionType,
    pub action_data: Option<ActionData>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionData {
    pub options: Option<Vec<SelectOption>>,   // For Select type
    pub default_value: Option<String>,
    pub placeholder: Option<String>,
    pub validation: Option<ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub required: bool,
    pub pattern: Option<String>,    // Regex pattern
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationResponse {
    pub id: String,
    pub notification_id: String,
    pub action_id: String,
    pub response_value: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

## Configuration

### File Locations

Using `directories` crate for platform-specific paths:

**Linux:**

- Config: `~/.config/agentd-notify/config.toml`
- Data: `~/.local/share/agentd-notify/notify.db`
- Logs: `~/.local/share/agentd-notify/notify.log`
- PID: `~/.local/share/agentd-notify/notify.pid`

**macOS:**

- Config: `~/Library/Application Support/agentd-notify/config.toml`
- Data: `~/Library/Application Support/agentd-notify/notify.db`
- Logs: `~/Library/Application Support/agentd-notify/notify.log`

**Windows:**

- Config: `C:\Users\<User>\AppData\Roaming\agentd-notify\config.toml`
- Data: `C:\Users\<User>\AppData\Roaming\agentd-notify\notify.db`

### config.toml

```toml
[server]
host = "127.0.0.1"
port = 8080

[notifications]
ui_mode = "system"              # "system" or "menubar" (future)
default_urgency = "normal"
default_timeout_seconds = 30
enable_sounds = true
max_queue_size = 50

[database]
# Auto-set to platform-specific path
# path = "~/.local/share/agentd-notify/notify.db"

[cleanup]
# Remove notifications older than this
retain_days = 30
# Run cleanup every N hours
cleanup_interval_hours = 24
```

## HTTP API

The service runs on `127.0.0.1:8080` (configurable).

### Endpoints

#### POST /notifications

Create and display a notification.

**Request Body:**

```json
{
  "source_service": "agentd-ask",
  "title": "Start a Project?",
  "message": "No active projects. Would you like to start one?",
  "notification_type": "question",
  "urgency": "normal",
  "actions": [
    {
      "action_id": "select-project",
      "label": "Select Project",
      "action_type": "select",
      "action_data": {
        "options": [
          { "value": "project-a", "label": "Project A" },
          { "value": "project-b", "label": "Project B" }
        ]
      }
    },
    {
      "action_id": "dismiss",
      "label": "Not Now",
      "action_type": "button"
    }
  ],
  "metadata": {
    "context": "poll-cycle",
    "poll_id": "123"
  },
  "expires_at": "2024-11-03T15:30:00Z"
}
```

**Response:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "created_at": "2024-11-03T14:30:00Z",
  "status": "displayed"
}
```

#### GET /notifications/:id

Get notification details and status.

**Response:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "source_service": "agentd-ask",
  "title": "Start a Project?",
  "message": "No active projects. Would you like to start one?",
  "notification_type": "question",
  "created_at": "2024-11-03T14:30:00Z",
  "displayed_at": "2024-11-03T14:30:01Z",
  "responded_at": "2024-11-03T14:30:15Z",
  "response": {
    "id": "660e8400-e29b-41d4-a716-446655440000",
    "action_id": "select-project",
    "response_value": "project-a",
    "created_at": "2024-11-03T14:30:15Z"
  }
}
```

#### GET /notifications/:id/response

Poll for notification response (blocks until response or timeout).

**Query Parameters:**

- `timeout`: Seconds to wait (default: 30, max: 300)

**Response (200 OK):**

```json
{
  "id": "660e8400-e29b-41d4-a716-446655440000",
  "notification_id": "550e8400-e29b-41d4-a716-446655440000",
  "action_id": "select-project",
  "response_value": "project-a",
  "created_at": "2024-11-03T14:30:15Z"
}
```

**Response (204 No Content):**
User has not responded yet (after timeout).

#### GET /notifications

List recent notifications.

**Query Parameters:**

- `limit`: Max results (default: 50)
- `source_service`: Filter by source
- `responded`: Filter by response status (true/false)

**Response:**

```json
{
  "notifications": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "Start a Project?",
      "created_at": "2024-11-03T14:30:00Z",
      "responded": true
    }
  ],
  "total": 1
}
```

#### GET /health

Health check endpoint.

**Response:**

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

## Notifier Trait (Abstraction)

To support both system notifications and future custom UI:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait Notifier: Send + Sync {
    /// Display a notification and wait for user response
    async fn notify(&self, notification: &Notification) -> Result<NotificationResponse>;

    /// Check if a notification is still displayed
    async fn is_displayed(&self, notification_id: &str) -> Result<bool>;

    /// Dismiss a notification programmatically
    async fn dismiss(&self, notification_id: &str) -> Result<()>;
}
```

### System Notifier Implementation (notify-rust)

```rust
pub struct SystemNotifier {
    db: Pool<Sqlite>,
}

#[async_trait]
impl Notifier for SystemNotifier {
    async fn notify(&self, notification: &Notification) -> Result<NotificationResponse> {
        // Store notification in database
        self.store_notification(notification).await?;

        // Display using notify-rust
        let mut n = notify_rust::Notification::new();
        n.summary(&notification.title)
         .body(&notification.message)
         .urgency(map_urgency(&notification.urgency));

        // Add action buttons (limited support)
        for action in &notification.actions {
            if action.action_type == ActionType::Button {
                n.action(&action.action_id, &action.label);
            }
        }

        // For select/input actions, show system dialog or use buttons
        n.show()?;

        // Wait for response (using database polling)
        self.wait_for_response(&notification.id, 30).await
    }
}
```

**Challenges with notify-rust:**

- Action button callback handling varies by platform
- Select/Input types require fallback to separate dialog
- Need to poll database for responses when using dialogs

### Menu Bar Notifier Implementation (Future)

```rust
pub struct MenuBarNotifier {
    db: Pool<Sqlite>,
    event_tx: mpsc::Sender<NotificationEvent>,
}

#[async_trait]
impl Notifier for MenuBarNotifier {
    async fn notify(&self, notification: &Notification) -> Result<NotificationResponse> {
        // Store notification
        self.store_notification(notification).await?;

        // Send to UI thread
        self.event_tx.send(NotificationEvent::Show(notification.clone())).await?;

        // Wait for response from UI
        self.wait_for_response(&notification.id, 30).await
    }
}
```

## Daemon Management

### Linux (systemd)

Service file: `~/.config/systemd/user/agentd-notify.service`

```ini
[Unit]
Description=Agentd Notification Service
After=network.target

[Service]
Type=simple
ExecStart=/path/to/agentd-notify daemon
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target
```

### macOS (launchd)

Plist: `~/Library/LaunchAgents/com.agentd.notify.plist`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.agentd.notify</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/agentd-notify</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
```

## Workflows

### Basic Notification Flow

1. **Service sends notification**

   - POST to `/notifications` with notification details
   - Service responds with notification ID

2. **agentd-notify processes**

   - Stores notification in database
   - Displays via system notifier
   - Sets `displayed_at` timestamp

3. **User interacts**

   - Clicks button or selects option
   - Response captured and stored in database
   - Sets `responded_at` timestamp

4. **Service polls for response**
   - GET `/notifications/:id/response` with timeout
   - Returns response when available
   - Or times out with 204 No Content

### Handling Complex Actions (Select/Input)

For `notify-rust` (Phase 1), handle non-button actions:

**Option A: Fallback to native dialogs**

```rust
// For Select type, show native picker
if action.action_type == ActionType::Select {
    let options: Vec<&str> = action.data.options.iter()
        .map(|o| o.label.as_str())
        .collect();

    // Use rfd or native-dialog crate
    let result = native_dialog::MessageDialog::new()
        .set_title(&notification.title)
        .set_text(&notification.message)
        .show_alert()?;
}
```

**Option B: URL callback**

```rust
// Open browser with form
n.action("respond", "Respond")
 .callback_url(&format!("http://localhost:8080/ui/respond/{}", notification.id));
```

**Option C: Multiple buttons**

```rust
// For small select lists, create button per option
for option in &action.options {
    n.action(&option.value, &option.label);
}
```

### Notification Expiration

Background task runs periodically:

```rust
async fn cleanup_expired_notifications(db: &Pool<Sqlite>) -> Result<()> {
    sqlx::query!(
        "UPDATE notifications
         SET responded_at = CURRENT_TIMESTAMP
         WHERE expires_at < CURRENT_TIMESTAMP
         AND responded_at IS NULL"
    )
    .execute(db)
    .await?;

    Ok(())
}
```

## Implementation Phases

### Phase 1: MVP with System Notifications

- [ ] Database schema and migrations
- [ ] Configuration loading
- [ ] HTTP API server (axum)
- [ ] Basic notification storage
- [ ] System notifications via notify-rust
- [ ] Button action handling
- [ ] Response polling endpoint
- [ ] Basic fallback for select/input (dialogs or buttons)
- [ ] Daemon management (systemd/launchd)

### Phase 2: Enhanced System Notifications

- [ ] Better fallback UI for complex actions
- [ ] Notification history UI (CLI)
- [ ] Notification expiration handling
- [ ] Queue management
- [ ] Priority/urgency handling
- [ ] Sound customization

### Phase 3: Custom Menu Bar UI (Future)

- [ ] Tray icon with menu
- [ ] Custom notification overlay
- [ ] Rich input forms
- [ ] Config option to switch modes
- [ ] Migration path from system to menubar

## Platform-Specific Considerations

### macOS

- **Notification Center**: Requires signed app for action buttons
- **Permission**: User must grant notification permission
- **Sound**: Custom sounds need to be bundled
- **Menu Bar**: Native look and feel with `tray-icon`

### Linux

- **D-Bus**: Depends on notification daemon (dunst, mako, etc.)
- **Action Support**: Varies by daemon
- **Desktop Files**: May need .desktop file for proper display
- **Wayland vs X11**: Ensure compatibility

### Windows

- **Toast Notifications**: Good action button support
- **Focus Assist**: Notifications may be suppressed
- **Tray Icon**: Different conventions than macOS/Linux

## Security Considerations

1. **API Access**: Localhost only (127.0.0.1)
2. **Database**: User-only file permissions
3. **Notification Content**: Sanitize for XSS if using HTML
4. **Response Validation**: Validate against action definitions
5. **Rate Limiting**: Prevent notification spam

## Testing Strategy

### Unit Tests

- Notification model validation
- Action data parsing
- Response validation
- Database operations

### Integration Tests

- Full HTTP API endpoints
- Notification creation → display → response flow
- Expiration handling
- Cleanup routines

### Manual Tests

- Visual appearance on each platform
- Action button handling
- Sound playback
- Permission handling

## Success Metrics

1. **Response Time**: < 100ms API response time
2. **Reliability**: Notifications always displayed
3. **Compatibility**: Works on macOS, Linux, Windows
4. **Usability**: Clear, actionable notifications
5. **Performance**: Low resource usage when idle

## Open Questions

1. Should notification history be accessible via CLI or just API?
2. How to handle notification sounds per-notification?
3. Should there be a "do not disturb" mode?
4. How to handle notifications when user is not logged in?
5. Should responses be encrypted in database?

## Future Enhancements

### Rich Notification Types

- Progress notifications (for long-running tasks)
- Interactive checklists
- Inline code snippets
- Embedded terminal output

### Smart Notifications

- Learn optimal notification times
- Group related notifications
- Deduplicate similar notifications
- Smart expiration based on context

### Integration

- Integration with calendar (time-aware)
- Integration with system focus modes
- Slack/Discord forwarding for remote work
- Mobile app companion

## Conclusion

The agentd-notify service provides a critical foundation for human-in-the-loop agent workflows by:

- Providing a simple JSON API for notification management
- Handling cross-platform notification display
- Collecting and storing user responses
- Supporting progressive enhancement (system → custom UI)

Starting with system notifications (notify-rust) provides fast time-to-value while maintaining flexibility to add a custom menu bar UI in the future.
