use crate::control::app::App;
use orchestrator::types::{ActivityState, AgentStatus};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["Name", "Status", "Activity", "Backend", "Working Dir"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.agents.iter().map(|agent| {
        let status_cell = Cell::from(agent.status.to_string()).style(status_color(&agent.status));

        let activity = match agent.activity {
            ActivityState::Busy => {
                Span::styled("busy", Style::default().fg(Color::Yellow))
            }
            ActivityState::Idle => Span::styled("idle", Style::default().fg(Color::DarkGray)),
        };

        let backend = agent.backend_type.as_deref().unwrap_or("-");
        let working_dir = truncate(&agent.config.working_dir, 35);

        let name_cell = if agent.built_in {
            Cell::from(Line::from(vec![
                Span::styled("[sys] ", Style::default().fg(Color::DarkGray)),
                Span::raw(agent.name.clone()),
            ]))
        } else {
            Cell::from(agent.name.clone())
        };

        Row::new(vec![
            name_cell,
            status_cell,
            Cell::from(Line::from(activity)),
            Cell::from(backend),
            Cell::from(working_dir),
        ])
        .height(1)
    });

    let widths = [
        Constraint::Percentage(22),
        Constraint::Length(9),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Min(10),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Agents ");

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, area, &mut app.agent_table_state);
}

fn status_color(status: &AgentStatus) -> Style {
    match status {
        AgentStatus::Running => Style::default().fg(Color::Green),
        AgentStatus::Pending => Style::default().fg(Color::Yellow),
        AgentStatus::Failed => Style::default().fg(Color::Red),
        AgentStatus::Stopped => Style::default().fg(Color::DarkGray),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
