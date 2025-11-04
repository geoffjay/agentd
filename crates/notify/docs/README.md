# GPUI API Research Documentation

This directory contains comprehensive research and implementation guidance for integrating the GPUI GUI framework into the notification service.

## Quick Start

1. **New to GPUI?** Start with `RESEARCH_SUMMARY.md` for a high-level overview
2. **Want API details?** Read `GPUI_API_RESEARCH.md` 
3. **Ready to build?** Follow `GPUI_IMPLEMENTATION_GUIDE.md`

## Files

### RESEARCH_SUMMARY.md
**Executive summary and quick reference**
- Key findings from the research
- Core GPUI concepts
- Implementation recommendations
- Code examples
- Next steps

**Best for:** Getting oriented quickly, understanding what was researched

### GPUI_API_RESEARCH.md
**Complete API reference documentation**
- App initialization
- Window creation and WindowOptions
- View patterns and the Render trait
- Element system
- Context types
- Integration patterns
- Design patterns
- File location references

**Best for:** Understanding how GPUI works, looking up specific API details

### GPUI_IMPLEMENTATION_GUIDE.md
**Practical step-by-step implementation guide**
- Architecture overview
- 5-step implementation walkthrough
- View structure
- Global notification manager
- Auto-dismiss logic
- Event system integration patterns
- Best practices
- Testing strategies
- Platform-specific considerations

**Best for:** Building the actual notification UI, implementing features

## Research Overview

### What Was Researched

- GPUI app initialization and lifecycle
- Window creation with full WindowOptions reference
- View patterns and the Render trait
- Element system and composition
- Context types and state management
- Global state management
- Async patterns and event integration
- Real examples from GPUI's own codebase

### Source Code Location

```
~/.cargo/git/checkouts/zed-a70e2ad075855582/5e41ce1/crates/gpui/
```

**Key files examined:**
- `src/app.rs` (85 KB) - Application initialization and window opening
- `src/element.rs` (26 KB) - Render trait and element system
- `src/window.rs` (185 KB) - Window management
- `src/view.rs` (12 KB) - View entity implementations
- `examples/` - Working examples including hello_world, pattern, svg, input, etc.

### Key Concepts

**Application**
- Entry point with `Application::new()`
- Owns the event loop via `run()`
- Can be configured with assets and HTTP client

**Windows**
- Created with `cx.open_window()`
- Takes `WindowOptions` for configuration
- One root view per window

**Views**
- Implement the `Render` trait
- Return elements that form the UI tree
- Wrapped in `Entity<V>` handles

**Elements**
- Built-in types: `div`, `text`, `button`, etc.
- Fluent builder pattern: `.flex().gap_2().child(...)`
- Custom elements via `RenderOnce` + `#[derive(IntoElement)]`

**State Management**
- Global state via `cx.set_global()` and `cx.global()`
- Entity updates via `entity.update(cx, |view, _cx| { ... })`
- Automatic re-rendering on state changes

## Implementation Roadmap

### Phase 1: Basic Setup
1. Create `NotificationView` struct implementing `Render`
2. Create `NotificationManager` global
3. Create notification window with basic styling

### Phase 2: Features
1. Add different severity levels (Info, Warning, Error, Success)
2. Implement auto-dismiss logic
3. Add dismiss button

### Phase 3: Integration
1. Connect to external notification sources
2. Implement event system integration
3. Add persistence if needed

### Phase 4: Polish
1. Test on multiple platforms
2. Optimize rendering
3. Add animations/transitions if desired

## Code Examples Quick Reference

### Creating an App
```rust
Application::new().run(|cx: &mut App| {
    cx.open_window(WindowOptions::default(), |_, cx| {
        cx.new(|_| MyView { ... })
    })?;
});
```

### Implementing a View
```rust
impl Render for MyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child("Content")
    }
}
```

### Updating State
```rust
entity.update(cx, |view, _cx| {
    view.field = new_value;
}); // Triggers re-render
```

### Global State
```rust
cx.set_global(NotificationManager::new());
cx.update_global::<NotificationManager, ()>(|manager, cx| {
    manager.show_notification(notification, cx);
});
```

## Important Notes

1. **Event Loop Ownership**: GPUI owns the event loop. `Application::run()` blocks until exit.
2. **Main Thread Only**: All UI operations on main thread. Use `cx.spawn()` for async work.
3. **Type Safe**: Strong typing with `Entity<V>` provides excellent IDE support and safety.
4. **Reactive**: Views automatically re-render when state changes via `cx.notify()`.
5. **Cross-Platform**: Single codebase runs on macOS, Linux (X11/Wayland), Windows.

## Next Steps

1. Review `RESEARCH_SUMMARY.md` for high-level understanding
2. Read `GPUI_API_RESEARCH.md` to learn the API
3. Follow `GPUI_IMPLEMENTATION_GUIDE.md` to build
4. Reference actual GPUI source code in `~/.cargo/git/checkouts/`
5. Examine GPUI examples for patterns

## Resources

- GPUI Source: `~/.cargo/git/checkouts/zed-a70e2ad075855582/5e41ce1/crates/gpui/`
- Zed Repository: https://github.com/zed-industries/zed
- GPUI Examples: See `examples/` in GPUI source

## Questions?

Refer to the appropriate documentation file:
- How does GPUI work? → `GPUI_API_RESEARCH.md`
- How do I build something? → `GPUI_IMPLEMENTATION_GUIDE.md`
- What should I do first? → `RESEARCH_SUMMARY.md`
- What exactly is X? → `GPUI_API_RESEARCH.md` (search for term)

