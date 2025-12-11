mod terminal;
mod terminal_builder;

pub use alacritty_terminal;
pub use terminal::{Terminal, TerminalEvent};
pub use terminal_builder::TerminalBuilder;

pub use alacritty_terminal::event::WindowSize;
