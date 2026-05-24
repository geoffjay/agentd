use agentd_common::config::AgentdConfig;
use crate::config::TuiConfig;
use super::stream;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use memory::client::MemoryClient;
use memory::types::{Memory, SearchRequest};
use orchestrator::client::OrchestratorClient;
use orchestrator::scheduler::types::{DispatchResponse, WorkflowResponse};
use orchestrator::types::{AgentResponse, ConversationHistoryQuery};
use ratatui::widgets::TableState;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Normalised conversation event for display.
///
/// Both REST history (`ConversationEventResponse`) and live stream events
/// (`serde_json::Value`) are converted into this type before being stored.
/// The stream JSON differs from the REST shape — it lacks `id` and
/// `session_number`, and puts tool details in top-level fields rather than
/// `metadata` — so we normalise on the way in.
#[derive(Debug, Clone)]
pub struct ConversationEntry {
    /// e.g. "agent:output", "agent:tool_use", "agent:prompt_sent"
    pub event_type: String,
    /// Primary text content (output line, prompt text, tool summary, etc.)
    pub line: Option<String>,
    /// Structured payload — tool `name` + `input` for tool_use events.
    pub metadata: Option<serde_json::Value>,
}

impl From<orchestrator::types::ConversationEventResponse> for ConversationEntry {
    fn from(e: orchestrator::types::ConversationEventResponse) -> Self {
        // Normalise tool_use metadata to the same shape we use for stream events:
        // { "name": ..., "input": ... }
        // The database stores the raw tool_use object with "tool_name"/"tool_input" keys.
        let metadata = if e.event_type == "agent:tool_use" {
            e.metadata.map(|m| {
                serde_json::json!({
                    "name":  m.get("tool_name").cloned().unwrap_or(serde_json::Value::Null),
                    "input": m.get("tool_input").cloned().unwrap_or(serde_json::Value::Null),
                })
            })
        } else {
            e.metadata
        };
        Self { event_type: e.event_type, line: e.line, metadata }
    }
}

impl From<serde_json::Value> for ConversationEntry {
    fn from(v: serde_json::Value) -> Self {
        let event_type = v["type"].as_str().unwrap_or("").to_string();

        // Stream events store content in different fields depending on type.
        let line = v["line"]
            .as_str()
            .or_else(|| v["text"].as_str())    // agent:thinking uses "text"
            .or_else(|| v["summary"].as_str()) // agent:tool_use has "summary"
            .map(|s| s.to_string());

        // Normalise tool_use metadata so the renderer can use the same path
        // as it does for history events.
        let metadata = if event_type == "agent:tool_use" {
            Some(serde_json::json!({
                "name": v["tool_name"],
                "input": v["tool_input"],
            }))
        } else {
            None
        };

        Self { event_type, line, metadata }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowFocus {
    None,
    Template,
    Dispatches,
}

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    AgentList,
    AgentDetail,
    WorkflowList,
    WorkflowDetail,
    MemoryList,
    MemoryDetail,
    Config,
}

/// State for memory-related dialogs that float over the memory list.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryDialog {
    None,
    /// Search dialog; holds the current input text.
    Search(String),
    /// Tag filter dialog; `cursor` is the highlighted row, `draft` is the
    /// working selection before the user confirms with Enter.
    TagFilter { cursor: usize, draft: Vec<String> },
}

impl MemoryDialog {
    pub fn is_open(&self) -> bool {
        !matches!(self, MemoryDialog::None)
    }
}

pub struct App {
    pub view: View,

    pub agents: Vec<AgentResponse>,
    pub agent_table_state: TableState,
    pub selected_agent: Option<AgentResponse>,

    pub workflows: Vec<WorkflowResponse>,
    pub workflow_table_state: TableState,
    pub selected_workflow: Option<WorkflowResponse>,
    pub workflow_dispatches: Vec<DispatchResponse>,
    pub workflow_template_scroll: u16,
    pub workflow_dispatch_scroll: u16,
    pub workflow_focus: WorkflowFocus,

    // Conversation pane (agent detail view)
    pub conversation: Vec<ConversationEntry>,
    pub conversation_scroll: u16,
    pub conversation_follow: bool,

    // Prompt input (agent detail view)
    pub input_mode: bool,
    pub input_buffer: String,
    pub input_cursor: usize,      // byte offset into input_buffer
    pub input_scroll: u16,        // visual row offset for the input box
    pub input_inner_width: usize, // set by render_input each frame for geometry calculations

    // Memory view
    pub memories: Vec<Memory>,
    pub memory_table_state: TableState,
    pub selected_memory: Option<Memory>,
    pub memory_scroll: u16,
    pub memory_search: Option<String>,
    pub memory_tag_filter: Vec<String>,
    pub memory_available_tags: Vec<String>,
    pub memory_dialog: MemoryDialog,

    pub agentd_config: AgentdConfig,
    pub config_scroll: u16,

    pub quitting: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub active_tab: usize,

    client: OrchestratorClient,
    memory_client: MemoryClient,
    orchestrator_url: String,
    last_refresh: Instant,
    refresh_interval: Duration,

    stream_rx: Option<mpsc::UnboundedReceiver<serde_json::Value>>,
    stream_abort: Option<tokio::task::AbortHandle>,
    pre_config_view: Option<View>,
}

impl App {
    pub fn new(config: TuiConfig) -> Self {
        Self {
            view: View::AgentList,
            agents: Vec::new(),
            agent_table_state: TableState::default(),
            selected_agent: None,
            workflows: Vec::new(),
            workflow_table_state: TableState::default(),
            selected_workflow: None,
            workflow_dispatches: Vec::new(),
            workflow_template_scroll: 0,
            workflow_dispatch_scroll: 0,
            workflow_focus: WorkflowFocus::None,
            conversation: Vec::new(),
            conversation_scroll: 0,
            conversation_follow: true,
            input_mode: false,
            input_buffer: String::new(),
            input_cursor: 0,
            input_scroll: 0,
            input_inner_width: 0,
            memories: Vec::new(),
            memory_table_state: TableState::default(),
            selected_memory: None,
            memory_scroll: 0,
            memory_search: None,
            memory_tag_filter: Vec::new(),
            memory_available_tags: Vec::new(),
            memory_dialog: MemoryDialog::None,
            agentd_config: config.agentd_config,
            config_scroll: 0,
            quitting: false,
            loading: false,
            error: None,
            active_tab: 0,
            orchestrator_url: config.orchestrator_url.clone(),
            client: OrchestratorClient::new(config.orchestrator_url),
            memory_client: MemoryClient::new(config.memory_url),
            last_refresh: Instant::now() - Duration::from_secs(3600),
            refresh_interval: Duration::from_secs(config.refresh_interval_secs),
            stream_rx: None,
            stream_abort: None,
            pre_config_view: None,
        }
    }

    pub async fn refresh(&mut self) {
        self.loading = true;
        self.error = None;

        match self.client.list_agents(None).await {
            Ok(resp) => self.agents = resp.items,
            Err(e) => self.error = Some(format!("agents: {e}")),
        }

        match self.client.list_workflows().await {
            Ok(resp) => self.workflows = resp.items,
            Err(e) => {
                let msg = format!("workflows: {e}");
                self.error = Some(match self.error.take() {
                    Some(prev) => format!("{prev}; {msg}"),
                    None => msg,
                });
            }
        }

        if let Some(ref sel) = self.selected_agent.clone() {
            self.selected_agent = self.agents.iter().find(|a| a.id == sel.id).cloned();
        }

        if let Some(ref wf) = self.selected_workflow.clone() {
            if let Ok(resp) = self.client.dispatch_history(&wf.id).await {
                self.workflow_dispatches = resp.items;
            }
        }

        self.loading = false;
        self.last_refresh = Instant::now();
    }

    pub async fn refresh_memories(&mut self) {
        self.loading = true;
        self.error = None;

        if let Some(ref query) = self.memory_search.clone() {
            let req = SearchRequest {
                query: query.clone(),
                tags: self.memory_tag_filter.clone(),
                limit: 100,
                ..Default::default()
            };
            match self.memory_client.search_memories(&req).await {
                Ok(resp) => self.memories = resp.memories,
                Err(e) => self.error = Some(format!("memory search: {e}")),
            }
        } else {
            let tag_param = if self.memory_tag_filter.is_empty() {
                None
            } else {
                Some(self.memory_tag_filter.join(","))
            };
            match self
                .memory_client
                .list_memories(None, tag_param.as_deref(), None, None, Some(200), None)
                .await
            {
                Ok(resp) => self.memories = resp.items,
                Err(e) => self.error = Some(format!("memories: {e}")),
            }
        }

        // Collect unique sorted tags from the loaded memories
        let mut tag_set: std::collections::HashSet<String> = self
            .memories
            .iter()
            .flat_map(|m| m.tags.iter().cloned())
            .collect();
        let mut sorted: Vec<String> = tag_set.drain().collect();
        sorted.sort();
        self.memory_available_tags = sorted;

        // Keep selection in bounds
        let len = self.memories.len();
        if len == 0 {
            self.memory_table_state.select(None);
        } else if self.memory_table_state.selected().map_or(true, |i| i >= len) {
            self.memory_table_state.select(Some(0));
        }

        self.loading = false;
    }

    pub async fn tick(&mut self) {
        self.drain_stream();
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.refresh().await;
        }
    }

    /// Pull any pending stream events into the conversation buffer.
    pub fn drain_stream(&mut self) {
        let Some(ref mut rx) = self.stream_rx else { return };
        while let Ok(value) = rx.try_recv() {
            self.conversation.push(ConversationEntry::from(value));
        }
    }

    pub fn select_next(&mut self) {
        match self.view {
            View::AgentList => {
                let len = self.agents.len();
                if len == 0 { return; }
                let next = self.agent_table_state.selected().map_or(0, |i| (i + 1).min(len - 1));
                self.agent_table_state.select(Some(next));
            }
            View::WorkflowList => {
                let len = self.workflows.len();
                if len == 0 { return; }
                let next =
                    self.workflow_table_state.selected().map_or(0, |i| (i + 1).min(len - 1));
                self.workflow_table_state.select(Some(next));
            }
            View::MemoryList => {
                let len = self.memories.len();
                if len == 0 { return; }
                let next =
                    self.memory_table_state.selected().map_or(0, |i| (i + 1).min(len - 1));
                self.memory_table_state.select(Some(next));
            }
            _ => {}
        }
    }

    pub fn select_prev(&mut self) {
        match self.view {
            View::AgentList => {
                if self.agents.is_empty() { return; }
                let prev = self.agent_table_state.selected().map_or(0, |i| i.saturating_sub(1));
                self.agent_table_state.select(Some(prev));
            }
            View::WorkflowList => {
                if self.workflows.is_empty() { return; }
                let prev =
                    self.workflow_table_state.selected().map_or(0, |i| i.saturating_sub(1));
                self.workflow_table_state.select(Some(prev));
            }
            View::MemoryList => {
                if self.memories.is_empty() { return; }
                let prev =
                    self.memory_table_state.selected().map_or(0, |i| i.saturating_sub(1));
                self.memory_table_state.select(Some(prev));
            }
            _ => {}
        }
    }

    pub fn scroll_conversation_up(&mut self) {
        self.conversation_follow = false;
        self.conversation_scroll = self.conversation_scroll.saturating_sub(1);
    }

    pub fn scroll_conversation_down(&mut self, max: u16) {
        if self.conversation_scroll < max {
            self.conversation_scroll += 1;
        } else {
            self.conversation_follow = true;
        }
    }

    async fn enter_agent_detail(&mut self, agent: AgentResponse) {
        self.stop_stream();
        self.conversation.clear();
        self.conversation_scroll = 0;
        self.conversation_follow = true;

        let id = agent.id;
        self.selected_agent = Some(agent);
        self.view = View::AgentDetail;

        // Load history. The API returns events oldest-first (no offset
        // support), so we fetch up to 500 events. The live stream fills in
        // anything that arrives after we load. The scroll fix in the renderer
        // (display_rows) ensures the view starts at the bottom regardless of
        // how many lines are present.
        let query = ConversationHistoryQuery {
            limit: Some(500),
            event_type: Some("output,prompt_sent,tool_use,result".to_string()),
            ..Default::default()
        };
        match self.client.list_conversation_events(&id, &query).await {
            Ok(resp) => {
                self.conversation = resp.events.into_iter().map(ConversationEntry::from).collect()
            }
            Err(e) => self.error = Some(format!("conversation history: {e}")),
        }

        // Start the live stream.
        let (rx, abort) = stream::spawn(&self.orchestrator_url, id);
        self.stream_rx = Some(rx);
        self.stream_abort = Some(abort);
    }

    async fn enter_workflow_detail(&mut self, wf: WorkflowResponse) {
        self.workflow_dispatches.clear();
        self.workflow_template_scroll = 0;
        self.workflow_dispatch_scroll = 0;
        self.workflow_focus = WorkflowFocus::None;

        let id = wf.id;
        self.selected_workflow = Some(wf);
        self.view = View::WorkflowDetail;

        if let Ok(resp) = self.client.dispatch_history(&id).await {
            self.workflow_dispatches = resp.items;
        }
    }

    pub async fn enter_detail(&mut self) {
        match self.view {
            View::AgentList => {
                if let Some(idx) = self.agent_table_state.selected() {
                    if let Some(agent) = self.agents.get(idx).cloned() {
                        self.enter_agent_detail(agent).await;
                    }
                }
            }
            View::WorkflowList => {
                if let Some(idx) = self.workflow_table_state.selected() {
                    if let Some(wf) = self.workflows.get(idx).cloned() {
                        self.enter_workflow_detail(wf).await;
                    }
                }
            }
            View::MemoryList => {
                if let Some(idx) = self.memory_table_state.selected() {
                    if let Some(m) = self.memories.get(idx).cloned() {
                        self.selected_memory = Some(m);
                        self.memory_scroll = 0;
                        self.view = View::MemoryDetail;
                    }
                }
            }
            _ => {}
        }
    }

    fn stop_stream(&mut self) {
        if let Some(abort) = self.stream_abort.take() {
            abort.abort();
        }
        self.stream_rx = None;
    }

    pub fn go_back(&mut self) {
        match self.view {
            View::AgentDetail => {
                self.stop_stream();
                self.conversation.clear();
                self.input_mode = false;
                self.input_buffer.clear();
                self.input_cursor = 0;
                self.input_scroll = 0;
                self.view = View::AgentList;
                self.selected_agent = None;
            }
            View::WorkflowDetail => {
                self.view = View::WorkflowList;
                self.selected_workflow = None;
                self.workflow_dispatches.clear();
                self.workflow_template_scroll = 0;
                self.workflow_dispatch_scroll = 0;
                self.workflow_focus = WorkflowFocus::None;
            }
            View::MemoryDetail => {
                self.memory_scroll = 0;
                self.view = View::MemoryList;
            }
            _ => {}
        }
    }

    fn enter_config(&mut self) {
        self.pre_config_view = Some(self.view.clone());
        self.view = View::Config;
        self.config_scroll = 0;
    }

    fn exit_config(&mut self) {
        self.view = self.pre_config_view.take().unwrap_or(View::AgentList);
    }

    fn switch_tab(&mut self, forward: bool) {
        const N: usize = 3;
        if forward {
            self.active_tab = (self.active_tab + 1) % N;
        } else {
            self.active_tab = (self.active_tab + N - 1) % N;
        }
        self.view = match self.active_tab {
            0 => View::AgentList,
            1 => View::WorkflowList,
            _ => View::MemoryList,
        };
    }

    // -- Input buffer helpers --

    fn input_insert(&mut self, ch: char) {
        self.input_buffer.insert(self.input_cursor, ch);
        self.input_cursor += ch.len_utf8();
    }

    fn input_insert_str(&mut self, s: &str) {
        self.input_buffer.insert_str(self.input_cursor, s);
        self.input_cursor += s.len();
    }

    fn input_move_up(&mut self) {
        if self.input_inner_width == 0 { return; }
        self.input_cursor = crate::input::cursor_move_vertical(
            &self.input_buffer,
            self.input_cursor,
            -1,
            self.input_inner_width,
        );
    }

    fn input_move_down(&mut self) {
        if self.input_inner_width == 0 { return; }
        self.input_cursor = crate::input::cursor_move_vertical(
            &self.input_buffer,
            self.input_cursor,
            1,
            self.input_inner_width,
        );
    }

    fn input_delete_before(&mut self) {
        if self.input_cursor == 0 { return; }
        let ch_start = self.input_buffer[..self.input_cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.input_buffer.drain(ch_start..self.input_cursor);
        self.input_cursor = ch_start;
    }

    fn input_delete_after(&mut self) {
        if self.input_cursor >= self.input_buffer.len() { return; }
        let ch_end = self.input_buffer[self.input_cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.input_cursor + i)
            .unwrap_or(self.input_buffer.len());
        self.input_buffer.drain(self.input_cursor..ch_end);
    }

    fn input_move_left(&mut self) {
        if self.input_cursor == 0 { return; }
        self.input_cursor = self.input_buffer[..self.input_cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
    }

    fn input_move_right(&mut self) {
        if self.input_cursor >= self.input_buffer.len() { return; }
        self.input_cursor += self.input_buffer[self.input_cursor..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
    }

    pub async fn handle_paste(&mut self, text: String) {
        if !self.input_mode { return; }

        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = text.split('\n').collect();
        let line_count = lines.len();

        let content = if line_count <= 4 {
            text.clone()
        } else {
            let extra = line_count - 4;
            let noun = if extra == 1 { "line" } else { "lines" };
            format!("{}\n[pasted +{} {}]", lines[..4].join("\n"), extra, noun)
        };

        self.input_insert_str(&content);
    }

    async fn submit_message(&mut self) {
        let text = self.input_buffer.trim().to_string();
        if text.is_empty() { return; }

        let Some(ref agent) = self.selected_agent.clone() else { return };
        let id: Uuid = agent.id;

        self.input_buffer.clear();
        self.input_cursor = 0;

        use orchestrator::types::SendMessageRequest;
        let req = SendMessageRequest { content: text };
        if let Err(e) = self.client.send_message(&id, &req).await {
            self.error = Some(format!("send failed: {e}"));
        }
    }

    // -- Memory dialog key handlers --

    async fn handle_search_dialog_key(&mut self, key: KeyEvent) {
        let input = match &self.memory_dialog {
            MemoryDialog::Search(s) => s.clone(),
            _ => return,
        };
        let mut input = input;

        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.push(c);
                self.memory_dialog = MemoryDialog::Search(input);
            }
            KeyCode::Backspace => {
                input.pop();
                self.memory_dialog = MemoryDialog::Search(input);
            }
            KeyCode::Enter => {
                let query = input.trim().to_string();
                self.memory_dialog = MemoryDialog::None;
                self.memory_search = if query.is_empty() { None } else { Some(query) };
                self.refresh_memories().await;
            }
            KeyCode::Esc => {
                self.memory_dialog = MemoryDialog::None;
            }
            _ => {}
        }
    }

    async fn handle_tag_dialog_key(&mut self, key: KeyEvent) {
        let (cursor, draft) = match &self.memory_dialog {
            MemoryDialog::TagFilter { cursor, draft } => (*cursor, draft.clone()),
            _ => return,
        };
        let tags = self.memory_available_tags.clone();
        let n = tags.len();

        let mut cursor = cursor;
        let mut draft = draft;

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                cursor = cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if cursor + 1 < n {
                    cursor += 1;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(tag) = tags.get(cursor) {
                    if draft.contains(tag) {
                        draft.retain(|t| t != tag);
                    } else {
                        draft.push(tag.clone());
                    }
                }
            }
            KeyCode::Char('a') => {
                draft = tags.clone();
            }
            KeyCode::Char('c') => {
                draft.clear();
            }
            KeyCode::Enter => {
                self.memory_tag_filter = draft;
                self.memory_dialog = MemoryDialog::None;
                self.refresh_memories().await;
                return;
            }
            KeyCode::Esc => {
                self.memory_dialog = MemoryDialog::None;
                return;
            }
            _ => {}
        }

        self.memory_dialog = MemoryDialog::TagFilter { cursor, draft };
    }

    /// Returns `true` if the application should exit.
    pub async fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Quit-confirmation dialog takes priority over everything else.
        if self.quitting {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => return true,
                _ => { self.quitting = false; return false; }
            }
        }

        // Memory dialogs take priority over normal view keys.
        if self.memory_dialog.is_open() {
            match &self.memory_dialog {
                MemoryDialog::Search(_) => self.handle_search_dialog_key(key).await,
                MemoryDialog::TagFilter { .. } => self.handle_tag_dialog_key(key).await,
                MemoryDialog::None => {}
            }
            return false;
        }

        if self.input_mode {
            match key.code {
                KeyCode::Esc => {
                    self.input_mode = false;
                }
                KeyCode::Enter => {
                    self.submit_message().await;
                }
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.input_insert('\n');
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.input_buffer.clear();
                    self.input_cursor = 0;
                    self.input_scroll = 0;
                }
                KeyCode::Char(c) => {
                    self.input_insert(c);
                }
                KeyCode::Backspace => {
                    self.input_delete_before();
                }
                KeyCode::Delete => {
                    self.input_delete_after();
                }
                KeyCode::Left => {
                    self.input_move_left();
                }
                KeyCode::Right => {
                    self.input_move_right();
                }
                KeyCode::Up => {
                    self.input_move_up();
                }
                KeyCode::Down => {
                    self.input_move_down();
                }
                KeyCode::Home => {
                    self.input_cursor = 0;
                }
                KeyCode::End => {
                    self.input_cursor = self.input_buffer.len();
                }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') => { self.quitting = true; }
            KeyCode::Tab => {
                let prev = self.active_tab;
                self.switch_tab(true);
                if self.active_tab == 2 && prev != 2 && self.memories.is_empty() {
                    self.refresh_memories().await;
                }
            }
            KeyCode::BackTab => {
                let prev = self.active_tab;
                self.switch_tab(false);
                if self.active_tab == 2 && prev != 2 && self.memories.is_empty() {
                    self.refresh_memories().await;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => match self.view {
                View::AgentDetail => self.scroll_conversation_down(u16::MAX),
                View::MemoryDetail => {
                    self.memory_scroll = self.memory_scroll.saturating_add(1);
                }
                View::Config => {
                    self.config_scroll = self.config_scroll.saturating_add(1);
                }
                View::WorkflowDetail => match self.workflow_focus {
                    WorkflowFocus::Template => {
                        self.workflow_template_scroll =
                            self.workflow_template_scroll.saturating_add(1);
                    }
                    _ => {
                        self.workflow_dispatch_scroll =
                            self.workflow_dispatch_scroll.saturating_add(1);
                    }
                },
                _ => self.select_next(),
            },
            KeyCode::Up | KeyCode::Char('k') => match self.view {
                View::AgentDetail => self.scroll_conversation_up(),
                View::MemoryDetail => {
                    self.memory_scroll = self.memory_scroll.saturating_sub(1);
                }
                View::Config => {
                    self.config_scroll = self.config_scroll.saturating_sub(1);
                }
                View::WorkflowDetail => match self.workflow_focus {
                    WorkflowFocus::Template => {
                        self.workflow_template_scroll =
                            self.workflow_template_scroll.saturating_sub(1);
                    }
                    _ => {
                        self.workflow_dispatch_scroll =
                            self.workflow_dispatch_scroll.saturating_sub(1);
                    }
                },
                _ => self.select_prev(),
            },
            KeyCode::Enter => {
                self.enter_detail().await;
            }
            KeyCode::Char('c') => {
                if self.view == View::Config {
                    self.exit_config();
                } else {
                    self.enter_config();
                }
            }
            KeyCode::Esc => {
                if self.view == View::Config {
                    self.exit_config();
                } else if self.view == View::WorkflowDetail
                    && self.workflow_focus != WorkflowFocus::None
                {
                    self.workflow_focus = WorkflowFocus::None;
                } else if matches!(self.view, View::MemoryList)
                    && (self.memory_search.is_some() || !self.memory_tag_filter.is_empty())
                {
                    // Clear active search/filter and reload
                    self.memory_search = None;
                    self.memory_tag_filter.clear();
                    self.refresh_memories().await;
                } else {
                    self.go_back();
                }
            }
            KeyCode::Char('r') => {
                if matches!(self.view, View::MemoryList | View::MemoryDetail) {
                    self.refresh_memories().await;
                } else {
                    self.refresh().await;
                }
            }
            KeyCode::Char('i') if self.view == View::AgentDetail => {
                self.input_mode = true;
            }
            KeyCode::Char('t') if self.view == View::WorkflowDetail => {
                self.workflow_focus = match self.workflow_focus {
                    WorkflowFocus::Template => WorkflowFocus::None,
                    _ => WorkflowFocus::Template,
                };
            }
            KeyCode::Char('d') if self.view == View::WorkflowDetail => {
                self.workflow_focus = match self.workflow_focus {
                    WorkflowFocus::Dispatches => WorkflowFocus::None,
                    _ => WorkflowFocus::Dispatches,
                };
            }
            // Memory: open search dialog
            KeyCode::Char('s') if self.view == View::MemoryList => {
                let current = self.memory_search.clone().unwrap_or_default();
                self.memory_dialog = MemoryDialog::Search(current);
            }
            // Memory: open tag filter dialog
            KeyCode::Char('t') if self.view == View::MemoryList => {
                let draft = self.memory_tag_filter.clone();
                self.memory_dialog = MemoryDialog::TagFilter { cursor: 0, draft };
            }
            _ => {}
        }
        false
    }

    pub fn secs_until_refresh(&self) -> u64 {
        self.refresh_interval
            .saturating_sub(self.last_refresh.elapsed())
            .as_secs()
    }
}
