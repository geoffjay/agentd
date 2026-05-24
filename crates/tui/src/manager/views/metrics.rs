use crate::manager::app::ManagerApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub fn render(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_query_bar(f, app, chunks[0]);
    render_results(f, app, chunks[1]);
}

fn render_query_bar(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let (query_style, border_style) = if app.metric_input_active {
        (
            Style::default().fg(Color::White),
            Style::default().fg(Color::Yellow),
        )
    } else {
        (
            Style::default().fg(Color::DarkGray),
            Style::default().fg(Color::DarkGray),
        )
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
                Cell::from(Span::styled(
                    s.timestamp.clone(),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Min(30), Constraint::Length(16), Constraint::Length(14)],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" Results ({}) ", app.metric_results.len())),
    );

    f.render_widget(table, area);
}
