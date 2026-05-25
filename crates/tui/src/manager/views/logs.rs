use crate::manager::app::{LogSource, ManagerApp};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let source_label = match &app.log_source {
        LogSource::None => Span::styled(" no source set", Style::default().fg(Color::DarkGray)),
        LogSource::File(p) => Span::styled(format!(" {p}"), Style::default().fg(Color::Cyan)),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let source_block = Paragraph::new(source_label).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Source  (l to change) "),
    );
    f.render_widget(source_block, chunks[0]);

    let lines: Vec<Line> = app.log_lines.iter().map(|l| Line::from(Span::raw(l.clone()))).collect();

    let total = lines.len() as u16;
    let height = chunks[1].height.saturating_sub(2);
    let scroll = if total > height {
        let max_scroll = total - height;
        max_scroll.saturating_sub(app.log_scroll)
    } else {
        0
    };

    let log_block = Paragraph::new(lines)
        .block(
            Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Logs "),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(log_block, chunks[1]);

    if let Some(ref input) = app.log_source_input {
        render_source_input(f, input, area);
    }
}

fn render_source_input(f: &mut Frame, input: &str, area: Rect) {
    const W: u16 = 60;
    const H: u16 = 5;
    let x = area.x + area.width.saturating_sub(W) / 2;
    let y = area.y + area.height.saturating_sub(H) / 2;
    let dialog = Rect { x, y, width: W.min(area.width), height: H.min(area.height) };

    f.render_widget(Clear, dialog);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .margin(1)
        .split(dialog);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Log source path (empty = none) ");
    f.render_widget(block, dialog);

    f.render_widget(
        Paragraph::new(Span::styled(
            "Path: ",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        )),
        inner[0],
    );
    f.render_widget(
        Paragraph::new(Span::styled(input, Style::default().fg(Color::White))),
        inner[1],
    );
}
