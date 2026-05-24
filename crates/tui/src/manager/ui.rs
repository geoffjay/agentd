use super::app::{ManagerApp, ManagerView};
use super::views;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Tabs},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut ManagerApp) {
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
}

fn render_header(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(22)])
        .split(area);

    let title = Paragraph::new(Span::styled(
        " agentd manager",
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, cols[0]);

    let right = if let Some(ref err) = app.error {
        let text = if err.len() > 20 { format!("{}...", &err[..17]) } else { err.clone() };
        Span::styled(format!(" {text}  "), Style::default().fg(Color::Red))
    } else {
        let secs = app.secs_until_refresh();
        Span::styled(
            format!(" refresh in {secs}s  "),
            Style::default().fg(Color::DarkGray),
        )
    };

    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), cols[1]);
}

fn render_tabs(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let labels: Vec<Line> = app.tab_labels().into_iter().map(Line::from).collect();

    let tabs = Tabs::new(labels)
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

fn render_content(f: &mut Frame, app: &mut ManagerApp, area: Rect) {
    match app.view {
        ManagerView::Services => views::services::render(f, app, area),
        ManagerView::Logs => views::logs::render(f, app, area),
        ManagerView::Config => views::config::render(f, app, area),
        ManagerView::Metrics => views::metrics::render(f, app, area),
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

fn render_footer(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let hints = match &app.view {
        ManagerView::Services => " ↑/k up  ↓/j down  r refresh  Tab switch  q quit",
        ManagerView::Logs => {
            if app.log_source_input.is_some() {
                " Type path  Enter confirm  Esc cancel"
            } else {
                " l set source  ↑/k scroll up  ↓/j scroll down  r refresh  Tab switch  q quit"
            }
        }
        ManagerView::Config => {
            if app.config_edit.is_some() {
                " Type value  Enter confirm  Esc cancel"
            } else {
                " ↑/k up  ↓/j down  e edit field  s save  Tab switch  q quit"
            }
        }
        ManagerView::Metrics => {
            if app.metric_input_active {
                " Type PromQL  Enter run  Esc cancel"
            } else if app.query_picker.is_some() {
                " ↑/k up  ↓/j down  / filter  Enter select  Esc close"
            } else {
                " i input query  p picker  Tab switch  q quit"
            }
        }
    };

    f.render_widget(
        Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray))),
        area,
    );
}
