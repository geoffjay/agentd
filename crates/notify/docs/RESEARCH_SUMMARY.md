# GPUI API Research - Executive Summary

## Research Completion

This directory contains comprehensive research on the GPUI API from the Zed codebase for integration into the notification service.

**Research Date:** 2025-11-03
**GPUI Source:** `~/.cargo/git/checkouts/zed-a70e2ad075855582/5e41ce1/crates/gpui/`

---

## Documentation Files

### 1. GPUI_API_RESEARCH.md (598 lines)
**Comprehensive API Reference**

Complete documentation of the GPUI API covering:
- App initialization and lifecycle
- Window creation and WindowOptions
- View patterns and the Render trait
- Element system and composition
- Context types (Context<T>, App, AsyncApp)
- Complete working examples
- Integration patterns for async/external systems
- Key design patterns
- File location references

**Use this for:** Understanding how GPUI works at the API level

### 2. GPUI_IMPLEMENTATION_GUIDE.md (516 lines)
**Practical Implementation Guide**

Step-by-step guide for implementing a notification service with GPUI:
- Architecture overview and diagrams
- 5-step implementation walkthrough
- View structure and container design
- Global notification manager
- Auto-dismiss logic
- External event system integration (2 patterns)
- Best practices
- Testing strategies
- Performance considerations
- Platform-specific notes

**Use this for:** Building the actual notification UI

---

## Key Findings

### 1. App Architecture

GPUI uses an event-driven, reactive architecture:

```
Application::new()
    .with_assets(...)
    .with_http_client(...)
    .run(|cx: &mut App| {
        // Initialize application
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| MyView { ... })
        })
    });
```

### 2. Core Concepts

**Application** - Entry point, owns the event loop
**Windows** - Created with `cx.open_window()`, takes WindowOptions and a builder
**Views** - Implement `Render` trait, return elements
**Elements** - Built-in (div, text, button) or custom, use fluent builders
**Entities** - All views are `Entity<V>` handles, can be cloned and updated

### 3. Rendering Model

```rust
pub trait Render: 'static + Sized {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}
```

- Called every frame when view is dirty
- Returns elements that form the UI tree
- Views automatically re-render on state changes

### 4. State Management

```rust
// Create entity
let entity = cx.new(|_| MyView { data: "initial" });

// Update state
entity.update(cx, |view, _cx| {
    view.data = "new value"; // Triggers re-render
});

// Read entity
entity.read(cx, |view, _| {
    println!("{}", view.data);
});
```

### 5. Global State

```rust
// Store global
cx.set_global(NotificationManager::new());

// Access from views
let manager = cx.global::<NotificationManager>();

// Update global
cx.update_global::<NotificationManager, ()>(|manager, _cx| {
    manager.add_notification(...);
});
```

---

## Implementation Recommendations

### For Notification Service

1. **Create NotificationView struct** implementing `Render`
   - Store title, message, severity, timestamps
   - Render with styling based on severity

2. **Create NotificationManager global**
   - Track all active notifications
   - Manage window lifecycle
   - Handle dismissal

3. **Create NotificationContainer**
   - Root view for the notification window
   - Renders all active notifications

4. **Integration approaches**
   - Use channel for external events
   - Store NotificationService globally
   - Use cx.spawn() for async operations

### Performance Notes

- Single window with multiple notifications is efficient
- Views re-render only when state changes
- GPUI handles layout via Taffy (Flexbox/CSS Grid)
- Main thread rendering with async support

---

## Code Examples From Research

### 1. Basic App with Window

```rust
fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| MyView { text: "Hello".into() }),
        )
        .unwrap();
        cx.activate(true);
    });
}
```

### 2. View with Render Impl

```rust
struct MyView {
    text: String,
}

impl Render for MyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size_full()
            .justify_center()
            .items_center()
            .child(format!("Hello, {}!", &self.text))
    }
}
```

### 3. Nested Elements

```rust
div()
    .flex()
    .flex_col()
    .gap_2()
    .bg(rgb(0xffffff))
    .w(px(500.0))
    .child("Title")
    .child(
        div()
            .bg(rgb(0xff0000))
            .size(px(100.0))
    )
```

---

## Tested Capabilities

### Successfully Researched

- [x] App initialization with `Application::new().run()`
- [x] Window creation with `open_window()`
- [x] WindowOptions struct and all fields
- [x] Render trait signature and parameters
- [x] View patterns (basic, nested, with state)
- [x] Context types (Context<T>, App, AsyncApp)
- [x] Element system and IntoElement trait
- [x] Global state management
- [x] Async patterns with spawn()
- [x] Real examples from GPUI examples/ directory

### Real Examples Found

- `hello_world.rs` - Basic example
- `pattern.rs` - Layout and styling
- `svg/svg.rs` - SVG rendering
- `input.rs` - Text input and events
- Many more in examples/ directory

---

## Important Considerations

### 1. Event Loop Ownership
GPUI owns the event loop. `Application::run()` blocks until the app exits. This is suitable for a standalone notification service but requires careful integration if combining with other event loops.

### 2. Main Thread Only
All UI operations must happen on the main thread. Use `cx.spawn()` and executors for async work that needs to interact with the UI.

### 3. Type Safety
GPUI uses strong typing throughout. Views are `Entity<V>` where V is the view type. This provides excellent type safety and IDE support.

### 4. Platform Differences
WindowOptions provides cross-platform configuration, but some options are platform-specific (e.g., `traffic_light_position` for macOS, `window_decorations` for Wayland).

---

## Next Steps

1. **Review the documentation**
   - Start with GPUI_API_RESEARCH.md for API understanding
   - Use GPUI_IMPLEMENTATION_GUIDE.md for building

2. **Create prototype**
   - Implement basic NotificationView
   - Test window creation
   - Verify styling and layout

3. **Integrate with notification service**
   - Connect to external event sources
   - Implement auto-dismiss logic
   - Add persistence if needed

4. **Test on multiple platforms**
   - Verify appearance on macOS, Linux, Windows
   - Test with different notification severities

---

## Resources

**GPUI Documentation Location:**
```
~/.cargo/git/checkouts/zed-a70e2ad075855582/5e41ce1/crates/gpui/
```

**Key Files:**
- `src/app.rs` - Application and App context
- `src/element.rs` - Element trait and Render trait
- `src/window.rs` - Window management
- `src/view.rs` - View entity implementations
- `examples/` - Working examples

**Cargo Dependency (when ready):**
Add to Cargo.toml in the notification crate:
```toml
[dependencies]
gpui = { path = "../../.cargo/git/checkouts/zed-a70e2ad075855582/5e41ce1/crates/gpui" }
```

Or from Zed repository if published.

---

## Conclusion

The GPUI API is well-designed for building cross-platform notification UIs. It provides:
- Clear, type-safe API for building UI
- Reactive rendering model
- Excellent async support
- Cross-platform consistency
- Integration with Zed ecosystem

The research provides sufficient information to implement a robust, production-quality notification service using GPUI.

