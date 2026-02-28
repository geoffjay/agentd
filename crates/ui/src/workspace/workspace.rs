use super::connections_panel::{ConnectionEvent, ConnectionsPanel};
use super::footer_bar::{FooterBar, FooterBarEvent};
use super::header_bar::HeaderBar;
use super::menu_bar::{MenuBar, MenuBarEvent, MenuItem};
use super::notifications_panel::{NotificationsPanel, NotificationsPanelEvent};
use super::settings_dialog::{SettingsDialog, SettingsDialogEvent};
use super::terminal_panel::{TerminalPanel, TerminalPanelEvent};

use crate::services::NotifyServiceManager;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use gpui_component::ActiveTheme;

pub struct Workspace {
    menu_bar: Entity<MenuBar>,
    header_bar: Entity<HeaderBar>,
    footer_bar: Entity<FooterBar>,
    connections_panel: Entity<ConnectionsPanel>,
    notifications_panel: Entity<NotificationsPanel>,
    terminal_panel: Entity<TerminalPanel>,
    settings_dialog: Option<Entity<SettingsDialog>>,
    _subscriptions: Vec<Subscription>,
    selected_menu_item: Option<MenuItem>,
    show_settings: bool,
    service_manager: Option<Arc<NotifyServiceManager>>,
    polling_task: Option<Task<()>>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let menu_bar = MenuBar::view(window, cx);
        let header_bar = HeaderBar::view(window, cx);
        let footer_bar = FooterBar::view(window, cx);
        let connections_panel = ConnectionsPanel::view(window, cx);
        let notifications_panel = NotificationsPanel::view(window, cx);
        let terminal_panel = TerminalPanel::view(window, cx);

        let _subscriptions = vec![
            cx.subscribe(&menu_bar, |this, _, event: &MenuBarEvent, cx| {
                match event {
                    MenuBarEvent::MenuItemSelected(item) => match item {
                        MenuItem::Settings => {
                            this.show_settings = true;
                            this.selected_menu_item = None;
                        }
                        _ => {
                            this.selected_menu_item = Some(*item);
                        }
                    },
                }
                cx.notify();
            }),
            cx.subscribe(&connections_panel, |this, _, event: &ConnectionEvent, cx| {
                match event {
                    ConnectionEvent::Connected(service_manager) => {
                        this.service_manager = Some(service_manager.clone());
                        // Pass service manager to notifications panel
                        this.notifications_panel.update(cx, |panel, _cx| {
                            panel.set_service_manager(service_manager.clone());
                        });
                        // Initial fetch
                        this.fetch_notifications(cx);
                        // Start polling
                        this.start_polling(cx);
                    }
                    ConnectionEvent::Disconnected => {
                        this.service_manager = None;
                        this.stop_polling();
                        this.notifications_panel.update(cx, |panel, cx| {
                            panel.clear_service_manager();
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
                        // Connections are always visible in the sidebar when Notifications is selected
                        this.selected_menu_item = Some(MenuItem::Notifications);
                        this.menu_bar.update(cx, |bar, cx| {
                            bar.select(MenuItem::Notifications, cx);
                        });
                    }
                    FooterBarEvent::ShowNotifications => {
                        this.selected_menu_item = Some(MenuItem::Notifications);
                        this.menu_bar.update(cx, |bar, cx| {
                            bar.select(MenuItem::Notifications, cx);
                        });
                    }
                    FooterBarEvent::ShowTerminal => {
                        this.selected_menu_item = Some(MenuItem::Terminal);
                        this.menu_bar.update(cx, |bar, cx| {
                            bar.select(MenuItem::Terminal, cx);
                        });
                    }
                    FooterBarEvent::OpenSettings => {
                        this.show_settings = true;
                        this.menu_bar.update(cx, |bar, cx| {
                            bar.select(MenuItem::Settings, cx);
                        });
                    }
                }
                cx.notify();
            }),
            cx.subscribe(&notifications_panel, |this, _, event: &NotificationsPanelEvent, cx| {
                match event {
                    NotificationsPanelEvent::Refresh => {
                        this.fetch_notifications(cx);
                    }
                    NotificationsPanelEvent::DismissNotification(notification_id) => {
                        this.dismiss_notification(*notification_id, cx);
                    }
                }
            }),
            cx.subscribe(
                &terminal_panel,
                |_this, _, event: &TerminalPanelEvent, _cx| match event {
                    TerminalPanelEvent::SessionAttached(session_name) => {
                        println!("Terminal session attached: {}", session_name);
                    }
                    TerminalPanelEvent::SessionDetached => {
                        println!("Terminal session detached");
                    }
                    TerminalPanelEvent::SessionError(error) => {
                        println!("Terminal session error: {}", error);
                    }
                },
            ),
        ];

        Self {
            menu_bar,
            header_bar,
            footer_bar,
            connections_panel,
            notifications_panel,
            terminal_panel,
            settings_dialog: None,
            _subscriptions,
            selected_menu_item: Some(MenuItem::Notifications), // Start with notifications selected
            show_settings: false,
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
            let task = cx.spawn(async move |view, cx| {
                loop {
                    // Wait 5 seconds between polls
                    cx.background_executor().timer(Duration::from_secs(5)).await;

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

    fn dismiss_notification(&mut self, notification_id: Uuid, cx: &mut Context<Self>) {
        if let Some(service_manager) = &self.service_manager {
            let service_manager = service_manager.clone();
            cx.spawn(async move |view, cx| {
                // Delete the notification
                if let Err(e) = service_manager.delete_notification(notification_id).await {
                    eprintln!("Failed to delete notification: {e}");
                    return;
                }

                // Refresh the notifications list
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

    fn apply_theme_by_name(&mut self, theme_name: &str, cx: &mut Context<Self>) {
        use crate::theme::THEMES;
        use std::rc::Rc;

        if let Some(theme_config) = THEMES.get(theme_name) {
            let theme_config = Rc::new(theme_config.clone());
            let theme = gpui_component::Theme::global_mut(cx);
            theme.mode = theme_config.mode;
            theme.apply_config(&theme_config);
        }
    }

    pub fn show_terminal_with_session(&mut self, session_name: String, cx: &mut Context<Self>) {
        // Switch to terminal panel
        self.selected_menu_item = Some(MenuItem::Terminal);
        self.menu_bar.update(cx, |bar, cx| {
            bar.select(MenuItem::Terminal, cx);
        });

        // Attach to the specified session
        self.terminal_panel.update(cx, |panel, cx| {
            panel.attach_session(session_name, cx);
        });

        cx.notify();
    }

    /// Determine if the sidebar should be shown (only when Notifications is selected)
    fn show_sidebar(&self) -> bool {
        matches!(self.selected_menu_item, Some(MenuItem::Notifications))
    }

    /// Render the main content area based on the selected menu item
    fn render_content(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.selected_menu_item {
            Some(MenuItem::Notifications) => self.notifications_panel.clone().into_any_element(),
            Some(MenuItem::Terminal) => self.terminal_panel.clone().into_any_element(),
            Some(MenuItem::Settings) | None => div().into_any_element(),
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Create settings dialog if needed
        if self.show_settings && self.settings_dialog.is_none() {
            let dialog = SettingsDialog::view(window, cx);
            let subscription =
                cx.subscribe(&dialog, |this, _, event: &SettingsDialogEvent, cx| match event {
                    SettingsDialogEvent::Close => {
                        this.show_settings = false;
                        this.settings_dialog = None;
                        cx.notify();
                    }
                    SettingsDialogEvent::ThemeChanged { theme_name } => {
                        this.apply_theme_by_name(theme_name, cx);
                        cx.notify();
                    }
                });
            self._subscriptions.push(subscription);
            self.settings_dialog = Some(dialog);
        } else if !self.show_settings {
            self.settings_dialog = None;
        }

        // Build the layout: MenuBar | Sidebar (optional) | Content
        let mut root = div()
            .id("workspace")
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .child(self.menu_bar.clone()); // Fixed width menu (48px)

        // Add sidebar when showing notifications
        root = root.when(self.show_sidebar(), |el| {
            el.child(
                div()
                    .id("workspace-sidebar")
                    .w(px(300.))
                    .h_full()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(self.connections_panel.clone()),
            )
        });

        // Add main content area with header, content, and footer
        root = root.child(
            div()
                .id("workspace-content")
                .flex()
                .flex_col()
                .flex_1() // Take remaining space
                .child(self.header_bar.clone())
                .child(
                    div()
                        .id("workspace-main")
                        .flex_1() // Content area takes remaining vertical space
                        .overflow_hidden()
                        .child(self.render_content(cx)),
                )
                .child(self.footer_bar.clone()),
        );

        // Overlay settings dialog if shown
        if let Some(settings_dialog) = &self.settings_dialog {
            root = root.child(settings_dialog.clone());
        }

        root
    }
}
