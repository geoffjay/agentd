use std::ops::Range;
use std::sync::Arc;

use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    table::{Column, Table, TableDelegate},
    v_flex, ActiveTheme as _, Icon, Sizable as _, StyledExt,
};
use notify::types::Notification;
use uuid::Uuid;

use crate::services::NotifyServiceManager;

pub enum NotificationsPanelEvent {
    Refresh,
    DismissNotification(Uuid),
}

impl EventEmitter<NotificationsPanelEvent> for NotificationsPanel {}

pub struct NotificationsPanel {
    notifications: Vec<Notification>,
    table: Entity<Table<NotificationsTableDelegate>>,
    service_manager: Option<Arc<NotifyServiceManager>>,
}

struct NotificationsTableDelegate {
    columns: Vec<Column>,
    notifications: Vec<Notification>,
    loading: bool,
    visible_rows: Range<usize>,
    panel: WeakEntity<NotificationsPanel>,
    sort_column: Option<usize>,
    sort_ascending: bool,
}

impl NotificationsTableDelegate {
    fn new(panel: WeakEntity<NotificationsPanel>) -> Self {
        let columns = vec![
            Column::new("priority", "Priority").width(80.0).sortable().resizable(false),
            Column::new("title", "Title").width(150.0).sortable(),
            Column::new("message", "Message").width(300.0),
            Column::new("source", "Source").width(100.0).sortable().resizable(false),
            Column::new("status", "Status").width(80.0).sortable(),
            Column::new("created_at", "Created").width(200.0).sortable(),
            Column::new("actions", "").width(60.0).paddings(px(8.)).resizable(false),
        ];

        Self {
            notifications: vec![],
            columns,
            loading: false,
            visible_rows: Range::default(),
            panel,
            sort_column: None,
            sort_ascending: true,
        }
    }

    pub fn update(&mut self, notifications: Vec<Notification>) {
        self.notifications = notifications;
        self.apply_sort();
    }

    fn apply_sort(&mut self) {
        if let Some(col_ix) = self.sort_column {
            let ascending = self.sort_ascending;
            self.notifications.sort_by(|a, b| {
                let ordering = match col_ix {
                    0 => a.priority.cmp(&b.priority),
                    1 => a.title.cmp(&b.title),
                    2 => a.message.cmp(&b.message),
                    3 => {
                        // Format source for comparison
                        let a_src = match &a.source {
                            notify::types::NotificationSource::System => "System",
                            notify::types::NotificationSource::AgentHook { .. } => "Agent",
                            notify::types::NotificationSource::AskService { .. } => "Ask Service",
                            notify::types::NotificationSource::MonitorService { .. } => "Monitor",
                        };
                        let b_src = match &b.source {
                            notify::types::NotificationSource::System => "System",
                            notify::types::NotificationSource::AgentHook { .. } => "Agent",
                            notify::types::NotificationSource::AskService { .. } => "Ask Service",
                            notify::types::NotificationSource::MonitorService { .. } => "Monitor",
                        };
                        a_src.cmp(b_src)
                    }
                    4 => {
                        // Compare status using discriminant ordering
                        let a_status = a.status as u8;
                        let b_status = b.status as u8;
                        a_status.cmp(&b_status)
                    }
                    5 => a.created_at.cmp(&b.created_at),
                    _ => std::cmp::Ordering::Equal,
                };
                if ascending {
                    ordering
                } else {
                    ordering.reverse()
                }
            });
        }
    }

    fn format_priority(&self, notification: &Notification) -> String {
        format!("{:?}", notification.priority)
    }

    fn format_source(&self, notification: &Notification) -> String {
        match &notification.source {
            notify::types::NotificationSource::System => "System".to_string(),
            notify::types::NotificationSource::AgentHook { agent_id, .. } => {
                format!("Agent: {agent_id}")
            }
            notify::types::NotificationSource::AskService { .. } => "Ask Service".to_string(),
            notify::types::NotificationSource::MonitorService { alert_type } => {
                format!("Monitor: {alert_type}")
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
        div().child(format!("{}", self.column(col_ix, cx).name))
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
            // Special handling for actions column
            if col_ix == 6 {
                let notification_id = notification.id;
                let panel = self.panel.clone();
                let dismiss_button = Button::new(SharedString::from(format!("dismiss_{row_ix}")))
                    .icon(Icon::empty().path("icons/x.svg"))
                    .xsmall()
                    .ghost()
                    .danger()
                    .on_click(move |_ev, _window, cx| {
                        let _ = panel.update(cx, |_panel, cx| {
                            cx.emit(NotificationsPanelEvent::DismissNotification(notification_id));
                        });
                    });
                return dismiss_button.into_any_element();
            }

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

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: gpui_component::table::ColumnSort,
        _: &mut Window,
        cx: &mut Context<Table<Self>>,
    ) {
        use gpui_component::table::ColumnSort;

        // Determine sort direction
        match sort {
            ColumnSort::Ascending => {
                self.sort_column = Some(col_ix);
                self.sort_ascending = true;
            }
            ColumnSort::Descending => {
                self.sort_column = Some(col_ix);
                self.sort_ascending = false;
            }
            ColumnSort::Default => {
                // Toggle or set ascending
                if self.sort_column == Some(col_ix) {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = Some(col_ix);
                    self.sort_ascending = true;
                }
            }
        }

        self.apply_sort();
        cx.notify();
    }
}

impl NotificationsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let panel_weak = cx.weak_entity();
        let delegate = NotificationsTableDelegate::new(panel_weak);
        let table = cx.new(|cx| {
            let mut t = Table::new(delegate, window, cx);
            t.set_stripe(true, cx);
            t
        });

        Self { notifications: vec![], table, service_manager: None }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn update_notifications(
        &mut self,
        notifications: Vec<Notification>,
        cx: &mut Context<Self>,
    ) {
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

    pub fn set_service_manager(&mut self, service_manager: Arc<NotifyServiceManager>) {
        self.service_manager = Some(service_manager);
    }

    pub fn clear_service_manager(&mut self) {
        self.service_manager = None;
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
                h_flex().flex_1().items_center().justify_center().child(
                    Label::new("No notifications")
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                ),
            )
        } else {
            v_flex().size_full().child(header).child(div().flex_1().p_4().child(self.table.clone()))
        }
    }
}
