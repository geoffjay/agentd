mod app;
mod config;
mod event;
mod input;
mod stream;
mod ui;
mod views;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event::{Event, EventHandler};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    let config = config::TuiConfig::from_env();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(config);
    let result = run(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableBracketedPaste)?;
    terminal.show_cursor()?;

    result
}

async fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> Result<()> {
    let mut events = EventHandler::new(250);

    app.refresh().await;

    loop {
        terminal.draw(|f| ui::render(f, &mut app))?;

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
