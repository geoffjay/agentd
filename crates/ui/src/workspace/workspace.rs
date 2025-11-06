use super::connections_panel::{ConnectionEvent, ConnectionsPanel};
use super::footer_bar::{FooterBar, FooterBarEvent};
use super::header_bar::HeaderBar;
use super::notifications_panel::NotificationsPanel;

use crate::services::TableInfo;
use gpui::prelude::FluentBuilder;
use gpui::*;

use gpui_component::ActiveTheme;
use gpui_component::resizable::{ResizableState, resizable_panel, v_resizable};

pub struct Workspace {
    resize_state: Entity<ResizableState>,
    header_bar: Entity<HeaderBar>,
    footer_bar: Entity<FooterBar>,
    connections_panel: Entity<ConnectionsPanel>,
    notifications_panel: Entity<NotificationsPanel>,
    _subscriptions: Vec<Subscription>,
    show_connections: bool,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let header_bar = HeaderBar::view(window, cx);
        let footer_bar = FooterBar::view(window, cx);
        let resize_state = ResizableState::new(cx);
        let connections_panel = ConnectionsPanel::view(window, cx);
        let notifications_panel = NotificationsPanel::view(window, cx);

        let _subscriptions = vec![
            cx.subscribe(&connections_panel, |this, _, event: &ConnectionEvent, cx| {
                this.tables_panel.update(cx, |tables_panel, cx| {
                    tables_panel.handle_connection_event(event, cx);
                });
            }),
            cx.subscribe(&footer_bar, |this, _, event: &FooterBarEvent, cx| {
                match event {
                    FooterBarEvent::ShowConnections => {
                        this.show_connections = true;
                        this.show_tables = false;
                    }
                    FooterBarEvent::ShowTables => {
                        this.show_connections = false;
                        this.show_tables = true;
                    }
                }
                cx.notify();
            }),
        ];

        Self {
            resize_state,
            header_bar,
            footer_bar,
            connections_panel,
            notifications_panel,
            _subscriptions,
            show_connections: true,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn show_table_columns(&mut self, table: TableInfo, cx: &mut Context<Self>) {
        // Get database manager from connections panel
        let db_manager = self.connections_panel.read(cx).db_manager.clone();

        cx.spawn(async move |this, cx| {
            let result = db_manager.get_table_columns(&table.table_name, &table.table_schema).await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(query_result) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(QueryExecutionResult::Select(query_result), cx);
                        });
                    }
                    Err(e) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(
                                QueryExecutionResult::Error(format!(
                                    "Failed to load table columns: {}",
                                    e
                                )),
                                cx,
                            );
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = div()
            .id("workspace-sidebar")
            .flex()
            .h_full()
            .border_color(cx.theme().border)
            .border_r_1()
            .min_w(px(300.0))
            .when(self.show_connections, |this| this.child(self.connections_panel.clone()))
            .when(self.show_tables, |this| this.child(self.tables_panel.clone()));

        let main = div().flex().flex_col().w_full().overflow_hidden().child(
            v_resizable("resizable", self.resize_state.clone())
                .child(
                    resizable_panel()
                        .size(px(400.))
                        .size_range(px(200.)..px(800.))
                        .child(self.editor.clone()),
                )
                .child(resizable_panel().size(px(200.)).child(self.results_panel.clone())),
        );

        let content = div()
            .id("workspace-content")
            .flex()
            .flex_grow()
            .bg(cx.theme().background)
            .child(sidebar)
            .child(main);

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.header_bar.clone())
            .child(content)
            .child(self.footer_bar.clone())
    }
}
