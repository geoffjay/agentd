# agentd-notify

Notification service daemon with system tray integration and persistent storage.

## Status

### ✅ Completed
- **Notification Data Model** - Hybrid lifetime support (ephemeral vs persistent)
- **SQLite Storage** - Full CRUD operations with platform-specific paths
- **System Tray Icon** - Native tray icon with menu (macOS/Linux)
- **Event Loop Integration** - tao event loop for native UI
- **Background Tasks** - Automatic cleanup of expired notifications

### 🚧 In Progress
- **HTTP API** - Axum server for receiving notifications
- **IPC Communication** - Communication with other agentd services

## Architecture

```
┌─────────────────────────────────────────┐
│           Main Thread (tao)             │
│  ┌─────────────────────────────────┐   │
│  │      System Tray Icon           │   │
│  │  • Show Notifications           │   │
│  │  • Show History                 │   │
│  │  • Quit                         │   │
│  └─────────────────────────────────┘   │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│      Tokio Runtime (background)         │
│  ┌─────────────────────────────────┐   │
│  │   SQLite Storage Layer          │   │
│  │  • Persistent notifications     │   │
│  │  • Ephemeral notifications      │   │
│  │  • Response tracking            │   │
│  └─────────────────────────────────┘   │
│  ┌─────────────────────────────────┐   │
│  │   Background Cleanup Task       │   │
│  │  • Runs every 5 minutes         │   │
│  │  • Removes expired notifications│   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

## Storage Location

The SQLite database is stored at platform-specific locations:

- **macOS**: `~/Library/Application Support/agentd-notify/notify.db`
- **Linux**: `~/.local/share/agentd-notify/notify.db`

## Running

```bash
# Build
cargo build -p agentd-notify

# Run
cargo run -p agentd-notify

# With debug logging
RUST_LOG=debug cargo run -p agentd-notify
```

## Testing the Tray Icon

1. Run the service
2. Look for the white circular icon in your system tray/menu bar
3. Click it to see the menu
4. Try each menu option:
   - **Show Notifications** - Queries actionable notifications from storage
   - **Show History** - Queries all notifications from storage
   - **Quit** - Gracefully exits the daemon

## Next Steps

### Phase 2: HTTP API
- [ ] Axum server setup
- [ ] POST /notifications endpoint
- [ ] GET /notifications/:id endpoint
- [ ] GET /notifications/:id/response endpoint
- [ ] Webhook/callback support

### Phase 3: Advanced Features
- [ ] Notification badges on tray icon
- [ ] Sound notifications
- [ ] Priority-based queuing
- [ ] Custom icons per notification type

## Development Notes

### Event Loop Architecture

The service uses **tao's event loop** on the main thread because system tray icons require native event pumping to be visible.

### Async/Sync Boundary

- **Main thread**: Synchronous, runs tao event loop
- **Background tasks**: Async with tokio runtime
- **Communication**: `Arc<Mutex<>>` for shared state

### Testing

Currently the notification and storage modules have unit tests. Run them with:

```bash
cargo test -p agentd-notify
```

## References

- [Notification Model](./src/notification.rs) - Data structures for notifications
- [Storage Layer](./src/storage.rs) - SQLite persistence
- [Tray Icon](./src/tray/menu.rs) - System tray integration
- [Architecture Plan](../../docs/agentd-notify-plan.md) - Detailed design document
