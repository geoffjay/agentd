pub mod control;
mod manager;
pub mod config;
pub mod event;
pub mod input;

use anyhow::Result;
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event::{Event, EventHandler};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

pub async fn run_control() -> Result<()> {
    let agentd_cfg = agentd_common::config::load().unwrap_or_default();
    let config = config::TuiConfig::from_agentd_config(agentd_cfg);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = control::app::App::new(config);
    let result = run_control_loop(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableBracketedPaste)?;
    terminal.show_cursor()?;

    result
}

async fn run_control_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: control::app::App,
) -> Result<()> {
    let mut events = EventHandler::new(250);

    app.refresh().await;

    loop {
        terminal.draw(|f| control::ui::render(f, &mut app))?;

        match events.next().await? {
            Event::Key(key) => {
                if app.handle_key(key).await {
                    break;
                }
            }
            Event::Paste(text) => {
                app.handle_paste(text).await;
            }
            Event::Tick => {
                app.tick().await;
            }
        }
    }

    Ok(())
}

pub async fn run_manager() -> Result<()> {
    let agentd_cfg = agentd_common::config::load().unwrap_or_default();
    let config = config::TuiConfig::from_agentd_config(agentd_cfg);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = manager::app::ManagerApp::new(config).await;
    let result = run_manager_loop(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableBracketedPaste)?;
    terminal.show_cursor()?;

    result
}

async fn run_manager_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: manager::app::ManagerApp,
) -> Result<()> {
    let mut events = EventHandler::new(250);

    app.refresh().await;

    loop {
        terminal.draw(|f| manager::ui::render(f, &mut app))?;

        match events.next().await? {
            Event::Key(key) => {
                if app.handle_key(key).await {
                    break;
                }
            }
            Event::Paste(_) => {}
            Event::Tick => {
                app.tick().await;
            }
        }
    }

    Ok(())
}
