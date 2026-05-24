use crate::manager::app::{ManagerApp, ServiceState};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut ManagerApp, area: Rect) {
    let header_cells = ["Service", "URL", "Status", "Latency"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app
        .services
        .iter()
        .map(|svc| {
            let (status_text, status_style, latency_text) = match &svc.state {
                ServiceState::Up(ms) => (
                    "up",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    format!("{ms}ms"),
                ),
                ServiceState::Down(reason) => {
                    let reason = if reason.len() > 20 {
                        format!("{}...", &reason[..17])
                    } else {
                        reason.clone()
                    };
                    (
                        "down",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        reason,
                    )
                }
                ServiceState::Unknown => (
                    "unknown",
                    Style::default().fg(Color::DarkGray),
                    String::new(),
                ),
            };

            Row::new(vec![
                Cell::from(svc.name.clone()),
                Cell::from(Span::styled(svc.url.clone(), Style::default().fg(Color::DarkGray))),
                Cell::from(Span::styled(status_text, status_style)),
                Cell::from(Span::styled(latency_text, Style::default().fg(Color::DarkGray))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Min(24),
            Constraint::Length(9),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Services "),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, area, &mut app.service_table_state);
}
