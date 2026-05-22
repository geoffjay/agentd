use crate::config::TuiConfig;
use crate::stream;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use orchestrator::client::OrchestratorClient;
use orchestrator::scheduler::types::WorkflowResponse;
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
pub enum View {
    AgentList,
    AgentDetail,
    WorkflowList,
    WorkflowDetail,
}

pub struct App {
    pub view: View,

    pub agents: Vec<AgentResponse>,
    pub agent_table_state: TableState,
    pub selected_agent: Option<AgentResponse>,

    pub workflows: Vec<WorkflowResponse>,
    pub workflow_table_state: TableState,
    pub selected_workflow: Option<WorkflowResponse>,

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

    pub loading: bool,
    pub error: Option<String>,
    pub active_tab: usize,

    client: OrchestratorClient,
    orchestrator_url: String,
    last_refresh: Instant,
    refresh_interval: Duration,

    stream_rx: Option<mpsc::UnboundedReceiver<serde_json::Value>>,
    stream_abort: Option<tokio::task::AbortHandle>,
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
            conversation: Vec::new(),
            conversation_scroll: 0,
            conversation_follow: true,
            input_mode: false,
            input_buffer: String::new(),
            input_cursor: 0,
            input_scroll: 0,
            input_inner_width: 0,
            loading: false,
            error: None,
            active_tab: 0,
            orchestrator_url: config.orchestrator_url.clone(),
            client: OrchestratorClient::new(config.orchestrator_url),
            last_refresh: Instant::now() - Duration::from_secs(3600),
            refresh_interval: Duration::from_secs(config.refresh_interval_secs),
            stream_rx: None,
            stream_abort: None,
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

        self.loading = false;
        self.last_refresh = Instant::now();
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
                let next = self.workflow_table_state.selected().map_or(0, |i| (i + 1).min(len - 1));
                self.workflow_table_state.select(Some(next));
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
                let prev = self.workflow_table_state.selected().map_or(0, |i| i.saturating_sub(1));
                self.workflow_table_state.select(Some(prev));
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

    fn enter_workflow_detail(&mut self, wf: WorkflowResponse) {
        self.selected_workflow = Some(wf);
        self.view = View::WorkflowDetail;
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
                        self.enter_workflow_detail(wf);
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
            }
            _ => {}
        }
    }

    fn switch_tab(&mut self, forward: bool) {
        if forward {
            self.active_tab = (self.active_tab + 1) % 2;
        } else {
            self.active_tab = self.active_tab.checked_sub(1).unwrap_or(1);
        }
        self.view = if self.active_tab == 0 { View::AgentList } else { View::WorkflowList };
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

    /// Returns `true` if the application should exit.
    pub async fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Global quit bindings — always active.
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
            _ => {}
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
            KeyCode::Char('q') => return true,
            KeyCode::Tab => self.switch_tab(true),
            KeyCode::BackTab => self.switch_tab(false),
            KeyCode::Down | KeyCode::Char('j') => {
                if self.view == View::AgentDetail {
                    self.scroll_conversation_down(u16::MAX);
                } else {
                    self.select_next();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.view == View::AgentDetail {
                    self.scroll_conversation_up();
                } else {
                    self.select_prev();
                }
            }
            KeyCode::Enter => {
                self.enter_detail().await;
            }
            KeyCode::Esc => self.go_back(),
            KeyCode::Char('r') => self.refresh().await,
            KeyCode::Char('i') if self.view == View::AgentDetail => {
                self.input_mode = true;
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
