mod connections_panel;
mod footer_bar;
mod header_bar;
mod menu_bar;
mod notifications_panel;
mod settings_dialog;
mod terminal_element;
mod terminal_panel;
mod workspace;

pub use workspace::*;

// Re-export components for external use
pub use menu_bar::{MenuBar, MenuBarEvent, MenuItem};
pub use terminal_panel::{TerminalPanel, TerminalPanelEvent};
