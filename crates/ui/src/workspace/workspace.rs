use super::connections_panel::{ConnectionEvent, ConnectionsPanel};
use super::footer_bar::{FooterBar, FooterBarEvent};
use super::header_bar::HeaderBar;
use super::notifications_panel::{NotificationsPanel, NotificationsPanelEvent};

use crate::services::NotifyServiceManager;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::Arc;
use std::time::Duration;

use gpui_component::ActiveTheme;

pub struct Workspace {
    header_bar: Entity<HeaderBar>,
    footer_bar: Entity<FooterBar>,
    connections_panel: Entity<ConnectionsPanel>,
    notifications_panel: Entity<NotificationsPanel>,
    _subscriptions: Vec<Subscription>,
    show_connections: bool,
    service_manager: Option<Arc<NotifyServiceManager>>,
    polling_task: Option<Task<()>>,
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
                        this.service_manager = Some(service_manager.clone());
                        // Initial fetch
                        this.fetch_notifications(cx);
                        // Start polling
                        this.start_polling(cx);
                    }
                    ConnectionEvent::Disconnected => {
                        this.service_manager = None;
                        this.stop_polling();
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
            cx.subscribe(&notifications_panel, |this, _, event: &NotificationsPanelEvent, cx| {
                match event {
                    NotificationsPanelEvent::Refresh => {
                        this.fetch_notifications(cx);
                    }
                }
            }),
        ];

        Self {
            header_bar,
            footer_bar,
            connections_panel,
            notifications_panel,
            _subscriptions,
            show_connections: true,
            service_manager: None,
            polling_task: None,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn fetch_notifications(&mut self, cx: &mut Context<Self>) {
        if let Some(service_manager) = &self.service_manager {
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
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        // Cancel any existing polling task
        self.stop_polling();

        if self.service_manager.is_some() {
            let task = cx.spawn(async move |view, mut cx| {
                loop {
                    // Wait 5 seconds between polls
                    cx.background_executor()
                        .timer(Duration::from_secs(5))
                        .await;

                    // Fetch notifications
                    let should_continue = view
                        .update(cx, |view, cx| {
                            if view.service_manager.is_some() {
                                view.fetch_notifications(cx);
                                true
                            } else {
                                false
                            }
                        })
                        .ok()
                        .unwrap_or(false);

                    if !should_continue {
                        break;
                    }
                }
            });

            self.polling_task = Some(task);
        }
    }

    fn stop_polling(&mut self) {
        if let Some(task) = self.polling_task.take() {
            drop(task);
        }
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
