# GPUI Implementation Guide for Notification Service

Based on the GPUI API research, this document provides concrete implementation guidance for integrating GPUI into the notification service.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                  Application                             │
│  Application::new().run(|cx: &mut App| { ... })         │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
        ┌──────────────────────────────┐
        │   NotificationWindow         │
        │   (Render impl)              │
        └────────┬─────────────────────┘
                 │
         ┌───────┴────────┐
         ▼                ▼
    ┌──────────┐    ┌──────────────┐
    │ Titlebar │    │ Content Area │
    └──────────┘    └────┬─────────┘
                         │
                    ┌────┴────┐
                    ▼         ▼
              ┌────────┐  ┌────────┐
              │ Title  │  │ Message│
              └────────┘  └────────┘
```

---

## Step-by-Step Implementation

### Step 1: Create the View Structure

```rust
use gpui::{Context, Render, Window, div, prelude::*};
use std::time::Duration;

#[derive(Clone)]
pub struct NotificationView {
    pub id: String,
    pub title: String,
    pub message: String,
    pub severity: NotificationSeverity,
    pub auto_dismiss_after: Option<Duration>,
    pub created_at: std::time::Instant,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
    Success,
}

impl NotificationView {
    pub fn new(id: impl Into<String>, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            message: message.into(),
            severity: NotificationSeverity::Info,
            auto_dismiss_after: Some(Duration::from_secs(5)),
            created_at: std::time::Instant::now(),
        }
    }
    
    pub fn error(id: impl Into<String>, title: impl Into<String>, message: impl Into<String>) -> Self {
        let mut notification = Self::new(id, title, message);
        notification.severity = NotificationSeverity::Error;
        notification.auto_dismiss_after = None; // Errors don't auto-dismiss
        notification
    }
    
    pub fn warning(id: impl Into<String>, title: impl Into<String>, message: impl Into<String>) -> Self {
        let mut notification = Self::new(id, title, message);
        notification.severity = NotificationSeverity::Warning;
        notification
    }
}

impl Render for NotificationView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = match self.severity {
            NotificationSeverity::Info => gpui::rgb(0x2a5c8e),
            NotificationSeverity::Warning => gpui::rgb(0x8b5a00),
            NotificationSeverity::Error => gpui::rgb(0x8b2c2c),
            NotificationSeverity::Success => gpui::rgb(0x2a6e2a),
        };
        
        let icon = match self.severity {
            NotificationSeverity::Info => "ℹ",
            NotificationSeverity::Warning => "⚠",
            NotificationSeverity::Error => "✕",
            NotificationSeverity::Success => "✓",
        };
        
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(bg_color)
            .border_1()
            .border_color(gpui::rgb(0x505050))
            .rounded_lg()
            .shadow_lg()
            .w(px(400.0))
            .max_w(px(600.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_bold()
                            .text_color(gpui::white())
                            .child(icon)
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_bold()
                            .text_color(gpui::white())
                            .child(&self.title)
                    )
            )
            .child(
                div()
                    .text_sm()
                    .text_color(gpui::rgb(0xe0e0e0))
                    .child(&self.message)
            )
    }
}
```

### Step 2: Define the Global Notification Manager

```rust
use gpui::Global;
use std::collections::HashMap;
use std::sync::Arc;

pub struct NotificationManager {
    notifications: HashMap<String, Entity<NotificationView>>,
    window_handle: Option<WindowHandle<NotificationContainer>>,
}

impl Global for NotificationManager {}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notifications: HashMap::new(),
            window_handle: None,
        }
    }
    
    pub fn show_notification(&mut self, notification: NotificationView, cx: &mut App) {
        let id = notification.id.clone();
        
        // Create window if not exists
        if self.window_handle.is_none() {
            let handle = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(
                        Bounds::centered(None, size(px(420.0), px(600.0)), cx)
                    )),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Notifications".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    focus: false,
                    show: true,
                    is_movable: false,
                    is_resizable: false,
                    is_minimizable: false,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| NotificationContainer::new()),
            ).ok();
            self.window_handle = handle;
        }
        
        // Create notification view entity
        let entity = cx.new(|_| notification);
        self.notifications.insert(id, entity);
    }
    
    pub fn dismiss_notification(&mut self, id: &str) {
        self.notifications.remove(id);
    }
}
```

### Step 3: Create Container View

```rust
use gpui::{Context, Render, Window, div, prelude::*};

pub struct NotificationContainer {
    // Empty - will render child notifications
}

impl NotificationContainer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Render for NotificationContainer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let manager = cx.global::<NotificationManager>();
        
        let notifications: Vec<Entity<NotificationView>> = manager
            .notifications
            .values()
            .cloned()
            .collect();
        
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(gpui::rgb(0x1a1a1a))
            .size_full()
            .children(notifications)
    }
}
```

### Step 4: Initialize in Main

```rust
use gpui::{Application, App, WindowOptions, Bounds, size, px};

fn main() {
    Application::new().run(|cx: &mut App| {
        // Initialize notification manager
        cx.set_global(NotificationManager::new());
        
        // Show a test notification
        let notification = NotificationView::new(
            "test-1",
            "Welcome",
            "The notification service is running"
        );
        
        cx.update_global::<NotificationManager, ()>(|manager, cx| {
            manager.show_notification(notification, cx);
        });
        
        cx.activate(true);
    });
}
```

### Step 5: Add Auto-Dismiss Logic

```rust
impl NotificationView {
    pub fn check_auto_dismiss(&self) -> bool {
        if let Some(duration) = self.auto_dismiss_after {
            self.created_at.elapsed() >= duration
        } else {
            false
        }
    }
}

// In the container render method, filter out dismissed notifications
impl Render for NotificationContainer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let manager = cx.global::<NotificationManager>();
        
        let notifications: Vec<Entity<NotificationView>> = manager
            .notifications
            .values()
            .filter(|entity| {
                let should_dismiss = entity.read(cx, |view, _| {
                    view.check_auto_dismiss()
                }).unwrap_or(false);
                
                if should_dismiss {
                    // Schedule dismissal
                    // Note: Cannot modify from render, need to use cx.spawn or defer
                }
                
                !should_dismiss
            })
            .cloned()
            .collect();
        
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(gpui::rgb(0x1a1a1a))
            .size_full()
            .children(notifications)
    }
}
```

---

## Integration with External Event System

### Pattern 1: Using Channels

```rust
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::Arc;
use std::sync::Mutex;

pub struct NotificationService {
    sender: Arc<Mutex<Sender<NotificationEvent>>>,
}

pub enum NotificationEvent {
    Show(NotificationView),
    Dismiss(String),
    Clear,
}

impl NotificationService {
    pub fn new(sender: Sender<NotificationEvent>) -> Self {
        Self {
            sender: Arc::new(Mutex::new(sender)),
        }
    }
    
    pub fn show(&self, notification: NotificationView) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(NotificationEvent::Show(notification));
        }
    }
}

// In the app loop
Application::new().run(|cx: &mut App| {
    let (tx, rx) = channel();
    let service = NotificationService::new(tx);
    
    // Store service globally
    cx.set_global(service);
    
    // Spawn task to check channel
    cx.spawn(|mut cx| async move {
        loop {
            if let Ok(event) = rx.try_recv() {
                cx.update(|app| {
                    match event {
                        NotificationEvent::Show(notif) => {
                            cx.update_global::<NotificationManager, ()>(|manager, cx| {
                                manager.show_notification(notif, cx);
                            });
                        },
                        NotificationEvent::Dismiss(id) => {
                            cx.update_global::<NotificationManager, ()>(|manager, _| {
                                manager.dismiss_notification(&id);
                            });
                        },
                        NotificationEvent::Clear => {
                            cx.update_global::<NotificationManager, ()>(|manager, _| {
                                manager.notifications.clear();
                            });
                        },
                    }
                }).ok();
            }
            
            smol::Timer::after(Duration::from_millis(100)).await;
        }
    });
});
```

### Pattern 2: Direct Integration with Global

```rust
// External system sends notifications
let notification = NotificationView::new("id", "Title", "Message");

// Store app handle to update later
let app_handle = cx.background_executor();

// From external thread
app_handle.spawn(|mut cx| async move {
    cx.update(|app| {
        cx.update_global::<NotificationManager, ()>(|manager, cx| {
            manager.show_notification(notification, cx);
        });
    }).ok();
});
```

---

## Best Practices

### 1. Use Entities for Notifications

Each notification should be an `Entity<NotificationView>` so it can be tracked and updated independently.

### 2. Store in Global Manager

The `NotificationManager` global keeps track of all active notifications and their window handles.

### 3. Async Operations

Use `cx.spawn()` for long-running operations:

```rust
cx.spawn(|mut cx| async move {
    let result = fetch_data().await;
    cx.update(|app| {
        // Update notification with result
    }).ok();
});
```

### 4. Handle Window Lifecycle

Store `WindowHandle<T>` in the manager to track open windows and prevent duplicate creation.

### 5. Update From Context

Always use `cx.update()` or `entity.update()` to modify state:

```rust
// Good
entity.update(cx, |notification, _cx| {
    notification.message = "Updated".into();
});

// Don't do this - won't trigger re-render
// notification.message = "Updated".into();
```

---

## Testing

### Test View Creation

```rust
#[test]
fn test_notification_creation() {
    let notification = NotificationView::new("id", "Title", "Message");
    assert_eq!(notification.title, "Title");
    assert_eq!(notification.severity, NotificationSeverity::Info);
}
```

### Test with App Context

```rust
#[test]
fn test_notification_window() {
    TestAppContext::new(|cx| {
        let notification = NotificationView::new("id", "Title", "Message");
        let entity = cx.new(|_| notification);
        
        entity.update(cx, |view, _| {
            assert_eq!(view.title, "Title");
        });
    });
}
```

---

## Performance Considerations

1. **Rendering**: Each notification re-renders only when its state changes
2. **Memory**: Store only necessary data in each view
3. **Windows**: One window with multiple notification views is more efficient than multiple windows
4. **Updates**: Batch updates using `cx.spawn()` to avoid frequent re-renders

---

## Platform-Specific Considerations

### macOS
- Use `window_decorations` to customize appearance
- `traffic_light_position` allows moving window controls
- System tray integration possible via platform APIs

### Linux (X11/Wayland)
- Use appropriate `WindowDecorations`
- Test with both X11 and Wayland

### Windows
- `appears_transparent` titlebar option available
- Consider Windows 11+ styling

---

## Example: Complete Notification Service

See `/Users/geoff/Projects/agentd/crates/notify/examples/gpui_integration.rs` for a complete working example combining all the patterns above.

