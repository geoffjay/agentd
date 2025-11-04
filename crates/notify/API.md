# agentd-notify API Documentation

The notify service is a pure API daemon that manages notifications. It exposes an HTTP REST API for creating, reading, updating, and deleting notifications.

## Base URL

```
http://127.0.0.1:3030
```

## Endpoints

### Health Check

Check if the service is running.

```
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "service": "agentd-notify",
  "version": "0.1.0"
}
```

---

### List Notifications

Get all notifications with optional status filter.

```
GET /notifications?status={status}
```

**Query Parameters:**
- `status` (optional): Filter by status - `Pending`, `Viewed`, `Dismissed`, `Responded`, `Expired`

**Response:**
```json
[
  {
    "id": "uuid",
    "source": "System",
    "lifetime": "Persistent",
    "priority": "High",
    "status": "Pending",
    "title": "Notification Title",
    "message": "Notification message",
    "requires_response": false,
    "response": null,
    "created_at": "2025-11-04T18:00:00Z",
    "updated_at": "2025-11-04T18:00:00Z"
  }
]
```

---

### List Actionable Notifications

Get notifications that require user attention (Pending or Viewed status, not expired).

```
GET /notifications/actionable
```

**Response:** Array of notification objects (same format as List Notifications)

---

### List Notification History

Get dismissed, responded, or expired notifications.

```
GET /notifications/history
```

**Response:** Array of notification objects (same format as List Notifications)

---

### Get Notification

Get a specific notification by ID.

```
GET /notifications/{id}
```

**Response:** Single notification object

---

### Create Notification

Create a new notification.

```
POST /notifications
Content-Type: application/json
```

**Request Body:**
```json
{
  "source": "System",
  "lifetime": "Persistent",
  "priority": "High",
  "title": "Notification Title",
  "message": "Notification message",
  "requires_response": false
}
```

**Source Options:**
- `"System"` - System notification
- `{"AgentHook": {"agent_id": "agent-1", "hook_type": "pre-commit"}}` - From agent hook
- `{"AskService": {"request_id": "uuid"}}` - From ask service
- `{"MonitorService": {"alert_type": "cpu-high"}}` - From monitor service

**Lifetime Options:**
- `"Persistent"` - Remains until explicitly dismissed
- `{"Ephemeral": {"expires_at": "2025-11-04T19:00:00Z"}}` - Auto-expires

**Priority Options:**
- `"Low"`
- `"Normal"`
- `"High"`
- `"Critical"`

**Response:** Created notification object with 201 status

---

### Update Notification

Update a notification's status or response.

```
PUT /notifications/{id}
Content-Type: application/json
```

**Request Body:**
```json
{
  "status": "Viewed",
  "response": "User response text"
}
```

**Status Options:**
- `"Pending"` - Not yet viewed
- `"Viewed"` - User has seen it
- `"Dismissed"` - User dismissed without responding
- `"Responded"` - User provided a response
- `"Expired"` - Ephemeral notification expired

Both fields are optional - include only what you want to update.

**Response:** Updated notification object

---

### Delete Notification

Delete a notification permanently.

```
DELETE /notifications/{id}
```

**Response:** 204 No Content

---

## Data Models

### NotificationSource

```rust
enum NotificationSource {
    System,
    AgentHook { agent_id: String, hook_type: String },
    AskService { request_id: Uuid },
    MonitorService { alert_type: String },
}
```

### NotificationLifetime

```rust
enum NotificationLifetime {
    Persistent,
    Ephemeral { expires_at: DateTime<Utc> },
}
```

### NotificationPriority

```rust
enum NotificationPriority {
    Low,
    Normal,
    High,
    Critical,
}
```

### NotificationStatus

```rust
enum NotificationStatus {
    Pending,   // Not yet viewed
    Viewed,    // User has seen it
    Dismissed, // User dismissed without responding
    Responded, // User provided a response
    Expired,   // Ephemeral notification expired
}
```

## Background Tasks

The service runs these background tasks automatically:

- **Cleanup Task**: Runs every 5 minutes to:
  1. Mark expired ephemeral notifications as `Expired`
  2. Delete expired notifications

## Storage

Notifications are stored in SQLite at:
```
~/Library/Application Support/agentd-notify/notify.db
```

## Usage Examples

### Create a system notification

```bash
curl -X POST http://127.0.0.1:3030/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "source": "System",
    "lifetime": "Persistent",
    "priority": "High",
    "title": "System Update Available",
    "message": "A new system update is ready to install",
    "requires_response": false
  }'
```

### Create an ephemeral notification from agent hook

```bash
curl -X POST http://127.0.0.1:3030/notifications \
  -H "Content-Type: application/json" \
  -d '{
    "source": {"AgentHook": {"agent_id": "git-agent", "hook_type": "pre-commit"}},
    "lifetime": {"Ephemeral": {"expires_at": "2025-11-04T19:00:00Z"}},
    "priority": "Normal",
    "title": "Commit Hook Notification",
    "message": "Pre-commit checks passed",
    "requires_response": false
  }'
```

### Get actionable notifications

```bash
curl http://127.0.0.1:3030/notifications/actionable
```

### Mark notification as viewed

```bash
curl -X PUT http://127.0.0.1:3030/notifications/{id} \
  -H "Content-Type: application/json" \
  -d '{"status": "Viewed"}'
```

### Respond to a notification

```bash
curl -X PUT http://127.0.0.1:3030/notifications/{id} \
  -H "Content-Type: application/json" \
  -d '{"status": "Responded", "response": "User approved the request"}'
```

## Running the Service

```bash
cargo run -p agentd-notify
```

The service will start on `http://127.0.0.1:3030`

## Future GUI Application

A separate GUI application can be built that:
- Displays a system tray icon
- Polls this API for notifications
- Shows notifications in GPUI windows
- Handles user interactions and updates notifications via the API

This separation allows the notification service to focus on data management while the GUI handles presentation.
