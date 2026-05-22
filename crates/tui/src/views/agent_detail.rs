use crate::app::App;
use orchestrator::types::{ActivityState, AgentStatus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    if app.selected_agent.is_none() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // compact info bar
            Constraint::Min(5),     // conversation
            Constraint::Length(3),  // input box
        ])
        .split(area);

    render_info(f, app, chunks[0]);
    render_conversation(f, app, chunks[1]);
    render_input(f, app, chunks[2]);

    // Position the terminal cursor inside the input box when in input mode.
    if app.input_mode {
        let inner = chunks[2];
        // Account for the block border (1 cell) and the "> " prefix (2 cells).
        let cursor_col = inner.x + 1 + 2 + visible_cursor_col(app);
        let cursor_row = inner.y + 1;
        if cursor_col < inner.x + inner.width.saturating_sub(1) {
            f.set_cursor_position((cursor_col, cursor_row));
        }
    }
}

fn render_info(f: &mut Frame, app: &App, area: Rect) {
    let Some(agent) = &app.selected_agent else { return };

    let status_style = match agent.status {
        AgentStatus::Running => Style::default().fg(Color::Green),
        AgentStatus::Pending => Style::default().fg(Color::Yellow),
        AgentStatus::Failed => Style::default().fg(Color::Red),
        AgentStatus::Stopped => Style::default().fg(Color::DarkGray),
    };

    let activity_span = match agent.activity {
        ActivityState::Busy => Span::styled("busy", Style::default().fg(Color::Yellow)),
        ActivityState::Idle => Span::styled("idle", Style::default().fg(Color::DarkGray)),
    };

    let backend = agent.backend_type.as_deref().unwrap_or("-");
    let session = agent.session_id.as_deref().unwrap_or("-");
    let model = agent.config.model.as_deref().unwrap_or("default");

    let lines = vec![
        Line::from(vec![
            dim("status: "),
            Span::styled(agent.status.to_string(), status_style),
            dim("  activity: "),
            activity_span,
            dim("  backend: "),
            Span::raw(backend),
        ]),
        Line::from(vec![
            dim("session: "),
            Span::raw(session),
        ]),
        Line::from(vec![
            dim("dir: "),
            Span::raw(agent.config.working_dir.clone()),
            dim("  model: "),
            Span::raw(model),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", agent.name));

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn render_conversation(f: &mut Frame, app: &mut App, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let lines = build_lines(app, inner_width);

    // Compute the total height in *terminal rows* rather than logical line
    // count.  ratatui's Paragraph::scroll() offset is in terminal rows, so
    // using lines.len() underestimates when long lines wrap and causes the
    // last entries to stay hidden below the visible area.
    let total_rows = display_rows(&lines, inner_width);
    let visible = area.height.saturating_sub(2);
    let max_scroll = total_rows.saturating_sub(visible);

    let scroll = if app.conversation_follow {
        max_scroll
    } else {
        app.conversation_scroll.min(max_scroll)
    };
    app.conversation_scroll = scroll;

    if app.conversation_follow {
        app.conversation_scroll = max_scroll;
    }

    let count = app.conversation.len();
    let title = if count == 0 {
        " Conversation ".to_string()
    } else {
        format!(" Conversation ({count}) ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title);

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(para, area);
}

fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let (border_style, title) = if app.input_mode {
        (Style::default().fg(Color::Cyan), " Send Message ")
    } else {
        (Style::default().fg(Color::DarkGray), " Message (i to type) ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(title);

    let text = if app.input_mode {
        let before = &app.input_buffer[..app.input_cursor];
        let after = &app.input_buffer[app.input_cursor..];
        // Show a block cursor character at the insertion point.
        format!("> {before}\u{2588}{after}")
    } else if app.input_buffer.is_empty() {
        String::new()
    } else {
        format!("> {}", app.input_buffer)
    };

    let para = Paragraph::new(text).block(block);
    f.render_widget(para, area);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Estimate the total number of terminal rows the lines will occupy when
/// rendered with word-wrap inside a pane of `width` columns.
///
/// ratatui's `Paragraph::scroll()` offset is in terminal rows, not logical
/// line count.  Using `lines.len()` as the total underestimates when any
/// line is wider than the pane and wraps to multiple rows, causing the scroll
/// math to leave the last entries below the visible area.
fn display_rows(lines: &[Line<'_>], width: usize) -> u16 {
    if width == 0 {
        return lines.len() as u16;
    }
    lines
        .iter()
        .map(|line| {
            let cols: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            // A zero-width line still occupies one row (blank line).
            let rows = cols.max(1).div_ceil(width);
            rows as u16
        })
        .fold(0u16, |acc, r| acc.saturating_add(r))
}

fn dim(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(Color::DarkGray))
}

/// Number of visible columns the cursor is offset from the start of the input text.
fn visible_cursor_col(app: &App) -> u16 {
    app.input_buffer[..app.input_cursor].chars().count() as u16
}

fn build_lines(app: &App, _width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for entry in &app.conversation {
        match entry.event_type.as_str() {
            "agent:prompt_sent" => {
                let text = entry.line.as_deref().unwrap_or("");
                push_multiline(
                    &mut lines,
                    "you  ",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    text,
                    Style::default(),
                );
                lines.push(Line::default());
            }
            "agent:output" => {
                let text = entry.line.as_deref().unwrap_or("");
                if text.trim().is_empty() {
                    lines.push(Line::default());
                } else {
                    push_multiline(
                        &mut lines,
                        "     ",
                        Style::default(),
                        text,
                        Style::default(),
                    );
                }
            }
            "agent:tool_use" => {
                let tool_name = entry
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let input_preview = tool_input_preview(entry);
                lines.push(Line::from(vec![
                    Span::styled("tool ", Style::default().fg(Color::Magenta)),
                    Span::styled(
                        tool_name,
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if input_preview.is_empty() {
                            String::new()
                        } else {
                            format!("  {input_preview}")
                        },
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
            "agent:result" => {
                let text = entry.line.as_deref().unwrap_or("");
                if !text.trim().is_empty() {
                    push_multiline(
                        &mut lines,
                        "done ",
                        Style::default().fg(Color::Green),
                        text,
                        Style::default().fg(Color::DarkGray),
                    );
                }
            }
            "agent:thinking" => {
                let text = entry.line.as_deref().unwrap_or("");
                if !text.trim().is_empty() {
                    push_multiline(
                        &mut lines,
                        "think",
                        Style::default().fg(Color::DarkGray),
                        text,
                        Style::default().fg(Color::DarkGray),
                    );
                }
            }
            "agent:context_cleared" => {
                lines.push(Line::from(Span::styled(
                    "─── context cleared ───".to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            _ => {}
        }
    }

    lines
}

/// Push `text` as one or more ratatui `Line`s, splitting on embedded `\n`.
///
/// The `prefix` (e.g. `"you  "` or `"tool "`) is placed on the first line.
/// Continuation lines receive a matching indent of spaces so the text column
/// stays aligned.
fn push_multiline(
    lines: &mut Vec<Line<'static>>,
    prefix: &'static str,
    prefix_style: Style,
    text: &str,
    text_style: Style,
) {
    let indent = " ".repeat(prefix.chars().count());
    let mut iter = text.lines().peekable();

    // If the text is empty, emit one line with just the prefix.
    if iter.peek().is_none() {
        lines.push(Line::from(Span::styled(prefix, prefix_style)));
        return;
    }

    let mut first = true;
    for text_line in iter {
        let pfx: Span<'static> = if first {
            Span::styled(prefix, prefix_style)
        } else {
            Span::styled(indent.clone(), Style::default())
        };
        lines.push(Line::from(vec![pfx, Span::styled(text_line.to_string(), text_style)]));
        first = false;
    }
}

fn tool_input_preview(entry: &crate::app::ConversationEntry) -> String {
    let Some(meta) = &entry.metadata else { return String::new() };
    let input = meta.get("input");

    // Bash: show the command string directly.
    if let Some(cmd) = input.and_then(|i| i.get("command")).and_then(|v| v.as_str()) {
        return truncate(cmd, 80);
    }

    // Read / Write / Edit / NotebookEdit: show the file path.
    if let Some(path) = input.and_then(|i| i.get("file_path")).and_then(|v| v.as_str()) {
        return path.to_string();
    }

    // Everything else (MCP tools, etc.): compact JSON of the input object.
    if let Some(input_val) = input {
        if !input_val.is_null() {
            let s = serde_json::to_string(input_val).unwrap_or_default();
            return truncate(&s, 80);
        }
    }

    String::new()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Truncate at a char boundary.
        let end = s
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i < max.saturating_sub(1))
            .last()
            .unwrap_or(0);
        format!("{}…", &s[..end])
    }
}
