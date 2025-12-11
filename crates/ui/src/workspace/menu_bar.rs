use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    ActiveTheme, Selectable, Sizable,
};

/// Menu items available in the vertical menu bar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Notifications,
    Terminal,
    Settings,
}

/// Events emitted by the MenuBar
pub enum MenuBarEvent {
    MenuItemSelected(MenuItem),
}

impl EventEmitter<MenuBarEvent> for MenuBar {}

/// Vertical menu bar component (similar to VSCode activity bar)
///
/// The MenuBar appears on the far left side of the workspace and provides
/// quick access to different panels and features. It's a vertical column
/// of icon buttons with selection state.
pub struct MenuBar {
    focus_handle: FocusHandle,
    selected: Option<MenuItem>,
}

impl MenuBar {
    /// Create a new MenuBar instance
    pub fn new(cx: &mut App) -> Self {
        Self { focus_handle: cx.focus_handle(), selected: None }
    }

    /// Create a new MenuBar view entity
    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(cx))
    }

    /// Get the currently selected menu item
    pub fn selected(&self) -> Option<MenuItem> {
        self.selected
    }

    /// Select a menu item programmatically
    pub fn select(&mut self, item: MenuItem, cx: &mut Context<Self>) {
        self.selected = Some(item);
        cx.emit(MenuBarEvent::MenuItemSelected(item));
        cx.notify();
    }

    /// Deselect the current menu item
    pub fn deselect(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = self.selected.take() {
            cx.emit(MenuBarEvent::MenuItemSelected(item));
        }
        cx.notify();
    }

    /// Handle menu item click
    fn handle_click(&mut self, item: MenuItem, _ev: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.selected == Some(item) {
            // Toggle off if clicking the same item
            self.deselect(cx);
        } else {
            // Select the new item
            self.select(item, cx);
        }
    }

    /// Render a menu item button
    fn render_menu_item(
        &mut self,
        item: MenuItem,
        icon_text: &'static str,
        tooltip: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self.selected == Some(item);

        Button::new(self.button_id(item))
            .label(icon_text)
            .small()
            .ghost()
            .selected(is_selected)
            .tooltip(tooltip)
            .on_click(cx.listener(move |this, ev, win, cx| {
                this.handle_click(item, ev, win, cx);
            }))
    }

    /// Generate a unique button ID for each menu item
    fn button_id(&self, item: MenuItem) -> SharedString {
        match item {
            MenuItem::Notifications => "menu-notifications".into(),
            MenuItem::Terminal => "menu-terminal".into(),
            MenuItem::Settings => "menu-settings".into(),
        }
    }
}

impl Focusable for MenuBar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MenuBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notifications_button = self.render_menu_item(
            MenuItem::Notifications,
            "🔔",
            "Notifications",
            cx,
        );

        let terminal_button = self.render_menu_item(
            MenuItem::Terminal,
            "⚡",
            "Terminal",
            cx,
        );

        let settings_button = self.render_menu_item(
            MenuItem::Settings,
            "⚙",
            "Settings",
            cx,
        );

        div()
            .id("menu-bar")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .h_full()
            .w(px(48.0))
            .bg(cx.theme().title_bar)
            .border_r_1()
            .border_color(cx.theme().border)
            .items_center()
            .gap_2()
            .pt_2()
            .child(notifications_button)
            .child(terminal_button)
            .child(
                // Spacer to push settings to the bottom
                div().flex_grow()
            )
            .child(settings_button)
            .pb_2()
    }
}
