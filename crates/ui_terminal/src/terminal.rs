use alacritty_terminal::{
    event::{Event as AlacTermEvent, EventListener, WindowSize},
    event_loop::EventLoopSender,
    sync::FairMutex,
    term::Term,
};
use anyhow::{Context, Result};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    TitleChanged,
    Wakeup,
}

#[derive(Clone)]
pub struct TerminalListener(pub UnboundedSender<AlacTermEvent>);

impl EventListener for TerminalListener {
    fn send_event(&self, event: AlacTermEvent) {
        self.0.unbounded_send(event).ok();
    }
}

pub struct Terminal {
    term: Arc<FairMutex<Term<TerminalListener>>>,
    events_rx: UnboundedReceiver<AlacTermEvent>,
    pty_tx: EventLoopSender,
}

impl Terminal {
    pub(crate) fn new(
        term: Arc<FairMutex<Term<TerminalListener>>>,
        events_rx: UnboundedReceiver<AlacTermEvent>,
        pty_tx: EventLoopSender,
    ) -> Self {
        Self { term, events_rx, pty_tx }
    }

    pub fn input(&self, data: impl Into<Vec<u8>>) -> Result<()> {
        use alacritty_terminal::event_loop::Msg;

        let bytes = data.into();
        self.pty_tx.send(Msg::Input(bytes.into())).context("Failed to send input to terminal")
    }

    pub fn resize(&self, size: WindowSize) -> Result<()> {
        use alacritty_terminal::event_loop::Msg;

        self.pty_tx.send(Msg::Resize(size)).context("Failed to send resize to terminal")
    }

    pub fn read_content(&self) -> Result<String> {
        let term = self.term.lock();
        let content = term.renderable_content();

        let mut output = String::new();
        for cell in content.display_iter {
            output.push(cell.c);
        }

        Ok(output)
    }

    pub fn poll_events(&mut self) -> Vec<TerminalEvent> {
        let mut events = Vec::new();

        while let Ok(Some(event)) = self.events_rx.try_next() {
            match event {
                AlacTermEvent::Title(_) => {
                    events.push(TerminalEvent::TitleChanged);
                }
                AlacTermEvent::Wakeup => {
                    events.push(TerminalEvent::Wakeup);
                }
                _ => {}
            }
        }

        events
    }

    pub fn term(&self) -> &Arc<FairMutex<Term<TerminalListener>>> {
        &self.term
    }
}
