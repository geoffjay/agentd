use super::connections_panel::{ConnectionEvent, ConnectionsPanel};
use super::footer_bar::{FooterBar, FooterBarEvent};
use super::header_bar::HeaderBar;
use super::notifications_panel::NotificationsPanel;

use gpui::prelude::FluentBuilder;
use gpui::*;

use gpui_component::ActiveTheme;

pub struct Workspace {
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
        let connections_panel = ConnectionsPanel::view(window, cx);
        let notifications_panel = NotificationsPanel::view(window, cx);

        let _subscriptions = vec![
            cx.subscribe(&connections_panel, |this, _, event: &ConnectionEvent, cx| {
                match event {
                    ConnectionEvent::Connected(service_manager) => {
                        // When connected, fetch and display notifications
                        let service_manager = service_manager.clone();
                        cx.spawn(async move |view, cx| {
                            let notifications = service_manager.list_notifications().await;
                            if let Ok(notifs) = notifications {
                                let _ = view.update(cx, |view, cx| {
                                    view.notifications_panel.update(cx, |panel, cx| {
                                        panel.update_notifications(notifs, cx);
                                    });
                                });
                            }
                        })
                        .detach();
                    }
                    ConnectionEvent::Disconnected => {
                        this.notifications_panel.update(cx, |panel, cx| {
                            panel.clear_notifications(cx);
                        });
                    }
                    ConnectionEvent::ConnectionError { .. } => {
                        // Handle connection error if needed
                    }
                }
                cx.notify();
            }),
            cx.subscribe(&footer_bar, |this, _, event: &FooterBarEvent, cx| {
                match event {
                    FooterBarEvent::ShowConnections => {
                        this.show_connections = true;
                    }
                    FooterBarEvent::ShowNotifications => {
                        this.show_connections = false;
                    }
                }
                cx.notify();
            }),
        ];

        Self {
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
            .when(self.show_connections, |this| this.child(self.connections_panel.clone()));

        let main = div()
            .flex()
            .flex_col()
            .w_full()
            .overflow_hidden()
            .child(self.notifications_panel.clone());

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
