use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let Some(wf) = &app.selected_workflow else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(6)])
        .split(area);

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

    let label = |s: &str| {
        Span::styled(format!("{s:<18}"), Style::default().add_modifier(Modifier::BOLD))
    };

    let lines = vec![
        Line::from(vec![label("ID"), Span::raw(wf.id.to_string())]),
        Line::from(vec![label("Name"), Span::raw(wf.name.clone())]),
        Line::from(vec![label("Agent"), Span::raw(agent_name)]),
        Line::from(vec![
            label("Trigger"),
            Span::raw(wf.trigger_config.trigger_type()),
        ]),
        Line::from(vec![
            label("Poll Interval"),
            Span::raw(format!("{}s", wf.poll_interval_secs)),
        ]),
        Line::from(vec![label("Enabled"), Span::styled(enabled_text, enabled_style)]),
        Line::from(vec![
            label("Created"),
            Span::raw(wf.created_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        ]),
        Line::from(vec![
            label("Updated"),
            Span::raw(wf.updated_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        ]),
    ];

    let info = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(format!(" Workflow: {} ", wf.name)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(info, chunks[0]);

    // --- Prompt template block ---
    let prompt = Paragraph::new(wf.prompt_template.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Prompt Template "),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(prompt, chunks[1]);
}
