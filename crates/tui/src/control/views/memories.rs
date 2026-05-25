use crate::control::app::{App, MemoryDialog, View};
use chrono::{DateTime, Utc};
use memory::types::MemoryType;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    match app.view {
        View::MemoryList => render_list(f, app, area),
        View::MemoryDetail => render_detail(f, app, area),
        _ => {}
    }
}

/// Render the active memory dialog (search or tag filter) as a centered overlay.
/// Called from ui.rs with the full terminal area so dialogs float above everything.
pub fn render_dialog(f: &mut Frame, app: &App, area: Rect) {
    match &app.memory_dialog {
        MemoryDialog::None => {}
        MemoryDialog::Search(_) => render_search_dialog(f, app, area),
        MemoryDialog::TagFilter { .. } => render_tag_dialog(f, app, area),
    }
}

// ── List view ─────────────────────────────────────────────────────────────────

fn render_list(f: &mut Frame, app: &mut App, area: Rect) {
    let mut title = " Memories ".to_string();
    if let Some(ref q) = app.memory_search {
        title.push_str(&format!("[search: {}] ", truncate(q, 20)));
    }
    if !app.memory_tag_filter.is_empty() {
        title.push_str(&format!("[tags: {}] ", truncate(&app.memory_tag_filter.join(", "), 30)));
    }

    if app.memories.is_empty() {
        let msg = if app.loading {
            "Loading..."
        } else {
            "No memories found.  r refresh  s search  t filter by tags"
        };
        let block =
            Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(title);
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))).block(block),
            area,
        );
        return;
    }

    let header_cells = ["Type", "Tags", "Created By", "Date", "Content"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.memories.iter().map(|m| {
        let (badge, badge_style) = type_badge_style(&m.memory_type);

        let tags =
            if m.tags.is_empty() { "-".to_string() } else { truncate(&m.tags.join(", "), 26) };

        let created_by = truncate(&m.created_by, 16);
        let date = format_date_short(m.created_at);
        let content = truncate(&m.content.replace('\n', " "), 60);

        Row::new(vec![
            Cell::from(badge).style(badge_style),
            Cell::from(tags),
            Cell::from(created_by),
            Cell::from(date).style(Style::default().fg(Color::DarkGray)),
            Cell::from(content),
        ])
        .height(1)
    });

    let widths = [
        Constraint::Length(5),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Length(12),
        Constraint::Min(20),
    ];

    let block =
        Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(title);

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, area, &mut app.memory_table_state);
}

// ── Detail view ───────────────────────────────────────────────────────────────

fn render_detail(f: &mut Frame, app: &App, area: Rect) {
    let Some(ref m) = app.selected_memory else {
        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Memory Detail "),
            area,
        );
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(0)])
        .split(area);

    // Bind temporaries so they live long enough for the Line borrows.
    let tags_str = if m.tags.is_empty() { "-".to_string() } else { m.tags.join(", ") };
    let refs_str = if m.references.is_empty() {
        "-".to_string()
    } else {
        truncate(&m.references.join(", "), 60)
    };
    let type_str = m.memory_type.to_string();
    let vis_str = m.visibility.to_string();
    let created_str = format_date_long(m.created_at);
    let updated_str = format_date_long(m.updated_at);

    let meta_lines = vec![
        meta_row("ID", &m.id),
        meta_row("Type", &type_str),
        meta_row("Tags", &tags_str),
        meta_row("Created By", &m.created_by),
        meta_row("Visibility", &vis_str),
        meta_row("Created", &created_str),
        meta_row("Updated", &updated_str),
        meta_row("References", &refs_str),
    ];

    f.render_widget(
        Paragraph::new(meta_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Memory Detail "),
        ),
        chunks[0],
    );

    // Content (scrollable)
    f.render_widget(
        Paragraph::new(m.content.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Content "),
            )
            .wrap(Wrap { trim: false })
            .scroll((app.memory_scroll, 0)),
        chunks[1],
    );
}

// ── Search dialog ─────────────────────────────────────────────────────────────

fn render_search_dialog(f: &mut Frame, app: &App, area: Rect) {
    let input = match &app.memory_dialog {
        MemoryDialog::Search(s) => s,
        _ => return,
    };

    const W: u16 = 60;
    const H: u16 = 7;
    let x = area.x + area.width.saturating_sub(W) / 2;
    let y = area.y + area.height.saturating_sub(H) / 2;
    let dialog = Rect { x, y, width: W.min(area.width), height: H.min(area.height) };

    f.render_widget(Clear, dialog);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(format!("  > {}_", input), Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" search  "),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::raw(" cancel"),
        ]),
    ];

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Search Memories "),
        ),
        dialog,
    );
}

// ── Tag filter dialog ─────────────────────────────────────────────────────────

fn render_tag_dialog(f: &mut Frame, app: &App, area: Rect) {
    let (cursor, draft) = match &app.memory_dialog {
        MemoryDialog::TagFilter { cursor, draft } => (*cursor, draft),
        _ => return,
    };

    let tags = &app.memory_available_tags;

    const MAX_VISIBLE: usize = 12;
    const W: u16 = 52;

    let visible_count = MAX_VISIBLE.min(tags.len().max(1));
    // 2 borders + 1 empty top + visible rows + 1 empty bottom + 1 hint row
    let h = (visible_count as u16) + 5;
    let h = h.min(area.height.saturating_sub(4)).max(7);

    let x = area.x + area.width.saturating_sub(W) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let dialog = Rect { x, y, width: W.min(area.width), height: h };

    f.render_widget(Clear, dialog);

    let hint = Line::from(vec![
        Span::raw("  "),
        Span::styled("Space", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" toggle  "),
        Span::styled("a", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" all  "),
        Span::styled("c", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" clear  "),
        Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" apply  "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw(" cancel"),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Filter by Tags ");

    if tags.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No tags found. Load memories first (r).",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            hint,
        ];
        f.render_widget(Paragraph::new(lines).block(block), dialog);
        return;
    }

    // Scroll so the cursor stays visible in the window
    let inner_h = h.saturating_sub(2) as usize;
    let available_rows = inner_h.saturating_sub(3); // subtract empty + hint + empty
    let scroll_off = if cursor >= available_rows { cursor - available_rows + 1 } else { 0 };

    let mut lines = vec![Line::from("")];

    for (i, tag) in tags.iter().enumerate() {
        if i < scroll_off {
            continue;
        }
        if lines.len() > available_rows {
            break;
        }

        let is_selected = draft.contains(tag);
        let is_cursor = i == cursor;

        let checkbox = if is_selected { "[x]" } else { "[ ]" };
        let checkbox_style = if is_selected {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let tag_style = if is_cursor {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(checkbox, checkbox_style),
            Span::raw(" "),
            Span::styled(tag.clone(), tag_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(hint);

    f.render_widget(Paragraph::new(lines).block(block), dialog);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn meta_row<'a>(label: &'static str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {:<12}", label),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
        Span::raw(value.to_string()),
    ])
}

fn type_badge_style(t: &MemoryType) -> (&'static str, Style) {
    match t {
        MemoryType::Information => ("info ", Style::default().fg(Color::Cyan)),
        MemoryType::Question => ("quest", Style::default().fg(Color::Yellow)),
        MemoryType::Request => ("req  ", Style::default().fg(Color::Magenta)),
    }
}

fn format_date_short(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

fn format_date_long(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max.saturating_sub(3)).collect::<String>())
    }
}
