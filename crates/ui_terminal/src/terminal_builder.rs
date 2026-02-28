use crate::terminal::{Terminal, TerminalListener};
use alacritty_terminal::{
    event::WindowSize,
    event_loop::EventLoop,
    grid::Dimensions,
    sync::FairMutex,
    term::{Config, Term},
    tty::{self, Options as PtyOptions, Shell},
};
use anyhow::{Context, Result};
use futures::channel::mpsc::unbounded;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

const DEFAULT_SCROLL_HISTORY_LINES: usize = 10_000;

#[derive(Clone, Copy)]
struct TermSize {
    pub num_lines: usize,
    pub num_cols: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.num_lines
    }

    fn screen_lines(&self) -> usize {
        self.num_lines
    }

    fn columns(&self) -> usize {
        self.num_cols
    }
}

impl From<WindowSize> for TermSize {
    fn from(size: WindowSize) -> Self {
        TermSize { num_lines: size.num_lines as usize, num_cols: size.num_cols as usize }
    }
}

pub struct TerminalBuilder {
    working_directory: Option<PathBuf>,
    env: HashMap<String, String>,
    scroll_history: usize,
    shell: Option<Shell>,
    window_size: WindowSize,
}

impl Default for TerminalBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalBuilder {
    pub fn new() -> Self {
        Self {
            working_directory: None,
            env: HashMap::new(),
            scroll_history: DEFAULT_SCROLL_HISTORY_LINES,
            shell: None,
            window_size: WindowSize {
                num_lines: 24,
                num_cols: 80,
                cell_width: 10,
                cell_height: 20,
            },
        }
    }

    pub fn working_directory(mut self, dir: PathBuf) -> Self {
        self.working_directory = Some(dir);
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn scroll_history(mut self, lines: usize) -> Self {
        self.scroll_history = lines;
        self
    }

    pub fn shell(mut self, program: String, args: Vec<String>) -> Self {
        self.shell = Some(Shell::new(program, args));
        self
    }

    pub fn tmux_attach(mut self, session_name: impl Into<String>) -> Self {
        let session = session_name.into();
        self.shell = Some(Shell::new(
            "tmux".to_string(),
            vec!["attach".to_string(), "-t".to_string(), session],
        ));
        self
    }

    pub fn window_size(mut self, size: WindowSize) -> Self {
        self.window_size = size;
        self
    }

    pub fn build(self) -> Result<Terminal> {
        let mut env = self.env;

        if std::env::var("LANG").is_err() {
            env.entry("LANG".to_string()).or_insert_with(|| "en_US.UTF-8".to_string());
        }

        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());

        let config = Config { scrolling_history: self.scroll_history, ..Config::default() };

        let (events_tx, events_rx) = unbounded();
        let listener = TerminalListener(events_tx);

        let term_size = TermSize::from(self.window_size);
        let term = Term::new(config, &term_size, listener.clone());
        let term = Arc::new(FairMutex::new(term));

        let pty_options = PtyOptions {
            shell: self.shell,
            working_directory: self.working_directory,
            drain_on_exit: true,
            env: env.into_iter().collect(),
            #[cfg(windows)]
            escape_args: false,
        };

        let pty = tty::new(&pty_options, self.window_size, 0).context("Failed to create PTY")?;

        let event_loop = EventLoop::new(term.clone(), listener, pty, false, false)
            .context("Failed to create event loop")?;
        let pty_tx = event_loop.channel();

        event_loop.spawn();

        Ok(Terminal::new(term, events_rx, pty_tx))
    }
}
