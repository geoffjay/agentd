# Terminal Panel Usage Guide

This document explains how to use the `TerminalPanel` component in the agentd UI.

## Overview

The `TerminalPanel` is a GPUI-based component that provides a terminal interface within the agentd UI. It's designed to attach to tmux sessions and display terminal content.

## Basic Usage

### Creating a Terminal Panel

```rust
use crate::workspace::TerminalPanel;

// In your workspace or component
let terminal_panel = TerminalPanel::view(window, cx);
```

### Attaching to a tmux Session

```rust
terminal_panel.update(cx, |panel, cx| {
    panel.attach_session("my-session".to_string(), cx);
});
```

### Detaching from a Session

```rust
terminal_panel.update(cx, |panel, cx| {
    panel.detach_session(cx);
});
```

### Checking Session Status

```rust
terminal_panel.update(cx, |panel, _cx| {
    if panel.is_attached() {
        if let Some(session_name) = panel.current_session() {
            println!("Connected to: {}", session_name);
        }
    }
});
```

## Events

The `TerminalPanel` emits the following events via `TerminalPanelEvent`:

- `SessionAttached(String)` - Fired when successfully attached to a tmux session
- `SessionDetached` - Fired when detached from a session
- `SessionError(String)` - Fired when an error occurs during connection

### Subscribing to Events

```rust
let terminal_panel = TerminalPanel::view(window, cx);

let subscription = cx.subscribe(&terminal_panel, |this, _, event: &TerminalPanelEvent, cx| {
    match event {
        TerminalPanelEvent::SessionAttached(session_name) => {
            println!("Attached to session: {}", session_name);
        }
        TerminalPanelEvent::SessionDetached => {
            println!("Detached from session");
        }
        TerminalPanelEvent::SessionError(error) => {
            eprintln!("Session error: {}", error);
        }
    }
    cx.notify();
});
```

## Integration Example

Here's a complete example of integrating the terminal panel into the workspace:

```rust
use crate::workspace::{TerminalPanel, TerminalPanelEvent};
use gpui::*;

pub struct Workspace {
    terminal_panel: Entity<TerminalPanel>,
    _subscriptions: Vec<Subscription>,
    // ... other fields
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let terminal_panel = TerminalPanel::view(window, cx);

        let _subscriptions = vec![
            cx.subscribe(&terminal_panel, |this, _, event: &TerminalPanelEvent, cx| {
                match event {
                    TerminalPanelEvent::SessionAttached(session_name) => {
                        // Update UI or state as needed
                        println!("Terminal connected: {}", session_name);
                    }
                    TerminalPanelEvent::SessionDetached => {
                        println!("Terminal disconnected");
                    }
                    TerminalPanelEvent::SessionError(error) => {
                        eprintln!("Terminal error: {}", error);
                    }
                }
                cx.notify();
            }),
        ];

        Self {
            terminal_panel,
            _subscriptions,
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(self.terminal_panel.clone())
    }
}
```

## Features

### Current Features (MVP)

- Attach to tmux sessions by name
- Display terminal content as text
- Basic keyboard event handling
- Focus management (Focusable trait)
- Event emission for session state changes
- Error handling and display

### Planned Features

- Integration with actual tmux backend (via ui_terminal crate)
- Scrollback buffer support
- Text selection and copy/paste
- Hyperlink detection and click handling
- ANSI color support
- Bidirectional keyboard input
- Terminal resizing

## Architecture

### Component Structure

```
TerminalPanel
├── Fields
│   ├── focus_handle: FocusHandle
│   ├── session_name: Option<String>
│   ├── terminal_content: Vec<String>
│   ├── error_message: Option<String>
│   └── status_message: String
└── Render output
    ├── Header (session name + status)
    └── Content (terminal output or placeholder)
```

### TerminalElement

A simple rendering element that displays terminal content as text. Uses the `RenderOnce` trait for efficient rendering.

```rust
pub struct TerminalElement {
    lines: Vec<String>,
}
```

## TODO: Integration with ui_terminal Crate

Currently, the `connect_to_tmux` function returns mock data. To integrate with a real terminal backend:

1. Add `ui_terminal` crate dependency to `crates/ui/Cargo.toml`
2. Replace the mock `connect_to_tmux` implementation with actual tmux integration
3. Implement bidirectional communication (keyboard input → tmux)
4. Add real-time terminal output updates (tmux → display)

Example integration point:

```rust
async fn connect_to_tmux(session_name: &str) -> Result<Vec<String>, String> {
    // Replace this with:
    // use ui_terminal::Terminal;
    // let terminal = Terminal::attach_tmux(session_name).await?;
    // Ok(terminal.get_content())

    // Current mock implementation
    Ok(vec!["Mock terminal output".to_string()])
}
```

## Keyboard Handling

The terminal panel tracks focus and can receive keyboard events. The `handle_key_down` method currently logs keystrokes but can be extended to:

- Forward input to the terminal backend
- Implement keyboard shortcuts (Ctrl+C, Ctrl+D, etc.)
- Handle special keys (arrows, function keys, etc.)

## Styling

The terminal panel uses the application theme via `cx.theme()`:

- `theme.foreground` - Terminal text color
- `theme.background` - Terminal background color
- `theme.border` - Border colors
- `theme.muted_foreground` - Status text color
- `theme.danger` - Error message color

## Error Handling

Errors are displayed inline in the terminal panel:

- Connection errors show in the main content area
- Status messages appear in the header
- Events are emitted for programmatic error handling
