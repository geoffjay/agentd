use crate::app::App;
use orchestrator::scheduler::types::TriggerConfig;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["Name", "Agent", "Trigger", "Interval", "Enabled"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.workflows.iter().map(|wf| {
        let agent_name = app
            .agents
            .iter()
            .find(|a| a.id == wf.agent_id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| wf.agent_id.to_string()[..8].to_string());

        let trigger = trigger_label(&wf.trigger_config);

        let interval = format!("{}s", wf.poll_interval_secs);

        let (enabled_text, enabled_style) = if wf.enabled {
            ("yes", Style::default().fg(Color::Green))
        } else {
            ("no", Style::default().fg(Color::DarkGray))
        };

        Row::new(vec![
            Cell::from(wf.name.clone()),
            Cell::from(agent_name),
            Cell::from(trigger),
            Cell::from(interval),
            Cell::from(enabled_text).style(enabled_style),
        ])
        .height(1)
    });

    let widths = [
        Constraint::Percentage(28),
        Constraint::Percentage(22),
        Constraint::Percentage(22),
        Constraint::Length(8),
        Constraint::Length(7),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Workflows ");

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, area, &mut app.workflow_table_state);
}

fn trigger_label(config: &TriggerConfig) -> &'static str {
    match config {
        TriggerConfig::GithubIssues { .. } => "github-issues",
        TriggerConfig::GithubPullRequests { .. } => "github-prs",
        TriggerConfig::GitlabIssues { .. } => "gitlab-issues",
        TriggerConfig::GitlabMergeRequests { .. } => "gitlab-mrs",
        TriggerConfig::LinearIssues { .. } => "linear-issues",
        TriggerConfig::Webhook { .. } => "webhook",
        TriggerConfig::Cron { .. } => "cron",
        TriggerConfig::Delay { .. } => "delay",
        TriggerConfig::Manual { .. } => "manual",
        TriggerConfig::AgentLifecycle { .. } => "agent-lifecycle",
        TriggerConfig::AgentIdle { .. } => "agent-idle",
        TriggerConfig::DispatchResult { .. } => "dispatch-result",
        TriggerConfig::Composite { .. } => "composite",
        TriggerConfig::AskResponse { .. } => "ask-response",
        TriggerConfig::Queue { .. } => "queue",
    }
}
