use crate::manager::app::ManagerApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

pub fn render(f: &mut Frame, app: &ManagerApp, area: Rect) {
    let rows: Vec<Row> = app
        .config_fields
        .iter()
        .enumerate()
        .map(|(i, (key, val))| {
            let key_style = Style::default().add_modifier(Modifier::BOLD);
            let val_style = if i == app.config_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(Color::White)
            };
            Row::new(vec![
                Cell::from(Span::styled(key.clone(), key_style)),
                Cell::from(Span::styled(val.clone(), val_style)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Length(24), Constraint::Min(20)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Configuration  (e edit  s save) "),
    );

    f.render_widget(table, area);

    if let Some(ref draft) = app.config_edit {
        if let Some((key, _)) = app.config_fields.get(app.config_selected) {
            render_edit_dialog(f, key, draft, area);
        }
    }
}

fn render_edit_dialog(f: &mut Frame, key: &str, draft: &str, area: Rect) {
    const W: u16 = 60;
    const H: u16 = 6;
    let x = area.x + area.width.saturating_sub(W) / 2;
    let y = area.y + area.height.saturating_sub(H) / 2;
    let dialog = Rect { x, y, width: W.min(area.width), height: H.min(area.height) };

    f.render_widget(Clear, dialog);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .margin(1)
        .split(dialog);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" Edit: {key} "));
    f.render_widget(block, dialog);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Value: ", Style::default().fg(Color::DarkGray)),
            Span::styled(draft, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Yellow)),
        ])),
        inner[0],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            "Enter confirm  Esc cancel",
            Style::default().fg(Color::DarkGray),
        )),
        inner[2],
    );
}
