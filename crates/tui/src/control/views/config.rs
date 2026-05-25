use crate::control::app::App;
use agentd_common::config::config_file_path;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let cfg = &app.agentd_config;
    let mut lines: Vec<Line<'static>> = Vec::new();

    let path_str = config_file_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown)".to_string());
    lines.push(kv_line("  file", path_str));
    lines.push(Line::from(""));

    section(&mut lines, "general");
    kv(&mut lines, "log_level", cfg.general.log_level.clone());
    kv(&mut lines, "log_format", cfg.general.log_format.clone());
    kv(&mut lines, "host", cfg.general.host.clone());
    lines.push(Line::from(""));

    section(&mut lines, "services.orchestrator");
    kv(&mut lines, "port", cfg.services.orchestrator.port.to_string());
    kv(&mut lines, "backend", cfg.services.orchestrator.backend.clone());
    kv(&mut lines, "communicate_url", cfg.services.orchestrator.communicate_url.clone());
    kv(
        &mut lines,
        "reconcile_interval_secs",
        cfg.services.orchestrator.reconcile_interval_secs.to_string(),
    );
    lines.push(Line::from(""));

    section(&mut lines, "services.memory");
    kv(&mut lines, "port", cfg.services.memory.port.to_string());
    kv(&mut lines, "embedding_provider", cfg.services.memory.embedding_provider.clone());
    kv(&mut lines, "embedding_model", cfg.services.memory.embedding_model.clone());
    kv(&mut lines, "lance_path", cfg.services.memory.lance_path.clone());
    lines.push(Line::from(""));

    section(&mut lines, "services.notify");
    kv(&mut lines, "port", cfg.services.notify.port.to_string());
    lines.push(Line::from(""));

    section(&mut lines, "services.ask");
    kv(&mut lines, "port", cfg.services.ask.port.to_string());
    kv(&mut lines, "orchestrator_url", cfg.services.ask.orchestrator_url.clone());
    lines.push(Line::from(""));

    section(&mut lines, "services.wrap");
    kv(&mut lines, "port", cfg.services.wrap.port.to_string());
    kv(&mut lines, "backend", cfg.services.wrap.backend.clone());
    lines.push(Line::from(""));

    section(&mut lines, "services.hook");
    kv(&mut lines, "port", cfg.services.hook.port.to_string());
    kv(&mut lines, "history_size", cfg.services.hook.history_size.to_string());
    kv(&mut lines, "notify_on_failure", cfg.services.hook.notify_on_failure.to_string());
    kv(&mut lines, "notify_on_long_running", cfg.services.hook.notify_on_long_running.to_string());
    kv(
        &mut lines,
        "long_running_threshold_ms",
        cfg.services.hook.long_running_threshold_ms.to_string(),
    );
    if let Some(ref url) = cfg.services.hook.notify_service_url.clone() {
        kv(&mut lines, "notify_service_url", url.clone());
    }
    lines.push(Line::from(""));

    section(&mut lines, "services.monitor");
    kv(&mut lines, "port", cfg.services.monitor.port.to_string());
    kv(
        &mut lines,
        "collection_interval_secs",
        cfg.services.monitor.collection_interval_secs.to_string(),
    );
    kv(
        &mut lines,
        "cpu_alert_threshold",
        format!("{:.0}%", cfg.services.monitor.cpu_alert_threshold),
    );
    kv(
        &mut lines,
        "memory_alert_threshold",
        format!("{:.0}%", cfg.services.monitor.memory_alert_threshold),
    );
    kv(
        &mut lines,
        "disk_alert_threshold",
        format!("{:.0}%", cfg.services.monitor.disk_alert_threshold),
    );
    lines.push(Line::from(""));

    section(&mut lines, "services.communicate");
    kv(&mut lines, "port", cfg.services.communicate.port.to_string());
    lines.push(Line::from(""));

    section(&mut lines, "services.core");
    kv(&mut lines, "port", cfg.services.core.port.to_string());
    lines.push(Line::from(""));

    section(&mut lines, "services.mcp");
    kv(&mut lines, "orchestrator_url", cfg.services.mcp.orchestrator_url.clone());
    kv(&mut lines, "notify_url", cfg.services.mcp.notify_url.clone());
    kv(&mut lines, "ask_url", cfg.services.mcp.ask_url.clone());
    kv(&mut lines, "memory_url", cfg.services.mcp.memory_url.clone());
    kv(&mut lines, "communicate_url", cfg.services.mcp.communicate_url.clone());
    kv(&mut lines, "wrap_url", cfg.services.mcp.wrap_url.clone());
    kv(&mut lines, "monitor_url", cfg.services.mcp.monitor_url.clone());
    kv(&mut lines, "hook_url", cfg.services.mcp.hook_url.clone());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Configuration ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let content_height = inner.height as usize;
    let max_scroll = lines.len().saturating_sub(content_height);
    if app.config_scroll as usize > max_scroll {
        app.config_scroll = max_scroll as u16;
    }

    f.render_widget(Paragraph::new(lines).scroll((app.config_scroll, 0)), inner);
}

fn section(lines: &mut Vec<Line<'static>>, name: &'static str) {
    lines.push(Line::from(vec![
        Span::styled("  [", Style::default().fg(Color::DarkGray)),
        Span::styled(name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("]", Style::default().fg(Color::DarkGray)),
    ]));
}

fn kv(lines: &mut Vec<Line<'static>>, key: &'static str, value: String) {
    lines.push(kv_line(key, value));
}

fn kv_line(key: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("    {:<28}", key), Style::default().fg(Color::DarkGray)),
        Span::styled(value, Style::default().fg(Color::White)),
    ])
}
