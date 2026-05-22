use anyhow::Result;
use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEvent};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::time;

pub enum Event {
    Key(KeyEvent),
    Paste(String),
    Tick,
}

pub struct EventHandler {
    reader: EventStream,
    tick_interval: time::Interval,
}

impl EventHandler {
    pub fn new(tick_rate_ms: u64) -> Self {
        Self {
            reader: EventStream::new(),
            tick_interval: time::interval(Duration::from_millis(tick_rate_ms)),
        }
    }

    pub async fn next(&mut self) -> Result<Event> {
        tokio::select! {
            event = self.reader.next() => {
                match event {
                    Some(Ok(CrosstermEvent::Key(key))) => Ok(Event::Key(key)),
                    Some(Ok(CrosstermEvent::Paste(text))) => Ok(Event::Paste(text)),
                    Some(Ok(_)) => Ok(Event::Tick),
                    Some(Err(e)) => Err(e.into()),
                    None => Err(anyhow::anyhow!("event stream closed")),
                }
            }
            _ = self.tick_interval.tick() => Ok(Event::Tick),
        }
    }
}
