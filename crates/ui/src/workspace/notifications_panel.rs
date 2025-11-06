use std::ops::Range;

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Icon, Size, StyleSized, Sizable as _, StyledExt, h_flex,
    button::{Button, ButtonVariants as _},
    label::Label,
    table::{Column, Table, TableDelegate},
    v_flex,
};
use notify::types::Notification;

pub enum NotificationsPanelEvent {
    Refresh,
}

impl EventEmitter<NotificationsPanelEvent> for NotificationsPanel {}

pub struct NotificationsPanel {
    notifications: Vec<Notification>,
    table: Entity<Table<NotificationsTableDelegate>>,
    size: Size,
}

struct NotificationsTableDelegate {
    columns: Vec<Column>,
    notifications: Vec<Notification>,
    size: Size,
    loading: bool,
    visible_rows: Range<usize>,
}

impl NotificationsTableDelegate {
    fn new() -> Self {
        let columns = vec![
            Column::new("priority", "Priority").width(80.0),
            Column::new("title", "Title").width(200.0),
            Column::new("message", "Message").width(600.0),
            Column::new("source", "Source").width(120.0),
            Column::new("status", "Status").width(100.0),
            Column::new("created_at", "Created").width(180.0),
        ];

        Self {
            size: Size::default(),
            notifications: vec![],
            columns,
            loading: false,
            visible_rows: Range::default(),
        }
    }

    pub fn update(&mut self, notifications: Vec<Notification>) {
        self.notifications = notifications;
    }

    fn format_priority(&self, notification: &Notification) -> String {
        format!("{:?}", notification.priority)
    }

    fn format_source(&self, notification: &Notification) -> String {
        match &notification.source {
            notify::types::NotificationSource::System => "System".to_string(),
            notify::types::NotificationSource::AgentHook { agent_id, .. } => {
                format!("Agent: {}", agent_id)
            }
            notify::types::NotificationSource::AskService { .. } => "Ask Service".to_string(),
            notify::types::NotificationSource::MonitorService { alert_type } => {
                format!("Monitor: {}", alert_type)
            }
        }
    }

    fn format_status(&self, notification: &Notification) -> String {
        format!("{:?}", notification.status)
    }

    fn format_created_at(&self, notification: &Notification) -> String {
        notification.created_at.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

impl TableDelegate for NotificationsTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.notifications.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> &Column {
        self.columns.get(col_ix).unwrap()
    }

    fn render_th(
        &self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<Table<Self>>,
    ) -> impl IntoElement {
        let th = div().child(format!("{}", self.column(col_ix, cx).name));
        th.table_cell_size(self.size)
    }

    fn render_tr(
        &self,
        row_ix: usize,
        _: &mut Window,
        cx: &mut Context<Table<Self>>,
    ) -> gpui::Stateful<gpui::Div> {
        div().id(row_ix).on_click(cx.listener(|_, ev: &ClickEvent, _, _| {
            println!("You have clicked notification with secondary: {}", ev.modifiers().secondary())
        }))
    }

    fn render_td(
        &self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        _cx: &mut Context<Table<Self>>,
    ) -> impl IntoElement {
        if let Some(notification) = self.notifications.get(row_ix) {
            let cell_value = match col_ix {
                0 => self.format_priority(notification),
                1 => notification.title.clone(),
                2 => notification.message.clone(),
                3 => self.format_source(notification),
                4 => self.format_status(notification),
                5 => self.format_created_at(notification),
                _ => "--".to_string(),
            };
            return cell_value.into_any_element();
        }

        "--".into_any_element()
    }

    fn move_column(
        &mut self,
        col_ix: usize,
        to_ix: usize,
        _: &mut Window,
        _: &mut Context<Table<Self>>,
    ) {
        let col = self.columns.remove(col_ix);
        self.columns.insert(to_ix, col);
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn load_more_threshold(&self) -> usize {
        50
    }

    fn load_more(&mut self, _: &mut Window, _: &mut Context<Table<Self>>) {
        // No-op for now - could implement pagination here
    }

    fn visible_rows_changed(
        &mut self,
        visible_range: Range<usize>,
        _: &mut Window,
        _: &mut Context<Table<Self>>,
    ) {
        self.visible_rows = visible_range;
    }
}

impl NotificationsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = NotificationsTableDelegate::new();
        let table = cx.new(|cx| {
            let mut t = Table::new(delegate, window, cx);
            t.set_stripe(true, cx);
            t
        });

        Self { notifications: vec![], table, size: Size::default() }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn update_notifications(&mut self, notifications: Vec<Notification>, cx: &mut Context<Self>) {
        self.notifications = notifications.clone();
        self.table.update(cx, |table, cx| {
            table.delegate_mut().update(notifications);
            table.refresh(cx);
        });
        cx.notify();
    }

    pub fn clear_notifications(&mut self, cx: &mut Context<Self>) {
        self.notifications = vec![];
        self.table.update(cx, |table, cx| {
            table.delegate_mut().update(vec![]);
            table.refresh(cx);
        });
        cx.notify();
    }

    fn on_refresh(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(NotificationsPanelEvent::Refresh);
    }
}

impl Render for NotificationsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let refresh_button = Button::new("refresh")
            .label("Refresh")
            .icon(Icon::empty().path("icons/refresh-cw.svg"))
            .small()
            .outline()
            .on_click(cx.listener(Self::on_refresh));

        let header = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Label::new(format!("Notifications ({})", self.notifications.len()))
                    .font_semibold()
                    .text_sm(),
            )
            .child(refresh_button);

        if self.notifications.is_empty() {
            v_flex().size_full().child(header).child(
                h_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(
                        Label::new("No notifications")
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
        } else {
            v_flex().size_full().child(header).child(
                div().flex_1().p_4().child(self.table.clone()),
            )
        }
    }
}
