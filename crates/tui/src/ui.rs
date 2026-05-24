use crate::app::{App, View, WorkflowFocus};
use crate::views;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Tabs},
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

    if app.quitting {
        render_quit_dialog(f, area);
    }

    if app.memory_dialog.is_open() {
        views::memories::render_dialog(f, app, area);
    }
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
        Line::from(format!(" Memories ({}) ", app.memories.len())),
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
        View::MemoryList | View::MemoryDetail => views::memories::render(f, app, area),
        View::Config => views::config::render(f, app, area),
    }
}

fn render_quit_dialog(f: &mut Frame, area: Rect) {
    const W: u16 = 36;
    const H: u16 = 5;
    let x = area.x + area.width.saturating_sub(W) / 2;
    let y = area.y + area.height.saturating_sub(H) / 2;
    let dialog = Rect { x, y, width: W.min(area.width), height: H.min(area.height) };

    f.render_widget(Clear, dialog);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Quit? ");

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" confirm    "),
            Span::styled("any key", Style::default().fg(Color::DarkGray)),
            Span::raw(" cancel"),
        ]),
    ];

    f.render_widget(Paragraph::new(text).block(block), dialog);
}

fn render_footer(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let hints: &str = if app.input_mode {
        " Enter send  Esc cancel  ←/→ move cursor"
    } else {
        match app.view {
            View::AgentList | View::WorkflowList => {
                " ↑/k up  ↓/j down  Enter detail  Tab/S-Tab switch  r refresh  q quit"
            }
            View::AgentDetail => " i input  ↑/k scroll up  ↓/j scroll down  Esc back  q quit",
            View::WorkflowDetail => match app.workflow_focus {
                WorkflowFocus::Template => " ↑/↓ scroll template  t unfocus  q quit",
                WorkflowFocus::Dispatches => " ↑/↓ scroll history  d unfocus  q quit",
                WorkflowFocus::None => " t template  d dispatches  Esc back  q quit",
            },
            View::MemoryList => {
                let has_filter = app.memory_search.is_some() || !app.memory_tag_filter.is_empty();
                if has_filter {
                    " s search  t tags  Esc clear  ↑/k up  ↓/j down  Enter detail  r refresh  q quit"
                } else {
                    " s search  t tags  ↑/k up  ↓/j down  Enter detail  Tab/S-Tab switch  r refresh  q quit"
                }
            }
            View::MemoryDetail => " ↑/k scroll up  ↓/j scroll down  Esc back  q quit",
            View::Config => " ↑/k scroll up  ↓/j scroll down  c/Esc close  q quit",
        }
    };

    let p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}
