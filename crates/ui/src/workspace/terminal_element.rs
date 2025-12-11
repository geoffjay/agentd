use gpui::*;
use gpui_component::{v_flex, ActiveTheme as _};

/// A basic terminal rendering element
/// This is a simplified version that just renders terminal content as text
/// In the future, this will be replaced with proper terminal emulation
#[derive(IntoElement)]
pub struct TerminalElement {
    lines: Vec<String>,
}

impl TerminalElement {
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines }
    }
}

impl RenderOnce for TerminalElement {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .font_family(".SystemUIFont")
            .text_sm()
            .text_color(theme.foreground)
            .gap_1()
            .children(self.lines.iter().map(|line| {
                div().child(if line.is_empty() {
                    // Empty line - use nbsp to maintain spacing
                    "\u{00A0}".to_string()
                } else {
                    line.clone()
                })
            }))
    }
}
