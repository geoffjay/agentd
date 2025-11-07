use std::sync::Arc;

use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputState, TextInput},
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem},
    v_flex, ActiveTheme as _, Disableable, Icon, IndexPath, Selectable, Sizable as _, StyledExt,
};

use crate::services::*;

pub enum ConnectionEvent {
    Connected(Arc<NotifyServiceManager>),
    Disconnected,
    ConnectionError { message: String },
}

impl EventEmitter<ConnectionEvent> for ConnectionsPanel {}

#[derive(IntoElement)]
struct ConnectionListItem {
    base: ListItem,
    ix: IndexPath,
    connection: ServiceConnection,
    selected: bool,
}

impl ConnectionListItem {
    pub fn new(
        id: impl Into<ElementId>,
        connection: ServiceConnection,
        ix: IndexPath,
        selected: bool,
    ) -> Self {
        Self { connection, ix, base: ListItem::new(id), selected }
    }
}

impl Selectable for ConnectionListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for ConnectionListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color =
            if self.selected { cx.theme().accent_foreground } else { cx.theme().foreground };

        let bg_color = if self.selected {
            cx.theme().list_active.opacity(0.2)
        } else if self.ix.row % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
        };

        self.base.px_3().py_2().overflow_x_hidden().bg(bg_color).child(
            h_flex().items_center().gap_3().text_color(text_color).child(
                v_flex()
                    .gap_1()
                    .flex_1()
                    .overflow_x_hidden()
                    .child(
                        Label::new(self.connection.name.clone())
                            .font_semibold()
                            .whitespace_nowrap(),
                    )
                    .child(
                        Label::new(self.connection.url.clone())
                            .text_xs()
                            .text_color(text_color.opacity(0.6))
                            .whitespace_nowrap(),
                    ),
            ),
        )
    }
}

struct ConnectionListDelegate {
    connections: Vec<ServiceConnection>,
    matched_connections: Vec<ServiceConnection>,
    selected_index: Option<IndexPath>,
    query: String,
}

impl ListDelegate for ConnectionListDelegate {
    type Item = ConnectionListItem;

    fn items_count(&self, _section: usize, _app: &App) -> usize {
        self.matched_connections.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Task<()> {
        self.query = query.to_string();
        self.matched_connections = if query.is_empty() {
            self.connections.clone()
        } else {
            self.connections
                .iter()
                .filter(|conn| {
                    conn.name.to_lowercase().contains(&query.to_lowercase())
                        || conn.url.to_lowercase().contains(&query.to_lowercase())
                })
                .cloned()
                .collect()
        };
        Task::ready(())
    }

    fn confirm(&mut self, _secondary: bool, _window: &mut Window, _cx: &mut Context<List<Self>>) {
        if let Some(selected) = self.selected_index {
            if let Some(conn) = self.matched_connections.get(selected.row) {
                println!("Selected connection: {} at {}", conn.name, conn.url);
            }
        }
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
        if let Some(conn) = self.matched_connections.get(ix.row) {
            return Some(ConnectionListItem::new(ix, conn.clone(), ix, selected));
        }
        None
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

impl ConnectionListDelegate {
    fn new() -> Self {
        Self {
            connections: vec![],
            matched_connections: vec![],
            selected_index: None,
            query: String::new(),
        }
    }

    fn update_connections(&mut self, connections: Vec<ServiceConnection>) {
        self.connections = connections;
        self.matched_connections = self.connections.clone();
        if !self.matched_connections.is_empty() && self.selected_index.is_none() {
            self.selected_index = Some(IndexPath::default());
        }
    }

    fn selected_connection(&self) -> Option<&ServiceConnection> {
        self.selected_index.and_then(|ix| self.matched_connections.get(ix.row))
    }
}

pub struct ConnectionsPanel {
    pub service_manager: Arc<NotifyServiceManager>,
    input_esc: Entity<InputState>,
    connection_list: Entity<List<ConnectionListDelegate>>,
    is_connected: bool,
    is_loading: bool,
    _subscriptions: Vec<Subscription>,
}

impl ConnectionsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_esc =
            cx.new(|cx| InputState::new(window, cx).placeholder("Service URL").clean_on_escape());

        let connection_list = cx.new(|cx| List::new(ConnectionListDelegate::new(), window, cx));

        let _subscriptions = vec![cx.subscribe_in(
            &connection_list,
            window,
            |this, _, ev: &ListEvent, window, cx| {
                if let ListEvent::Confirm(ix) = ev {
                    if let Some(conn) = this.get_selected_connection(*ix, cx) {
                        this.input_esc.update(cx, |is, cx| {
                            is.set_value(conn.url.clone(), window, cx);
                            cx.notify();
                        })
                    }
                }
            },
        )];

        let conn_list_clone = connection_list.clone();
        cx.spawn(async move |_view, cx| {
            let connections = load_connections().await;
            let _ = cx.update_entity(&conn_list_clone, |list, cx| {
                list.delegate_mut().update_connections(connections);
                cx.notify();
            });
        })
        .detach();

        Self {
            service_manager: Arc::new(NotifyServiceManager::new()),
            input_esc,
            is_connected: false,
            is_loading: false,
            connection_list,
            _subscriptions,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn get_selected_connection(&self, ix: IndexPath, cx: &App) -> Option<ServiceConnection> {
        self.connection_list.read(cx).delegate().matched_connections.get(ix.row).cloned()
    }

    pub fn connect_to_service(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_loading {
            return;
        }

        self.is_loading = true;
        cx.notify();

        let service_manager = self.service_manager.clone();
        let service_url = self.input_esc.read(cx).value().clone();

        cx.spawn(async move |this: WeakEntity<ConnectionsPanel>, cx| {
            let result = service_manager.connect(&service_url).await;

            this.update(cx, |this, cx| {
                this.is_loading = false;
                match result {
                    Ok(_) => {
                        this.is_connected = true;
                        cx.emit(ConnectionEvent::Connected(this.service_manager.clone()));
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to connect to service: {e}");
                        eprintln!("{error_msg}");
                        this.is_connected = false;
                        cx.emit(ConnectionEvent::ConnectionError { message: error_msg });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn disconnect_from_service(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let service_manager = self.service_manager.clone();

        cx.spawn(async move |this, cx| {
            service_manager.disconnect().await;

            this.update(cx, |this, cx| {
                this.is_connected = false;
                cx.emit(ConnectionEvent::Disconnected);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    fn render_connection_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let connection_button = if self.is_connected {
            Button::new("disconnect")
                .label("Disconnect")
                .icon(Icon::empty().path("icons/unplug.svg"))
                .small()
                .danger()
                .on_click(cx.listener(Self::disconnect_from_service))
        } else {
            Button::new("connect")
                .label(if self.is_loading { "Connecting..." } else { "Connect" })
                .icon(Icon::empty().path("icons/plug-zap.svg"))
                .small()
                .outline()
                .disabled(self.is_loading)
                .on_click(cx.listener(Self::connect_to_service))
        };

        v_flex()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Label::new("Service Connection").font_bold().text_sm())
            .child(TextInput::new(&self.input_esc).cleanable())
            .child(connection_button)
    }

    fn render_connections_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .p_3()
            .flex_1()
            .child(Label::new("Saved Connections").font_bold().text_sm())
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .overflow_hidden()
                    .child(self.connection_list.clone()),
            )
    }
}

impl Render for ConnectionsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().sidebar_primary_foreground)
            .child(self.render_connection_section(cx))
            .child(self.render_connections_list(cx))
    }
}
