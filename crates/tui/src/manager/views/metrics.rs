use crate::manager::app::ManagerApp;
use crate::manager::queries::{self, PREDEFINED_QUERIES};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

pub fn render(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_query_bar(f, app, chunks[0]);
    render_results(f, app, chunks[1]);

    if app.query_picker.is_some() {
        render_query_picker(f, app, area);
    }
}

fn render_query_bar(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let (query_style, border_style) = if app.metric_input_active {
        (Style::default().fg(Color::White), Style::default().fg(Color::Yellow))
    } else {
        (Style::default().fg(Color::DarkGray), Style::default().fg(Color::DarkGray))
    };

    let mut spans = vec![
        Span::styled("PromQL: ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Span::styled(&app.metric_query, query_style),
    ];
    if app.metric_input_active {
        spans.push(Span::styled("█", Style::default().fg(Color::Yellow)));
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .title(" Query  (i to edit, Enter to run) "),
    );
    f.render_widget(p, area);
}

fn render_results(f: &mut Frame, app: &ManagerApp, area: Rect) {
    if let Some(ref err) = app.metric_error {
        let p = Paragraph::new(Span::styled(err, Style::default().fg(Color::Red))).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Results "),
        );
        f.render_widget(p, area);
        return;
    }

    if app.metric_results.is_empty() {
        let msg = if app.metric_query.is_empty() {
            "Press i to enter a PromQL query"
        } else {
            "No results"
        };
        let p = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Results "),
        );
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(
        ["Metric / Labels", "Value", "Timestamp"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD))),
    )
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .metric_results
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(Span::styled(s.metric.clone(), Style::default().fg(Color::Cyan))),
                Cell::from(Span::styled(
                    s.value.clone(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(s.timestamp.clone(), Style::default().fg(Color::DarkGray))),
            ])
        })
        .collect();

    let table =
        Table::new(rows, [Constraint::Min(30), Constraint::Length(16), Constraint::Length(14)])
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(format!(" Results ({}) ", app.metric_results.len())),
            );

    f.render_widget(table, area);
}

fn render_query_picker(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let Some(picker) = &app.query_picker else { return };

    let filtered = queries::filtered_indices(&picker.filter);

    // Dialog sized to roughly 80% of the area, capped.
    let w: u16 = area.width.saturating_sub(6).clamp(50, 90);
    let h: u16 = area.height.saturating_sub(4).clamp(10, 24);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let dialog = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, dialog);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" PromQL Queries ({}) ", filtered.len()));
    f.render_widget(block, dialog);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // filter
            Constraint::Min(0),    // list
            Constraint::Length(1), // hint
        ])
        .margin(1)
        .split(dialog);

    // Filter row
    let filter_prefix = if picker.filter_active {
        Span::styled("Filter: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("Filter: ", Style::default().fg(Color::DarkGray))
    };
    let mut filter_spans =
        vec![filter_prefix, Span::styled(&picker.filter, Style::default().fg(Color::White))];
    if picker.filter_active {
        filter_spans.push(Span::styled("█", Style::default().fg(Color::Yellow)));
    } else if picker.filter.is_empty() {
        filter_spans
            .push(Span::styled("(press / to filter)", Style::default().fg(Color::DarkGray)));
    }
    f.render_widget(Paragraph::new(Line::from(filter_spans)), inner[0]);

    // List rows — scroll to keep cursor visible
    let list_h = inner[1].height as usize;
    let scroll_start = if filtered.is_empty() || list_h == 0 {
        0
    } else if picker.cursor >= list_h {
        picker.cursor - list_h + 1
    } else {
        0
    };
    let visible = filtered.iter().enumerate().skip(scroll_start).take(list_h);

    let rows: Vec<Row> = visible
        .map(|(visible_idx, &q_idx)| {
            let q = &PREDEFINED_QUERIES[q_idx];
            let selected = visible_idx == picker.cursor;
            let row_style = if selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(Span::styled(q.category, Style::default().fg(Color::DarkGray))),
                Cell::from(Span::styled(q.name, Style::default().fg(Color::White))),
                Cell::from(Span::styled(q.query, Style::default().fg(Color::DarkGray))),
            ])
            .style(row_style)
        })
        .collect();

    let table =
        Table::new(rows, [Constraint::Length(14), Constraint::Length(32), Constraint::Min(20)]);
    f.render_widget(table, inner[1]);

    // Hint row
    let hint = if picker.filter_active {
        " typing filter  Enter/Esc done"
    } else {
        " ↑/k up  ↓/j down  / filter  Enter select  Esc close"
    };
    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray))),
        inner[2],
    );
}
