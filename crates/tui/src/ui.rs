use crate::app::{App, View};
use crate::views;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Tabs},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(3), // tabs
            Constraint::Min(0),    // content
            Constraint::Length(1), // footer
        ])
        .split(area);

    render_header(f, app, root[0]);
    render_tabs(f, app, root[1]);
    render_content(f, app, root[2]);
    render_footer(f, app, root[3]);
}

fn render_header(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(20)])
        .split(area);

    let title = Paragraph::new(Span::styled(
        " agentd tui",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, cols[0]);

    let status = if app.loading {
        Span::styled(" loading...  ", Style::default().fg(Color::Yellow))
    } else {
        let secs = app.secs_until_refresh();
        Span::styled(
            format!(" refresh in {secs}s  "),
            Style::default().fg(Color::DarkGray),
        )
    };

    if let Some(ref err) = app.error {
        let err_text = if err.len() > 18 { format!("{}...", &err[..15]) } else { err.clone() };
        let error_line = Paragraph::new(Span::styled(
            format!(" {err_text}"),
            Style::default().fg(Color::Red),
        ))
        .alignment(Alignment::Right);
        f.render_widget(error_line, cols[1]);
    } else {
        let p = Paragraph::new(status).alignment(Alignment::Right);
        f.render_widget(p, cols[1]);
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let tab_titles = vec![
        Line::from(format!(" Agents ({}) ", app.agents.len())),
        Line::from(format!(" Workflows ({}) ", app.workflows.len())),
    ];

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(app.active_tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    f.render_widget(tabs, area);
}

fn render_content(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    match app.view {
        View::AgentList => views::agents::render(f, app, area),
        View::AgentDetail => views::agent_detail::render(f, app, area),
        View::WorkflowList => views::workflows::render(f, app, area),
        View::WorkflowDetail => views::workflow_detail::render(f, app, area),
    }
}

fn render_footer(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let hints = if app.input_mode {
        " Enter send  Esc cancel  ←/→ move cursor"
    } else {
        match app.view {
            View::AgentList | View::WorkflowList => {
                " ↑/k up  ↓/j down  Enter detail  Tab/S-Tab switch  r refresh  q quit"
            }
            View::AgentDetail => " i input  ↑/k scroll up  ↓/j scroll down  Esc back  q quit",
            View::WorkflowDetail => " Esc back  q quit",
        }
    };

    let p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}
