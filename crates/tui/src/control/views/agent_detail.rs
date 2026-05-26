use crate::control::app::App;
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

    let inner_width = area.width.saturating_sub(2) as usize;
    let input_rows = crate::input::compute_input_rows(&app.input_buffer, inner_width);
    let input_box_height = input_rows.clamp(1, crate::input::MAX_INPUT_ROWS) + 2;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(input_box_height),
        ])
        .split(area);

    render_info(f, app, chunks[0]);
    render_conversation(f, app, chunks[1]);
    render_input(f, app, chunks[2]);
    // Cursor is rendered as a styled span inside the input lines; no
    // set_cursor_position call needed.
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
        Line::from(vec![dim("session: "), Span::raw(session)]),
        Line::from(vec![
            dim("dir: "),
            Span::raw(agent.config.working_dir.clone()),
            dim("  model: "),
            Span::raw(model),
        ]),
    ];

    let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(
        if agent.built_in {
            format!(" {} [system] ", agent.name)
        } else {
            format!(" {} ", agent.name)
        },
    );

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn render_conversation(f: &mut Frame, app: &mut App, area: Rect) {
    let inner_width = area.width.saturating_sub(2);
    let lines = build_lines(app, inner_width as usize);

    let count = app.conversation.len();
    let title =
        if count == 0 { " Conversation ".to_string() } else { format!(" Conversation ({count}) ") };

    let block =
        Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(title);

    // Build the paragraph once and ask ratatui itself for the wrapped row
    // count. The previous hand-rolled `display_rows` undercounted because it
    // estimated wrapping as `chars / width`, ignoring word-wrap behaviour
    // (long lines with no spaces, multi-byte/wide characters, etc.). On a
    // long conversation the undercount made `max_scroll` smaller than the
    // true tail, so follow mode silently stopped scrolling before the last
    // events and the visible bottom row was an older event.
    let para = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    let total_rows: u16 = para.line_count(inner_width).try_into().unwrap_or(u16::MAX);
    let visible = area.height.saturating_sub(2);
    let max_scroll = total_rows.saturating_sub(visible);

    let scroll =
        if app.conversation_follow { max_scroll } else { app.conversation_scroll.min(max_scroll) };
    app.conversation_scroll = scroll;

    let para = para.scroll((scroll, 0));
    f.render_widget(para, area);
}

fn render_input(f: &mut Frame, app: &mut App, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;

    // Expose inner_width so App::input_move_vertical can use the same geometry.
    app.input_inner_width = inner_width;

    // Keep cursor visible: adjust scroll so the cursor row stays in the viewport.
    if app.input_mode {
        let (cursor_row, _) =
            crate::input::cursor_visual_pos(&app.input_buffer, app.input_cursor, inner_width);
        let visible = area.height.saturating_sub(2);
        if cursor_row < app.input_scroll {
            app.input_scroll = cursor_row;
        }
        if visible > 0 && cursor_row >= app.input_scroll + visible {
            app.input_scroll = cursor_row - visible + 1;
        }
    } else {
        app.input_scroll = 0;
    }

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

    // White-block cursor is embedded as a styled span; no terminal cursor needed.
    let cursor_byte = if app.input_mode { Some(app.input_cursor) } else { None };
    let lines = if app.input_mode || !app.input_buffer.is_empty() {
        crate::input::build_input_display_lines(&app.input_buffer, cursor_byte, inner_width)
    } else {
        vec![]
    };

    let para = Paragraph::new(lines).block(block).scroll((app.input_scroll, 0));

    f.render_widget(para, area);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn dim(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(Color::DarkGray))
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
                    push_multiline(&mut lines, "     ", Style::default(), text, Style::default());
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

fn tool_input_preview(entry: &crate::control::app::ConversationEntry) -> String {
    let Some(meta) = &entry.metadata else { return String::new() };
    let input: Option<&serde_json::Value> = meta.get("input");

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
