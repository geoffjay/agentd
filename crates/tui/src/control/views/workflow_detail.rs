use crate::control::app::{App, WorkflowFocus};
use orchestrator::scheduler::types::DispatchStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    if app.selected_workflow.is_none() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // info
            Constraint::Length(32), // prompt template (30 lines + 2 borders)
            Constraint::Min(4),     // dispatch history
        ])
        .split(area);

    render_info(f, app, chunks[0]);
    render_template(f, app, chunks[1]);
    render_dispatches(f, app, chunks[2]);
}

fn render_info(f: &mut Frame, app: &App, area: Rect) {
    let Some(wf) = &app.selected_workflow else {
        return;
    };

    let agent_name = app
        .agents
        .iter()
        .find(|a| a.id == wf.agent_id)
        .map(|a| a.name.clone())
        .unwrap_or_else(|| wf.agent_id.to_string());

    let (enabled_text, enabled_style) = if wf.enabled {
        ("yes", Style::default().fg(Color::Green))
    } else {
        ("no", Style::default().fg(Color::DarkGray))
    };

    let lbl = |s: &str| -> Span<'static> {
        Span::styled(
            format!("{s:<16}"),
            Style::default().fg(Color::DarkGray),
        )
    };

    let lines = vec![
        Line::from(vec![lbl("name"), Span::raw(wf.name.clone())]),
        Line::from(vec![lbl("id"), Span::raw(wf.id.to_string())]),
        Line::from(vec![lbl("agent"), Span::raw(agent_name)]),
        Line::from(vec![
            lbl("trigger"),
            Span::raw(wf.trigger_config.trigger_type()),
        ]),
        Line::from(vec![
            lbl("poll interval"),
            Span::raw(format!("{}s", wf.poll_interval_secs)),
        ]),
        Line::from(vec![
            lbl("enabled"),
            Span::styled(enabled_text, enabled_style),
        ]),
        Line::from(vec![
            lbl("created"),
            Span::raw(wf.created_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        ]),
        Line::from(vec![
            lbl("updated"),
            Span::raw(wf.updated_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", wf.name));

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_template(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(wf) = &app.selected_workflow.clone() else {
        return;
    };

    let focused = app.workflow_focus == WorkflowFocus::Template;

    // Count total lines and clamp scroll.
    let total_lines = wf.prompt_template.lines().count() as u16;
    let visible = area.height.saturating_sub(2);
    let max_scroll = total_lines.saturating_sub(visible);
    app.workflow_template_scroll = app.workflow_template_scroll.min(max_scroll);

    let (border_style, title) = if focused {
        (
            Style::default().fg(Color::Cyan),
            " Prompt Template (↑/↓ scroll  t unfocus) ",
        )
    } else {
        (
            Style::default(),
            " Prompt Template (t to focus) ",
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(title);

    let para = Paragraph::new(wf.prompt_template.clone())
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.workflow_template_scroll, 0));

    f.render_widget(para, area);
}

fn render_dispatches(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.workflow_focus == WorkflowFocus::Dispatches;

    let (border_style, title) = if focused {
        (
            Style::default().fg(Color::Cyan),
            " Dispatch History (↑/↓ scroll  d unfocus) ",
        )
    } else {
        (
            Style::default(),
            " Dispatch History (d to focus) ",
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(title);

    if app.workflow_dispatches.is_empty() {
        let para = Paragraph::new(Span::styled(
            " no dispatches yet",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        f.render_widget(para, area);
        return;
    }

    // Sort most-recent first.
    let mut dispatches = app.workflow_dispatches.clone();
    dispatches.sort_by(|a, b| b.dispatched_at.cmp(&a.dispatched_at));

    let visible_rows = area.height.saturating_sub(4) as usize; // borders + header + margin
    let max_scroll = dispatches.len().saturating_sub(visible_rows);
    app.workflow_dispatch_scroll = (app.workflow_dispatch_scroll as usize).min(max_scroll) as u16;
    let offset = app.workflow_dispatch_scroll as usize;

    let header = Row::new(["Source", "Status", "Dispatched", "Duration"].map(|h| {
        Cell::from(h).style(Style::default().add_modifier(Modifier::BOLD))
    }))
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = dispatches
        .iter()
        .skip(offset)
        .take(visible_rows)
        .map(|d| {
            let source = truncate(&d.source_id, 24);
            let status_span = status_cell(&d.status);
            let dispatched = d.dispatched_at.format("%Y-%m-%d %H:%M").to_string();
            let duration = match d.completed_at {
                Some(done) => {
                    let secs = (done - d.dispatched_at).num_seconds();
                    if secs < 60 {
                        format!("{secs}s")
                    } else {
                        format!("{}m {}s", secs / 60, secs % 60)
                    }
                }
                None => match d.status {
                    DispatchStatus::Pending | DispatchStatus::Dispatched => "running".to_string(),
                    _ => "-".to_string(),
                },
            };

            Row::new(vec![
                Cell::from(source),
                status_span,
                Cell::from(dispatched),
                Cell::from(duration),
            ])
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Percentage(28),
        Constraint::Percentage(14),
        Constraint::Percentage(28),
        Constraint::Percentage(30),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    f.render_widget(table, area);
}

fn status_cell(status: &DispatchStatus) -> Cell<'static> {
    let (text, color) = match status {
        DispatchStatus::Pending => ("pending", Color::Yellow),
        DispatchStatus::Dispatched => ("dispatched", Color::Cyan),
        DispatchStatus::Completed => ("completed", Color::Green),
        DispatchStatus::Failed => ("failed", Color::Red),
        DispatchStatus::Skipped => ("skipped", Color::DarkGray),
    };
    Cell::from(text).style(Style::default().fg(color))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i < max.saturating_sub(1))
            .last()
            .unwrap_or(0);
        format!("{}…", &s[..end])
    }
}
