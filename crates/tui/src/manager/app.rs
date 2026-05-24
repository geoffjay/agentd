use crate::config::TuiConfig;
use crate::manager::queries::{self, PREDEFINED_QUERIES};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct QueryPickerState {
    pub cursor: usize,
    pub filter: String,
    pub filter_active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManagerView {
    Services,
    Logs,
    Config,
    Metrics,
}

#[derive(Debug, Clone)]
pub enum ServiceState {
    Unknown,
    Up(u64),
    Down(String),
}

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub url: String,
    pub state: ServiceState,
}

#[derive(Debug, Clone)]
pub enum LogSource {
    None,
    File(String),
}

#[derive(Debug, Clone)]
pub struct MetricSample {
    pub metric: String,
    pub value: String,
    pub timestamp: String,
}

pub struct ManagerApp {
    pub view: ManagerView,
    pub active_tab: usize,
    pub config: TuiConfig,

    // Services view
    pub services: Vec<ServiceStatus>,
    pub service_table_state: TableState,

    // Logs view
    pub log_source: LogSource,
    pub log_lines: VecDeque<String>,
    pub log_scroll: u16,
    pub log_source_input: Option<String>,

    // Config view
    pub config_fields: Vec<(String, String)>,
    pub config_selected: usize,
    pub config_edit: Option<String>,

    // Metrics view
    pub metrics_available: bool,
    pub metric_query: String,
    pub metric_query_cursor: usize,
    pub metric_input_active: bool,
    pub metric_results: Vec<MetricSample>,
    pub metric_error: Option<String>,
    pub query_picker: Option<QueryPickerState>,

    pub quitting: bool,
    pub error: Option<String>,
    last_refresh: Instant,
    refresh_interval: Duration,
}

impl ManagerApp {
    pub async fn new(config: TuiConfig) -> Self {
        let metrics_available = check_prometheus(&config.prometheus_url).await;
        let config_fields = build_config_fields(&config);
        Self {
            view: ManagerView::Services,
            active_tab: 0,
            config,
            services: Vec::new(),
            service_table_state: TableState::default(),
            log_source: LogSource::None,
            log_lines: VecDeque::with_capacity(500),
            log_scroll: 0,
            log_source_input: None,
            config_fields,
            config_selected: 0,
            config_edit: None,
            metrics_available,
            metric_query: String::new(),
            metric_query_cursor: 0,
            metric_input_active: false,
            metric_results: Vec::new(),
            metric_error: None,
            query_picker: None,
            quitting: false,
            error: None,
            last_refresh: Instant::now() - Duration::from_secs(3600),
            refresh_interval: Duration::from_secs(10),
        }
    }

    pub async fn refresh(&mut self) {
        self.services = probe_services(&self.config).await;
        if self.service_table_state.selected().is_none() && !self.services.is_empty() {
            self.service_table_state.select(Some(0));
        }
        if let LogSource::File(ref path) = self.log_source.clone() {
            self.tail_file(path);
        }
        self.last_refresh = Instant::now();
    }

    pub async fn tick(&mut self) {
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.refresh().await;
        }
    }

    fn tail_file(&mut self, path: &str) {
        use std::fs::File;
        use std::io::{BufRead, BufReader};
        if let Ok(f) = File::open(path) {
            let reader = BufReader::new(f);
            let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
            let start = lines.len().saturating_sub(500);
            self.log_lines.clear();
            for line in &lines[start..] {
                self.log_lines.push_back(line.clone());
            }
        }
    }

    pub async fn execute_metric_query(&mut self) {
        if self.metric_query.trim().is_empty() {
            return;
        }
        let url = format!("{}/api/v1/query", self.config.prometheus_url);
        let query = self.metric_query.trim().to_string();
        match query_prometheus(&url, &query).await {
            Ok(results) => {
                self.metric_results = results;
                self.metric_error = None;
            }
            Err(e) => {
                self.metric_error = Some(e.to_string());
                self.metric_results.clear();
            }
        }
    }

    pub fn tab_count(&self) -> usize {
        if self.metrics_available { 4 } else { 3 }
    }

    pub fn tab_labels(&self) -> Vec<String> {
        let mut labels = vec![
            " Services ".to_string(),
            " Logs ".to_string(),
            " Config ".to_string(),
        ];
        if self.metrics_available {
            labels.push(" Metrics ".to_string());
        }
        labels
    }

    fn switch_tab(&mut self, forward: bool) {
        let n = self.tab_count();
        if forward {
            self.active_tab = (self.active_tab + 1) % n;
        } else {
            self.active_tab = (self.active_tab + n - 1) % n;
        }
        self.view = tab_to_view(self.active_tab);
    }

    fn navigate_down(&mut self) {
        match self.view {
            ManagerView::Services => {
                let len = self.services.len();
                if len == 0 { return; }
                let next = self.service_table_state.selected().map_or(0, |i| (i + 1).min(len - 1));
                self.service_table_state.select(Some(next));
            }
            ManagerView::Config => {
                let len = self.config_fields.len();
                if self.config_selected + 1 < len {
                    self.config_selected += 1;
                }
            }
            ManagerView::Logs => {
                self.log_scroll = self.log_scroll.saturating_add(1);
            }
            ManagerView::Metrics => {}
        }
    }

    fn navigate_up(&mut self) {
        match self.view {
            ManagerView::Services => {
                if self.services.is_empty() { return; }
                let prev = self.service_table_state.selected().map_or(0, |i| i.saturating_sub(1));
                self.service_table_state.select(Some(prev));
            }
            ManagerView::Config => {
                self.config_selected = self.config_selected.saturating_sub(1);
            }
            ManagerView::Logs => {
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
            ManagerView::Metrics => {}
        }
    }

    fn apply_config_edit(&mut self, value: String) {
        if let Some((key, val)) = self.config_fields.get_mut(self.config_selected) {
            *val = value.clone();
            match key.as_str() {
                "orchestrator_url" => self.config.orchestrator_url = value,
                "memory_url" => self.config.memory_url = value,
                "prometheus_url" => self.config.prometheus_url = value,
                "refresh_interval_secs" => {
                    if let Ok(n) = value.parse::<u64>() {
                        self.config.refresh_interval_secs = n;
                        self.refresh_interval = Duration::from_secs(n);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn save_config(&mut self) -> anyhow::Result<()> {
        let path = agentd_common::config::config_file_path()
            .ok_or_else(|| anyhow::anyhow!("no config file path resolved"))?;
        let toml_str = toml::to_string_pretty(&self.config.agentd_config)?;
        std::fs::write(&path, toml_str)?;
        Ok(())
    }

    async fn handle_picker_key(&mut self, key: KeyEvent) -> bool {
        let Some(picker) = self.query_picker.as_mut() else { return false };

        if picker.filter_active {
            match key.code {
                KeyCode::Char(c) => {
                    picker.filter.push(c);
                    picker.cursor = 0;
                }
                KeyCode::Backspace => {
                    picker.filter.pop();
                    picker.cursor = 0;
                }
                KeyCode::Enter | KeyCode::Esc => {
                    picker.filter_active = false;
                }
                _ => {}
            }
            return false;
        }

        let filtered = queries::filtered_indices(&picker.filter);
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if !filtered.is_empty() && picker.cursor + 1 < filtered.len() {
                    picker.cursor += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                picker.cursor = picker.cursor.saturating_sub(1);
            }
            KeyCode::Char('/') => {
                picker.filter_active = true;
            }
            KeyCode::Enter => {
                if let Some(&idx) = filtered.get(picker.cursor) {
                    let q = PREDEFINED_QUERIES[idx].query.to_string();
                    self.metric_query = q;
                    self.metric_query_cursor = self.metric_query.len();
                    self.query_picker = None;
                    self.execute_metric_query().await;
                }
            }
            KeyCode::Esc => {
                self.query_picker = None;
            }
            _ => {}
        }
        false
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.quitting {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => return true,
                _ => {
                    self.quitting = false;
                    return false;
                }
            }
        }

        // Config inline field edit
        if self.view == ManagerView::Config {
            if let Some(ref mut draft) = self.config_edit {
                match key.code {
                    KeyCode::Char(c) => { draft.push(c); }
                    KeyCode::Backspace => { draft.pop(); }
                    KeyCode::Enter => {
                        let value = draft.clone();
                        self.apply_config_edit(value);
                        self.config_edit = None;
                    }
                    KeyCode::Esc => { self.config_edit = None; }
                    _ => {}
                }
                return false;
            }
        }

        // Log source picker
        if let Some(ref mut input) = self.log_source_input {
            match key.code {
                KeyCode::Char(c) => { input.push(c); }
                KeyCode::Backspace => { input.pop(); }
                KeyCode::Enter => {
                    let path = input.trim().to_string();
                    if path.is_empty() {
                        self.log_source = LogSource::None;
                        self.log_lines.clear();
                    } else {
                        self.log_source = LogSource::File(path.clone());
                        self.tail_file(&path);
                    }
                    self.log_source_input = None;
                }
                KeyCode::Esc => { self.log_source_input = None; }
                _ => {}
            }
            return false;
        }

        // Metrics picker dialog (takes priority over input mode)
        if self.view == ManagerView::Metrics && self.query_picker.is_some() {
            return self.handle_picker_key(key).await;
        }

        // Metric query input
        if self.view == ManagerView::Metrics && self.metric_input_active {
            match key.code {
                KeyCode::Char(c) => {
                    self.metric_query.insert(self.metric_query_cursor, c);
                    self.metric_query_cursor += c.len_utf8();
                }
                KeyCode::Backspace => {
                    if self.metric_query_cursor > 0 {
                        let prev = self.metric_query[..self.metric_query_cursor]
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        self.metric_query.drain(prev..self.metric_query_cursor);
                        self.metric_query_cursor = prev;
                    }
                }
                KeyCode::Left => {
                    if self.metric_query_cursor > 0 {
                        self.metric_query_cursor = self.metric_query[..self.metric_query_cursor]
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                    }
                }
                KeyCode::Right => {
                    if self.metric_query_cursor < self.metric_query.len() {
                        self.metric_query_cursor += self.metric_query[self.metric_query_cursor..]
                            .chars()
                            .next()
                            .map(|c| c.len_utf8())
                            .unwrap_or(0);
                    }
                }
                KeyCode::Enter => {
                    self.execute_metric_query().await;
                    self.metric_input_active = false;
                }
                KeyCode::Esc => { self.metric_input_active = false; }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') => { self.quitting = true; }
            KeyCode::Tab => self.switch_tab(true),
            KeyCode::BackTab => self.switch_tab(false),
            KeyCode::Char('1') => {
                self.active_tab = 0;
                self.view = ManagerView::Services;
            }
            KeyCode::Char('2') => {
                self.active_tab = 1;
                self.view = ManagerView::Logs;
            }
            KeyCode::Char('3') => {
                self.active_tab = 2;
                self.view = ManagerView::Config;
            }
            KeyCode::Char('4') if self.metrics_available => {
                self.active_tab = 3;
                self.view = ManagerView::Metrics;
            }
            KeyCode::Down | KeyCode::Char('j') => self.navigate_down(),
            KeyCode::Up | KeyCode::Char('k') => self.navigate_up(),
            KeyCode::Char('r') => { self.refresh().await; }
            // Config: start editing selected field
            KeyCode::Char('e') if self.view == ManagerView::Config => {
                if let Some((_, ref val)) = self.config_fields.get(self.config_selected).cloned() {
                    self.config_edit = Some(val.clone());
                }
            }
            // Config: save to disk
            KeyCode::Char('s') if self.view == ManagerView::Config => {
                if let Err(e) = self.save_config() {
                    self.error = Some(format!("save failed: {e}"));
                } else {
                    self.error = None;
                }
            }
            // Logs: open source picker
            KeyCode::Char('l') if self.view == ManagerView::Logs => {
                let current = match &self.log_source {
                    LogSource::File(p) => p.clone(),
                    LogSource::None => String::new(),
                };
                self.log_source_input = Some(current);
            }
            // Metrics: enter query input
            KeyCode::Char('i') if self.view == ManagerView::Metrics => {
                self.metric_input_active = true;
            }
            // Metrics: open predefined query picker
            KeyCode::Char('p') if self.view == ManagerView::Metrics => {
                self.query_picker = Some(QueryPickerState::default());
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

fn tab_to_view(tab: usize) -> ManagerView {
    match tab {
        0 => ManagerView::Services,
        1 => ManagerView::Logs,
        2 => ManagerView::Config,
        _ => ManagerView::Metrics,
    }
}

fn build_config_fields(config: &TuiConfig) -> Vec<(String, String)> {
    vec![
        ("orchestrator_url".to_string(), config.orchestrator_url.clone()),
        ("memory_url".to_string(), config.memory_url.clone()),
        ("prometheus_url".to_string(), config.prometheus_url.clone()),
        ("refresh_interval_secs".to_string(), config.refresh_interval_secs.to_string()),
    ]
}

async fn check_prometheus(url: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    client
        .get(format!("{url}/-/healthy"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn query_prometheus(url: &str, query: &str) -> anyhow::Result<Vec<MetricSample>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let resp = client.get(url).query(&[("query", query)]).send().await?;
    let json: serde_json::Value = resp.json().await?;

    let results = json["data"]["result"].as_array().cloned().unwrap_or_default();
    let samples = results
        .iter()
        .map(|r| {
            let metric_labels: Vec<String> = r["metric"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("")))
                        .collect()
                })
                .unwrap_or_default();
            let metric = if metric_labels.is_empty() {
                "(no labels)".to_string()
            } else {
                metric_labels.join(", ")
            };
            let (timestamp, value) = r["value"]
                .as_array()
                .and_then(|arr| {
                    let ts = arr.first()?.as_f64().map(|t| format!("{t:.0}"))?;
                    let v = arr.get(1)?.as_str()?.to_string();
                    Some((ts, v))
                })
                .unwrap_or_default();
            MetricSample { metric, value, timestamp }
        })
        .collect();

    Ok(samples)
}

const SERVICE_DEFS: &[(&str, &str, &str)] = &[
    ("orchestrator", "AGENTD_ORCHESTRATOR_SERVICE_URL", "http://localhost:17006"),
    ("notify",       "AGENTD_NOTIFY_SERVICE_URL",       "http://localhost:17004"),
    ("ask",          "AGENTD_ASK_SERVICE_URL",           "http://localhost:17001"),
    ("wrap",         "AGENTD_WRAP_SERVICE_URL",          "http://localhost:17005"),
    ("hook",         "AGENTD_HOOK_SERVICE_URL",          "http://localhost:17002"),
    ("monitor",      "AGENTD_MONITOR_SERVICE_URL",       "http://localhost:17003"),
    ("memory",       "AGENTD_MEMORY_SERVICE_URL",        "http://localhost:17008"),
    ("core",         "AGENTD_CORE_SERVICE_URL",          "http://localhost:17000"),
    ("communicate",  "AGENTD_COMMUNICATE_SERVICE_URL",   "http://localhost:17010"),
];

async fn probe_services(config: &TuiConfig) -> Vec<ServiceStatus> {
    let host = &config.agentd_config.general.host;
    let s = &config.agentd_config.services;

    // Build URL list: prefer env vars, then config-derived URLs, then defaults.
    let urls: Vec<(&str, String)> = SERVICE_DEFS
        .iter()
        .map(|(name, env_var, default)| {
            let url = std::env::var(env_var).unwrap_or_else(|_| match *name {
                "orchestrator" => config.orchestrator_url.clone(),
                "memory" => config.memory_url.clone(),
                "notify" => format!("http://{}:{}", host, s.notify.port),
                "ask" => format!("http://{}:{}", host, s.ask.port),
                "wrap" => format!("http://{}:{}", host, s.wrap.port),
                "hook" => format!("http://{}:{}", host, s.hook.port),
                "monitor" => format!("http://{}:{}", host, s.monitor.port),
                "core" => format!("http://{}:{}", host, s.core.port),
                "communicate" => format!("http://{}:{}", host, s.communicate.port),
                _ => default.to_string(),
            });
            (*name, url)
        })
        .collect();

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return urls
                .into_iter()
                .map(|(name, url)| ServiceStatus {
                    name: name.to_string(),
                    url,
                    state: ServiceState::Unknown,
                })
                .collect();
        }
    };

    let mut handles = Vec::new();
    for (name, url) in urls {
        let c = client.clone();
        let health_url = format!("{url}/health");
        let name = name.to_string();
        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let state = match c.get(&health_url).send().await {
                Ok(r) if r.status().is_success() => {
                    ServiceState::Up(start.elapsed().as_millis() as u64)
                }
                Ok(r) => ServiceState::Down(format!("HTTP {}", r.status())),
                Err(e) => {
                    let msg = if e.is_connect() || e.is_timeout() {
                        "unreachable".to_string()
                    } else {
                        e.to_string()
                    };
                    ServiceState::Down(msg)
                }
            };
            ServiceStatus { name, url, state }
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(status) = h.await {
            results.push(status);
        }
    }
    results
}
