# GPUI API Research Summary

Comprehensive research on the GPUI API from the Zed codebase for integrating into the notification service.

**Source Location:**
`~/.cargo/git/checkouts/zed-a70e2ad075855582/5e41ce1/crates/gpui/`

---

## Overview

GPUI is a cross-platform GUI framework built for Zed. It's built on a reactive architecture with entities, views, and elements. The API uses a trait-based design pattern with fluent builders for composing UI components.

Key characteristics:
- **Event-driven**: Platform manages the event loop
- **Reactive**: Views automatically re-render when state changes
- **Entity-based**: All views are `Entity<V>` handles
- **Type-safe**: Strong typing throughout
- **Fluent builders**: Elements use method chaining for composition

---

## 1. App Initialization

### Creating and Running an App

```rust
use gpui::{Application, App, WindowOptions, WindowBounds, div, prelude::*};

fn main() {
    Application::new().run(|cx: &mut App| {
        // Application initialization code here
        cx.activate(true);
    });
}
```

### Application Methods

```rust
impl Application {
    /// Create a new app
    pub fn new() -> Self
    
    /// Create a headless app (no GUI)
    pub fn headless() -> Self
    
    /// Set custom asset source
    pub fn with_assets(self, asset_source: impl AssetSource) -> Self
    
    /// Set HTTP client
    pub fn with_http_client(self, http_client: Arc<dyn HttpClient>) -> Self
    
    /// Start the application event loop
    /// Takes ownership and blocks until app exits
    pub fn run<F>(self, on_finish_launching: F)
    where
        F: 'static + FnOnce(&mut App)
}
```

### Event Handlers

```rust
let app = Application::new();

// Handle URL opens
app.on_open_urls(|urls| {
    println!("Opening URLs: {:?}", urls);
});

// Handle app reopen (macOS only)
app.on_reopen(|cx: &mut App| {
    println!("App was reopened");
});
```

---

## 2. Window Creation

### Opening Windows

```rust
pub fn open_window<V: 'static + Render>(
    &mut self,
    options: WindowOptions,
    build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> anyhow::Result<WindowHandle<V>>
```

The `build_root_view` function is called immediately to create the root view entity.

### Example: Basic Window

```rust
cx.open_window(
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    },
    |_window, cx| cx.new(|_cx| MyView { /* ... */ }),
)?;
```

### WindowOptions Structure

```rust
pub struct WindowOptions {
    /// Window state and bounds in screen coordinates
    pub window_bounds: Option<WindowBounds>,
    
    /// Titlebar configuration
    pub titlebar: Option<TitlebarOptions>,
    
    /// Whether to focus when created
    pub focus: bool,
    
    /// Whether to show when created
    pub show: bool,
    
    /// Window type (Normal, Modal, PopUp, etc.)
    pub kind: WindowKind,
    
    /// Can user move the window
    pub is_movable: bool,
    
    /// Can user resize the window
    pub is_resizable: bool,
    
    /// Can user minimize the window
    pub is_minimizable: bool,
    
    /// Display to create on (None = main display)
    pub display_id: Option<DisplayId>,
    
    /// Background appearance
    pub window_background: WindowBackgroundAppearance,
    
    /// Application identifier for grouping
    pub app_id: Option<String>,
    
    /// Minimum window size
    pub window_min_size: Option<Size<Pixels>>,
    
    /// Client/server side decorations (Wayland only)
    pub window_decorations: Option<WindowDecorations>,
    
    /// Tabbing identifier (macOS 10.12+)
    pub tabbing_identifier: Option<String>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            window_bounds: None,
            titlebar: Some(TitlebarOptions { ... }),
            focus: true,
            show: true,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            display_id: None,
            window_background: WindowBackgroundAppearance::default(),
            app_id: None,
            window_min_size: None,
            window_decorations: None,
            tabbing_identifier: None,
        }
    }
}
```

### TitlebarOptions

```rust
pub struct TitlebarOptions {
    /// Initial title of the window
    pub title: Option<SharedString>,
    
    /// Hide default system titlebar (macOS/Windows only)
    pub appears_transparent: bool,
    
    /// Position of macOS traffic light buttons
    pub traffic_light_position: Option<Point<Pixels>>,
}
```

### WindowBounds

```rust
pub enum WindowBounds {
    /// Windowed with specific bounds
    Windowed(Bounds<Pixels>),
    
    /// Maximized state
    Maximized,
    
    /// Full screen
    Fullscreen,
}
```

### Window with Custom Options

```rust
use gpui::{Bounds, size, px};

let bounds = Bounds::centered(None, size(px(500.0), px(400.0)), cx);
cx.open_window(
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Notification".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        is_movable: true,
        is_resizable: true,
        is_minimizable: false,
        ..Default::default()
    },
    |_window, cx| cx.new(|_cx| NotificationView),
)?;
```

---

## 3. View Patterns

### The Render Trait

```rust
pub trait Render: 'static + Sized {
    /// Render this view into an element tree.
    /// Called every frame (when the view is dirty or the app is refreshing)
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}
```

### Basic View Example

```rust
use gpui::{Context, Render, Window, div, prelude::*};

struct NotificationView {
    title: String,
    message: String,
}

impl Render for NotificationView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(gpui::rgb(0x2a2a2a))
            .border_1()
            .border_color(gpui::rgb(0x404040))
            .rounded_md()
            .shadow_lg()
            .child(
                div()
                    .text_xl()
                    .font_bold()
                    .text_color(gpui::white())
                    .child(&self.title)
            )
            .child(
                div()
                    .text_sm()
                    .text_color(gpui::rgb(0xcccccc))
                    .child(&self.message)
            )
    }
}
```

### Creating View Entities

```rust
// In the app initialization or window opening handler
let view = cx.new(|_cx| NotificationView {
    title: "New Message".into(),
    message: "You have a new notification".into(),
});

// Or in a window's build_root_view function
cx.open_window(
    WindowOptions::default(),
    |_window, cx| cx.new(|_cx| NotificationView {
        title: "Alert".into(),
        message: "Something happened".into(),
    }),
)?;
```

### View with State Updates

```rust
struct Counter {
    count: i32,
}

impl Render for Counter {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(format!("Count: {}", self.count))
    }
}

// Update the view - triggers re-render
entity.update(cx, |view, _cx| {
    view.count += 1;
});
```

### Nested Views

```rust
// Using entity references (automatically implements IntoElement)
impl Render for ParentView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(self.child_view.clone()) // AnyView is IntoElement
            .child(self.another_view.clone())
    }
}

// Creating child views in render
struct Parent {
    child_entity: Option<Entity<Child>>,
}

impl Render for Parent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.child_entity.is_none() {
            self.child_entity = Some(cx.new(|_| Child { /* ... */ }));
        }
        
        let child = self.child_entity.clone().unwrap();
        
        div()
            .child(child) // Entity<T> implements IntoElement
    }
}
```

---

## 4. Render Trait Deep Dive

### Method Signature

```rust
pub trait Render: 'static + Sized {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}
```

### Parameter: `window: &mut Window`

Access to window-specific functionality:
- Layout system via `window.request_layout()`
- Text measurement via `window.text_system()`
- Element state management
- Content masking for overlays
- Event handling setup

### Parameter: `cx: &mut Context<Self>`

Type-specific context for the view:

```rust
// Create sub-entities
let child = cx.new(|_| ChildView { /* ... */ });

// Update other entities
entity.update(cx, |view, _cx| {
    view.state = new_value;
});

// Access globals
let settings = cx.global::<Settings>();
let config = cx.global_mut::<Config>();

// Subscribe to events
let _sub = cx.subscribe(&entity, |event: &MyEvent, view, _window, cx| {
    // Handle event
});
```

---

## 5. Complete Working Example

```rust
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, 
    div, prelude::*, px, rgb, size,
};

struct HelloWorld {
    text: String,
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(format!("Hello, {}!", &self.text))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| HelloWorld {
                    text: "World".into(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
```

---

## 6. Integration Patterns for Notification Service

### Async Tasks Pattern

```rust
// Spawn background task
cx.spawn(|mut cx| async move {
    // Do async work
    let notification = fetch_notification().await;
    
    // Update UI back in foreground
    cx.update(|app| {
        entity.update(app, |view, _cx| {
            view.notification = notification;
        });
    }).ok();
});
```

### Using Globals for Shared State

```rust
// Define a global
pub struct NotificationService {
    notifications: Vec<Notification>,
}

impl Global for NotificationService {}

// Initialize in app
cx.set_global(NotificationService::new());

// Access from views
let service = cx.global::<NotificationService>();
let notifications = &service.notifications;

// Update from views
cx.update_global::<NotificationService, ()>(|service, _cx| {
    service.notifications.push(new_notification);
});
```

### External Event Integration

GPUI owns the event loop. Integration patterns:

1. **Pre-initialization**: Set up external systems before calling `run()`
2. **Foreground Executor**: Use `cx.foreground_executor()` to schedule work
3. **Background Executor**: Use `cx.background_executor()` for async tasks
4. **Global State**: Store references to external systems in globals

```rust
// Setup external system first
let notification_manager = Arc::new(NotificationManager::new());

Application::new().run(|cx: &mut App| {
    // Store as global
    cx.set_global(notification_manager.clone());
    
    // Open notification window
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |_, cx| cx.new(|_| NotificationView),
    )?;
});
```

---

## 7. Key Design Patterns

### 1. Reactive Architecture
- Views automatically re-render when state changes
- Use `cx.notify()` to mark view as dirty
- GPUI handles scheduling re-renders

### 2. Entity-Based System
- All views are `Entity<V>` handles
- Can be cloned and passed around
- Use `.update()` to mutate and access the view

### 3. Fluent Builder Pattern
- Elements use method chaining
- Each method returns `Self` for chaining

### 4. Type Safety
- Generic view types with associated render methods
- Strong typing via `Entity<T>`, `WindowHandle<T>`

---

## 8. Common Styling Methods

```rust
div()
    // Layout
    .flex()
    .flex_col()
    .gap_2()
    
    // Size
    .w(px(100.0))
    .h(px(100.0))
    .size_full()
    
    // Positioning
    .justify_center()
    .items_center()
    .p_2()
    .m_2()
    
    // Styling
    .bg(rgb(0xffffff))
    .text_color(rgb(0x000000))
    .border_1()
    .rounded_md()
    .shadow_lg()
    
    // Text
    .text_xl()
    .font_bold()
    
    // Content
    .child("Text")
    .children(vec![...])
```

---

## Summary

The GPUI API is well-structured for building cross-platform notification UIs:

1. **App initialization** is straightforward - create Application, set options, call run()
2. **Window creation** uses WindowOptions with flexible configuration
3. **Views** implement the Render trait and return elements
4. **Elements** use fluent builders for composition
5. **State management** via entities with update() method
6. **Async integration** via spawn() and foreground/background executors
7. **Type safety** throughout with generics and traits

This makes GPUI an excellent choice for implementing a notification display service with proper integration into the agentd ecosystem.

