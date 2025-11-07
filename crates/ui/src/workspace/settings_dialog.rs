use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    list::{List, ListDelegate, ListItem},
    modal::Modal,
    v_flex, ActiveTheme as _, Icon, IndexPath, Selectable, Sizable as _, StyledExt,
};

use crate::theme::THEMES;

#[derive(Clone, Copy, PartialEq)]
enum SettingsArea {
    General,
}

impl SettingsArea {
    fn label(&self) -> &str {
        match self {
            SettingsArea::General => "General",
        }
    }
}

pub enum SettingsDialogEvent {
    Close,
    ThemeChanged { theme_name: String },
}

impl EventEmitter<SettingsDialogEvent> for SettingsDialog {}

#[derive(IntoElement)]
struct SettingsAreaListItem {
    base: ListItem,
    area: SettingsArea,
    selected: bool,
}

impl SettingsAreaListItem {
    pub fn new(id: impl Into<ElementId>, area: SettingsArea, selected: bool) -> Self {
        Self { area, base: ListItem::new(id), selected }
    }
}

impl Selectable for SettingsAreaListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for SettingsAreaListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color =
            if self.selected { cx.theme().accent_foreground } else { cx.theme().foreground };

        let bg_color =
            if self.selected { cx.theme().list_active.opacity(0.2) } else { cx.theme().list };

        let label_text = self.area.label().to_string();

        self.base
            .px_3()
            .py_2()
            .bg(bg_color)
            .child(Label::new(label_text).text_sm().text_color(text_color).whitespace_nowrap())
    }
}

struct SettingsAreaListDelegate {
    areas: Vec<SettingsArea>,
    selected_index: Option<IndexPath>,
}

impl ListDelegate for SettingsAreaListDelegate {
    type Item = SettingsAreaListItem;

    fn items_count(&self, _section: usize, _app: &App) -> usize {
        self.areas.len()
    }

    fn perform_search(
        &mut self,
        _query: &str,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Task<()> {
        Task::ready(())
    }

    fn confirm(&mut self, _secondary: bool, _window: &mut Window, _cx: &mut Context<List<Self>>) {
        // Handled by set_selected_index
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<List<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_item(
        &self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index;
        self.areas.get(ix.row).map(|area| SettingsAreaListItem::new(ix, *area, selected))
    }

    fn loading(&self, _: &App) -> bool {
        false
    }

    fn load_more_threshold(&self) -> usize {
        0
    }

    fn load_more(&mut self, _window: &mut Window, _cx: &mut Context<List<Self>>) {
        // No-op
    }
}

impl SettingsAreaListDelegate {
    fn new() -> Self {
        Self { areas: vec![SettingsArea::General], selected_index: Some(IndexPath::default()) }
    }

    fn selected_area(&self) -> Option<SettingsArea> {
        self.selected_index.and_then(|ix| self.areas.get(ix.row).copied())
    }
}

pub struct SettingsDialog {
    areas_list: Entity<List<SettingsAreaListDelegate>>,
    selected_theme: String,
    theme_dropdown_open: bool,
}

impl SettingsDialog {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let areas_list = cx.new(|cx| List::new(SettingsAreaListDelegate::new(), window, cx));

        // Get current theme name
        let current_theme = cx.theme();
        let selected_theme = if current_theme.mode.is_dark() {
            "Kanagawa Dragon".to_string()
        } else {
            "Kanagawa Lotus".to_string()
        };

        Self { areas_list, selected_theme, theme_dropdown_open: false }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn on_close(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(SettingsDialogEvent::Close);
    }

    fn on_theme_select(
        &mut self,
        theme_name: String,
        _: &ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_theme = theme_name.clone();
        self.theme_dropdown_open = false;
        cx.emit(SettingsDialogEvent::ThemeChanged { theme_name });
        cx.notify();
    }

    fn toggle_theme_dropdown(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.theme_dropdown_open = !self.theme_dropdown_open;
        cx.notify();
    }

    fn render_general_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // Get all available themes sorted by name
        let mut theme_names: Vec<_> = THEMES.keys().map(|s| s.to_string()).collect();
        theme_names.sort();

        // Dropdown trigger button
        let dropdown_trigger = Button::new("theme-dropdown-trigger")
            .label(self.selected_theme.clone())
            .icon(Icon::empty().path("icons/chevron-down.svg"))
            .small()
            .outline()
            .w(px(250.0))
            .on_click(cx.listener(Self::toggle_theme_dropdown));

        // Dropdown menu (only show if open)
        let mut dropdown_container = div().relative().w(px(250.0)).child(dropdown_trigger);

        if self.theme_dropdown_open {
            let mut dropdown_menu = v_flex()
                .absolute()
                .top(px(32.0))
                .left(px(0.0))
                .w_full()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(cx.theme().radius)
                .bg(cx.theme().background)
                .shadow_lg()
                .max_h(px(200.0))
                .overflow_hidden();

            for theme_name in theme_names {
                let is_selected = self.selected_theme == theme_name;
                let theme_name_clone = theme_name.clone();

                let mut button = Button::new(SharedString::from(format!("theme_{theme_name}")))
                    .label(theme_name)
                    .small()
                    .w_full();

                button = if is_selected { button.outline() } else { button.ghost() };

                button = button.on_click(cx.listener(move |this, ev, window, cx| {
                    this.on_theme_select(theme_name_clone.clone(), ev, window, cx);
                }));

                dropdown_menu = dropdown_menu.child(button);
            }

            dropdown_container = dropdown_container.child(dropdown_menu);
        }

        v_flex().gap_4().p_4().child(
            h_flex()
                .gap_3()
                .items_start()
                .child(Label::new("Theme:").text_sm().w(px(80.0)).mt_2())
                .child(dropdown_container),
        )
    }

    fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_area =
            self.areas_list.read(cx).delegate().selected_area().unwrap_or(SettingsArea::General);

        match selected_area {
            SettingsArea::General => self.render_general_settings(cx).into_any_element(),
        }
    }
}

impl Render for SettingsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .w_full()
            .items_center()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Label::new("Settings").font_semibold().text_lg());

        let sidebar = div()
            .w(px(200.0))
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar_primary_foreground)
            .child(self.areas_list.clone());

        let content = div().flex_1().overflow_hidden().child(self.render_content(cx));

        let body = h_flex().flex_1().overflow_hidden().child(sidebar).child(content);

        Modal::new(_window, cx)
            .width(px(700.0))
            .on_close(cx.listener(|this, _ev, _window, cx| {
                this.on_close(_ev, _window, cx);
            }))
            .child(
                v_flex()
                    .h(px(500.0))
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(body)
            )
    }
}
