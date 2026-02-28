use gpui::*;
use gpui_component::{h_flex, label::Label, v_flex, ActiveTheme as _, StyledExt};

use super::terminal_element::TerminalElement;

pub enum TerminalPanelEvent {
    SessionAttached(String),
    SessionDetached,
    SessionError(String),
}

impl EventEmitter<TerminalPanelEvent> for TerminalPanel {}

pub struct TerminalPanel {
    focus_handle: FocusHandle,
    session_name: Option<String>,
    terminal_content: Vec<String>,
    error_message: Option<String>,
    status_message: String,
}

impl TerminalPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            session_name: None,
            terminal_content: vec![],
            error_message: None,
            status_message: "No terminal session attached".to_string(),
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn attach_session(&mut self, session_name: String, cx: &mut Context<Self>) {
        self.session_name = Some(session_name.clone());
        self.error_message = None;
        self.status_message = format!("Attaching to tmux session: {}", session_name);

        // Spawn async task to attach to the tmux session
        let session_name_clone = session_name.clone();
        cx.spawn(async move |view, cx| {
            // TODO: Replace with actual terminal integration
            // For now, simulate terminal output
            let result = Self::connect_to_tmux(&session_name_clone).await;

            let _ = view.update(cx, |view, cx| match result {
                Ok(content) => {
                    view.terminal_content = content;
                    view.status_message =
                        format!("Connected to tmux session: {}", session_name_clone);
                    cx.emit(TerminalPanelEvent::SessionAttached(session_name_clone));
                    cx.notify();
                }
                Err(err) => {
                    view.error_message = Some(err.clone());
                    view.status_message = format!("Failed to attach to session: {}", err);
                    cx.emit(TerminalPanelEvent::SessionError(err));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn detach_session(&mut self, cx: &mut Context<Self>) {
        if let Some(session_name) = self.session_name.take() {
            self.terminal_content.clear();
            self.status_message = format!("Detached from session: {}", session_name);
            cx.emit(TerminalPanelEvent::SessionDetached);
            cx.notify();
        }
    }

    pub fn current_session(&self) -> Option<&str> {
        self.session_name.as_deref()
    }

    pub fn is_attached(&self) -> bool {
        self.session_name.is_some()
    }

    async fn connect_to_tmux(session_name: &str) -> Result<Vec<String>, String> {
        // TODO: Replace with actual tmux integration using ui_terminal crate
        // For now, return mock data
        use std::time::Duration;
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate successful connection
        Ok(vec![
            format!("Connected to tmux session: {}", session_name),
            "".to_string(),
            "Welcome to agentd terminal".to_string(),
            "".to_string(),
            "Type 'help' for available commands".to_string(),
        ])
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, _cx: &mut Context<Self>) {
        // TODO: Forward keyboard input to terminal
        // For now, just log the event
        if self.session_name.is_some() {
            println!("Key pressed: {:?}", event.keystroke.key);
        }
    }
}

impl Focusable for TerminalPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Label::new(
                    self.session_name
                        .as_ref()
                        .map(|s| format!("Terminal: {}", s))
                        .unwrap_or_else(|| "Terminal".to_string()),
                )
                .font_semibold()
                .text_sm(),
            )
            .child(
                Label::new(&self.status_message).text_xs().text_color(cx.theme().muted_foreground),
            );

        let terminal_view = if let Some(error) = &self.error_message {
            // Show error message
            v_flex().flex_1().items_center().justify_center().child(
                Label::new(format!("Error: {}", error)).text_sm().text_color(cx.theme().danger),
            )
        } else if self.session_name.is_some() {
            // Show terminal content
            v_flex()
                .flex_1()
                .p_4()
                .bg(cx.theme().background)
                .child(TerminalElement::new(self.terminal_content.clone()))
        } else {
            // Show placeholder when no session is attached
            v_flex().flex_1().items_center().justify_center().child(
                Label::new("No terminal session attached")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
        };

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(header)
            .child(terminal_view)
    }
}
