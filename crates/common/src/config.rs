//! Centralised configuration for all agentd services.
//!
//! Provides a single [`AgentdConfig`] struct that covers every service, a
//! TOML-file schema, XDG-compliant file discovery, and a [`load()`] function
//! that merges three layers in ascending precedence order:
//!
//! ```text
//! compiled defaults  <  config file  <  environment variables
//! ```
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use agentd_common::config;
//!
//! let cfg = config::load()?;
//! println!("notify port: {}", cfg.services.notify.port);
//! println!("log level:   {}", cfg.general.log_level);
//! ```
//!
//! # Config File Location
//!
//! Searched in order:
//! 1. Path in `AGENTD_CONFIG` environment variable (if set and non-empty)
//! 2. `$XDG_CONFIG_HOME/agentd/config.toml`
//! 3. `~/.config/agentd/config.toml`
//!
//! A missing file is not an error — defaults and env vars still apply.
//!
//! # Environment Variable Overrides
//!
//! | Variable                             | Config field                                  |
//! |--------------------------------------|-----------------------------------------------|
//! | `AGENTD_LOG_LEVEL`                   | `general.log_level`                           |
//! | `AGENTD_LOG_FORMAT`                  | `general.log_format`                          |
//! | `AGENTD_HOST`                        | `general.host`                                |
//! | `AGENTD_ASK_PORT`                    | `services.ask.port`                           |
//! | `AGENTD_ASK_ORCHESTRATOR_URL`        | `services.ask.orchestrator_url`               |
//! | `AGENTD_NOTIFY_PORT`                 | `services.notify.port`                        |
//! | `AGENTD_ORCHESTRATOR_PORT`           | `services.orchestrator.port`                  |
//! | `AGENTD_ORCHESTRATOR_BACKEND`        | `services.orchestrator.backend`               |
//! | `AGENTD_ORCHESTRATOR_COMMUNICATE_URL`| `services.orchestrator.communicate_url` *(replaces legacy `AGENTD_COMMUNICATE_SERVICE_URL`)* |
//! | `AGENTD_WRAP_PORT`                   | `services.wrap.port`                          |
//! | `AGENTD_WRAP_BACKEND`                | `services.wrap.backend`                       |
//! | `AGENTD_MEMORY_PORT`                 | `services.memory.port`                        |
//! | `AGENTD_MEMORY_EMBEDDING_PROVIDER`   | `services.memory.embedding_provider`          |
//! | `AGENTD_MEMORY_EMBEDDING_MODEL`      | `services.memory.embedding_model`             |
//! | `AGENTD_MEMORY_LANCE_PATH`           | `services.memory.lance_path`                  |
//! | `AGENTD_HOOK_PORT`                   | `services.hook.port`                          |
//! | `AGENTD_HISTORY_SIZE`                | `services.hook.history_size`                  |
//! | `AGENTD_NOTIFY_SERVICE_URL`          | `services.hook.notify_service_url`            |
//! | `AGENTD_MONITOR_PORT`                | `services.monitor.port`                       |
//! | `AGENTD_COLLECTION_INTERVAL_SECS`    | `services.monitor.collection_interval_secs`   |
//! | `AGENTD_COMMUNICATE_PORT`            | `services.communicate.port`                   |
//! | `AGENTD_CORE_PORT`                   | `services.core.port`                          |
//! | `AGENTD_MCP_ORCHESTRATOR_URL`        | `services.mcp.orchestrator_url`               |
//! | `AGENTD_MCP_NOTIFY_URL`              | `services.mcp.notify_url`                     |
//! | `AGENTD_MCP_ASK_URL`                 | `services.mcp.ask_url`                        |
//! | `AGENTD_MCP_MEMORY_URL`              | `services.mcp.memory_url`                     |
//! | `AGENTD_MCP_COMMUNICATE_URL`         | `services.mcp.communicate_url`                |
//! | `AGENTD_MCP_WRAP_URL`                | `services.mcp.wrap_url`                       |
//! | `AGENTD_MCP_MONITOR_URL`             | `services.mcp.monitor_url`                    |
//! | `AGENTD_MCP_HOOK_URL`                | `services.mcp.hook_url`                       |
//! | `AGENTD_CORE_SERVICE_URL`            | `apps.cli.core_url`                           |
//! | `AGENTD_RECONCILE_INTERVAL_SECS`     | `services.orchestrator.reconcile_interval_secs` |
//! | `AGENTD_UI_PORT`                     | `services.ui.port`                            |
//! | `AGENTD_UI_DIR`                      | `services.ui.ui_dir`                          |
//! | `AGENTD_RECONCILE_INTERVAL_SECS`     | `services.orchestrator.reconcile_interval_secs` |
//! | `AGENTD_COLLECTION_INTERVAL_SECS`    | `services.monitor.collection_interval_secs`     |
//! | `AGENTD_NOTIFY_SERVICE_URL`          | `services.hook.notify_service_url`              |
//! | `AGENTD_NOTIFY_ON_FAILURE`           | `services.hook.notify_on_failure`               |
//! | `AGENTD_NOTIFY_ON_LONG_RUNNING`      | `services.hook.notify_on_long_running`          |
//! | `AGENTD_LONG_RUNNING_THRESHOLD_MS`   | `services.hook.long_running_threshold_ms`       |
//! | `AGENTD_CPU_ALERT_THRESHOLD`         | `services.monitor.cpu_alert_threshold`          |
//! | `AGENTD_MEMORY_ALERT_THRESHOLD`      | `services.monitor.memory_alert_threshold`       |
//! | `AGENTD_DISK_ALERT_THRESHOLD`        | `services.monitor.disk_alert_threshold`         |
//! | `AGENTD_MONITOR_HISTORY_SIZE`        | `services.monitor.history_size`                 |
//! | `AGENTD_MCP_NOTIFY_URL`              | `services.mcp.notify_url`                       |
//! | `AGENTD_MCP_ASK_URL`                 | `services.mcp.ask_url`                          |
//! | `AGENTD_MCP_MEMORY_URL`              | `services.mcp.memory_url`                       |
//! | `AGENTD_MCP_COMMUNICATE_URL`         | `services.mcp.communicate_url`                  |
//! | `AGENTD_MCP_WRAP_URL`                | `services.mcp.wrap_url`                         |
//! | `AGENTD_MCP_MONITOR_URL`             | `services.mcp.monitor_url`                      |
//! | `AGENTD_MCP_HOOK_URL`                | `services.mcp.hook_url`                         |

use anyhow::{bail, Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// ValidateConfig trait
// ---------------------------------------------------------------------------

/// Trait for validating a configuration section.
///
/// Implementations should return `Ok(())` when the configuration is valid, or
/// an error with a descriptive message indicating which field is invalid and why.
pub trait ValidateConfig {
    fn validate(&self) -> Result<()>;
}

// ---------------------------------------------------------------------------
// GeneralConfig
// ---------------------------------------------------------------------------

/// Cross-cutting settings that apply to all services.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GeneralConfig {
    /// Minimum log level filter (`trace`, `debug`, `info`, `warn`, `error`).
    ///
    /// Defaults to `"info"`.
    pub log_level: String,

    /// Log output format: `"text"` (human-readable) or `"json"` (structured).
    ///
    /// Defaults to `"text"`.
    pub log_format: String,

    /// Default bind host for all services.
    ///
    /// Defaults to `"127.0.0.1"`.
    pub host: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            log_format: "text".to_string(),
            host: "127.0.0.1".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Per-service config structs
// ---------------------------------------------------------------------------

/// Configuration for the `agentd-ask` service (port 17001).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AskConfig {
    /// HTTP listen port. Defaults to `17001`.
    pub port: u16,
    /// Orchestrator callback URL. Defaults to `"http://localhost:17006"`.
    pub orchestrator_url: String,
}

impl Default for AskConfig {
    fn default() -> Self {
        Self { port: 17001, orchestrator_url: "http://localhost:17006".to_string() }
    }
}

/// Configuration for the `agentd-notify` service (port 17004).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotifyConfig {
    /// HTTP listen port. Defaults to `17004`.
    pub port: u16,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self { port: 17004 }
    }
}

/// Configuration for the `agentd-orchestrator` service (port 17006).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OrchestratorConfig {
    /// HTTP listen port. Defaults to `17006`.
    pub port: u16,
    /// Execution backend: `"tmux"`, `"docker"`, `"pty"`, or `"subprocess"`.
    ///
    /// Defaults to `"tmux"`.
    pub backend: String,
    /// Communicate service URL for agent message delivery.
    ///
    /// Defaults to `"http://localhost:17010"`.
    pub communicate_url: String,
    /// Agent reconciliation interval in seconds. Defaults to `30`.
    pub reconcile_interval_secs: u64,
    /// `PATH` injected into spawned subprocesses when using the subprocess
    /// backend.  Empty string means inherit the orchestrator process's own
    /// PATH.  Set this to the user's full PATH (e.g. including
    /// `~/.cargo/bin`, asdf shims, nodenv shims) so that `claude` and other
    /// tools are locatable when the service runs as a LaunchAgent/systemd
    /// unit with a bare system PATH.
    ///
    /// Defaults to `""` (inherit).
    pub subprocess_path: String,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            port: 17006,
            backend: "tmux".to_string(),
            communicate_url: "http://localhost:17010".to_string(),
            reconcile_interval_secs: 30,
            subprocess_path: String::new(),
        }
    }
}

/// Configuration for the `agentd-wrap` service (port 17005).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WrapConfig {
    /// HTTP listen port. Defaults to `17005`.
    pub port: u16,
    /// Execution backend: `"tmux"`, `"docker"`, `"pty"`, or `"subprocess"`.
    ///
    /// Defaults to `"tmux"`.
    pub backend: String,
}

impl Default for WrapConfig {
    fn default() -> Self {
        Self { port: 17005, backend: "tmux".to_string() }
    }
}

/// Configuration for the `agentd-memory` service (port 17008).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MemoryConfig {
    /// HTTP listen port. Defaults to `17008`.
    pub port: u16,
    /// Embedding provider: `"openai"` or `"none"`. Defaults to `"none"`.
    pub embedding_provider: String,
    /// Embedding model name. Defaults to `"text-embedding-3-small"`.
    pub embedding_model: String,
    /// LanceDB directory path. Defaults to XDG data dir.
    pub lance_path: String,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            port: 17008,
            embedding_provider: "none".to_string(),
            embedding_model: "text-embedding-3-small".to_string(),
            lance_path: default_memory_lance_path(),
        }
    }
}

/// Configuration for the `agentd-knowledge` service (port 17011).
///
/// | Field  | Env var                   | Default                                |
/// |--------|---------------------------|----------------------------------------|
/// | `port` | `AGENTD_KNOWLEDGE_PORT`   | `17011`                                |
/// | `root` | `AGENTD_KNOWLEDGE_ROOT`   | XDG data dir for `agentd-knowledge`    |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct KnowledgeConfig {
    /// HTTP listen port. Override with `AGENTD_KNOWLEDGE_PORT`. Defaults to `17011`.
    pub port: u16,
    /// Root directory for markdown document storage.
    ///
    /// Each project gets a subdirectory `<root>/<project_uuid>/`.
    /// Override with `AGENTD_KNOWLEDGE_ROOT`. Defaults to the XDG data dir
    /// for `agentd-knowledge` (e.g. `~/.local/share/agentd-knowledge/docs`).
    pub root: String,
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self { port: 17011, root: default_knowledge_docs_path() }
    }
}

/// Configuration for the `agentd-hook` service (port 17002).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HookConfig {
    /// HTTP listen port. Defaults to `17002`.
    pub port: u16,
    /// Maximum shell-event history retained in memory. Defaults to `500`.
    pub history_size: usize,
    /// Optional notify service URL for forwarding notable events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_service_url: Option<String>,
    /// Send a notification when a command exits non-zero. Defaults to `true`.
    pub notify_on_failure: bool,
    /// Send a notification when a command runs longer than the threshold. Defaults to `true`.
    pub notify_on_long_running: bool,
    /// Minimum duration in milliseconds to consider a command "long-running". Defaults to `30_000`.
    pub long_running_threshold_ms: u64,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            port: 17002,
            history_size: 500,
            notify_service_url: None,
            notify_on_failure: true,
            notify_on_long_running: true,
            long_running_threshold_ms: 30_000,
        }
    }
}

/// Configuration for the `agentd-monitor` service (port 17003).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MonitorConfig {
    /// HTTP listen port. Defaults to `17003`.
    pub port: u16,
    /// Metrics collection interval in seconds. Defaults to `15`.
    pub collection_interval_secs: u64,
    /// CPU usage % above which an alert is raised. Defaults to `90.0`.
    pub cpu_alert_threshold: f64,
    /// Memory usage % above which an alert is raised. Defaults to `90.0`.
    pub memory_alert_threshold: f64,
    /// Disk usage % above which an alert is raised. Defaults to `90.0`.
    pub disk_alert_threshold: f64,
    /// Maximum number of metric snapshots to retain in memory. Defaults to `120`.
    pub history_size: usize,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            port: 17003,
            collection_interval_secs: 15,
            cpu_alert_threshold: 90.0,
            memory_alert_threshold: 90.0,
            disk_alert_threshold: 90.0,
            history_size: 120,
        }
    }
}

/// Configuration for the `agentd-communicate` service (port 17010).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CommunicateConfig {
    /// HTTP listen port. Defaults to `17010`.
    pub port: u16,
}

impl Default for CommunicateConfig {
    fn default() -> Self {
        Self { port: 17010 }
    }
}

/// Configuration for the `agentd-core` service (port 17000).
///
/// The core service is also the API gateway: it reverse-proxies
/// `/api/v1/<service>/*` to the upstream services listed below. Each
/// `*_url` is the base URL the gateway forwards to. Set these when the
/// upstream services do not listen on their default `127.0.0.1:17xxx`
/// addresses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CoreConfig {
    /// HTTP listen port. Defaults to `17000`.
    pub port: u16,
    /// Upstream URL for the orchestrator service. Defaults to `"http://localhost:17006"`.
    pub orchestrator_url: String,
    /// Upstream URL for the notify service. Defaults to `"http://localhost:17004"`.
    pub notify_url: String,
    /// Upstream URL for the ask service. Defaults to `"http://localhost:17001"`.
    pub ask_url: String,
    /// Upstream URL for the wrap service. Defaults to `"http://localhost:17005"`.
    pub wrap_url: String,
    /// Upstream URL for the hook service. Defaults to `"http://localhost:17002"`.
    pub hook_url: String,
    /// Upstream URL for the monitor service. Defaults to `"http://localhost:17003"`.
    pub monitor_url: String,
    /// Upstream URL for the memory service. Defaults to `"http://localhost:17008"`.
    pub memory_url: String,
    /// Upstream URL for the communicate service. Defaults to `"http://localhost:17010"`.
    pub communicate_url: String,
    /// Upstream URL for the knowledge service. Defaults to `"http://localhost:17011"`.
    pub knowledge_url: String,
    /// Upstream URL for the index service. Defaults to `"http://localhost:17012"`.
    pub index_url: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            port: 17000,
            orchestrator_url: "http://localhost:17006".to_string(),
            notify_url: "http://localhost:17004".to_string(),
            ask_url: "http://localhost:17001".to_string(),
            wrap_url: "http://localhost:17005".to_string(),
            hook_url: "http://localhost:17002".to_string(),
            monitor_url: "http://localhost:17003".to_string(),
            memory_url: "http://localhost:17008".to_string(),
            communicate_url: "http://localhost:17010".to_string(),
            knowledge_url: "http://localhost:17011".to_string(),
            index_url: "http://localhost:17012".to_string(),
        }
    }
}

/// Configuration for the `agentd-mcp` server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct McpConfig {
    /// Orchestrator service URL. Defaults to `"http://localhost:17006"`.
    pub orchestrator_url: String,
    /// Notify service URL. Defaults to `"http://localhost:17004"`.
    pub notify_url: String,
    /// Ask service URL. Defaults to `"http://localhost:17001"`.
    pub ask_url: String,
    /// Memory service URL. Defaults to `"http://localhost:17008"`.
    pub memory_url: String,
    /// Communicate service URL. Defaults to `"http://localhost:17010"`.
    pub communicate_url: String,
    /// Wrap service URL. Defaults to `"http://localhost:17005"`.
    pub wrap_url: String,
    /// Monitor service URL. Defaults to `"http://localhost:17003"`.
    pub monitor_url: String,
    /// Hook service URL. Defaults to `"http://localhost:17002"`.
    pub hook_url: String,
    /// Knowledge service URL. Defaults to `"http://localhost:17011"`.
    pub knowledge_url: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            orchestrator_url: "http://localhost:17006".to_string(),
            notify_url: "http://localhost:17004".to_string(),
            ask_url: "http://localhost:17001".to_string(),
            memory_url: "http://localhost:17008".to_string(),
            communicate_url: "http://localhost:17010".to_string(),
            wrap_url: "http://localhost:17005".to_string(),
            monitor_url: "http://localhost:17003".to_string(),
            hook_url: "http://localhost:17002".to_string(),
            knowledge_url: "http://localhost:17011".to_string(),
        }
    }
}

/// Configuration for the `agentd-ui` server (port 17009).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UiConfig {
    /// HTTP listen port. Defaults to `17009`.
    pub port: u16,
    /// Directory containing the compiled frontend assets.
    ///
    /// Defaults to `"./ui/dist"`.
    pub ui_dir: String,
    /// Explicit browser-facing URL overrides per service, served to the SPA
    /// via `GET /config.json`.
    ///
    /// By default the SPA reaches each service at
    /// `<page protocol>//<page hostname>:<service port>`, which is correct
    /// when the service ports are reachable from the browser. Deployments
    /// that front services with a reverse proxy or TLS can override the full
    /// URL per service instead:
    ///
    /// ```toml
    /// [services.ui.public_urls]
    /// orchestrator = "https://agentd.example.com/orchestrator"
    /// ```
    pub public_urls: std::collections::BTreeMap<String, String>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            port: 17009,
            ui_dir: "./ui/dist".to_string(),
            public_urls: std::collections::BTreeMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// App (client) config structs
// ---------------------------------------------------------------------------

/// Configuration for the `agent` command-line interface.
///
/// Unlike the `[services.*]` sections, which configure servers when they bind,
/// this section configures a *client*: where the CLI sends its requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CliConfig {
    /// Base URL of the core auth gateway that fronts all services.
    ///
    /// Every `agent` subcommand is routed through this gateway as
    /// `<core_url>/api/v1/<service>`. Defaults to `"http://localhost:17000"`.
    ///
    /// Overridden at runtime by the `AGENTD_CORE_SERVICE_URL` environment
    /// variable when set.
    pub core_url: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self { core_url: "http://localhost:17000".to_string() }
    }
}

// ---------------------------------------------------------------------------
// AppsConfig
// ---------------------------------------------------------------------------

/// Container for client-application configuration sections (as opposed to the
/// server `[services.*]` sections).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppsConfig {
    pub cli: CliConfig,
}

// ---------------------------------------------------------------------------
// ServicesConfig
// ---------------------------------------------------------------------------

/// Container for all per-service configuration sections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ServicesConfig {
    pub ask: AskConfig,
    pub notify: NotifyConfig,
    pub orchestrator: OrchestratorConfig,
    pub wrap: WrapConfig,
    pub memory: MemoryConfig,
    pub knowledge: KnowledgeConfig,
    pub hook: HookConfig,
    pub monitor: MonitorConfig,
    pub communicate: CommunicateConfig,
    pub core: CoreConfig,
    pub mcp: McpConfig,
    pub ui: UiConfig,
}

// ---------------------------------------------------------------------------
// AgentdConfig
// ---------------------------------------------------------------------------

/// Top-level configuration struct for the entire agentd system.
///
/// Can be loaded from a TOML config file, environment variables, or both via
/// [`load()`].  Each section corresponds to one service or cross-cutting concern.
///
/// # Example TOML
///
/// ```toml
/// [general]
/// log_level = "debug"
/// log_format = "json"
///
/// [services.notify]
/// port = 17004
///
/// [services.orchestrator]
/// backend = "docker"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AgentdConfig {
    /// Cross-cutting settings (log level, format, host).
    pub general: GeneralConfig,
    /// Per-service configuration sections.
    pub services: ServicesConfig,
    /// Client-application configuration sections (e.g. the CLI).
    pub apps: AppsConfig,
}

// ---------------------------------------------------------------------------
// ValidateConfig implementations for shared config sections
// ---------------------------------------------------------------------------

/// Returns `Err` if the port is 0.
fn validate_port(port: u16, service: &str) -> Result<()> {
    if port == 0 {
        bail!("{service}.port must be non-zero");
    }
    Ok(())
}

/// Returns `Err` if the string does not start with `http://` or `https://`.
///
/// Service crates can import this helper to avoid duplicating the same check
/// in their own `ValidateConfig` implementations.
pub fn validate_url(url: &str, field: &str) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        bail!("{field} must be a valid HTTP/HTTPS URL, got: {url}");
    }
    Ok(())
}

impl ValidateConfig for AskConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "ask")
    }
}

impl ValidateConfig for NotifyConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "notify")
    }
}

impl ValidateConfig for OrchestratorConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "orchestrator")?;
        match self.backend.as_str() {
            "tmux" | "docker" | "pty" | "subprocess" => {}
            other => bail!(
                "orchestrator.backend must be one of tmux, docker, pty, subprocess; got: {other}"
            ),
        }
        Ok(())
    }
}

impl ValidateConfig for WrapConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "wrap")?;
        match self.backend.as_str() {
            "tmux" | "docker" | "pty" | "subprocess" => {}
            other => {
                bail!("wrap.backend must be one of tmux, docker, pty, subprocess; got: {other}")
            }
        }
        Ok(())
    }
}

impl ValidateConfig for MemoryConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "memory")?;
        match self.embedding_provider.as_str() {
            "none" | "ollama" | "openai" => {}
            other => {
                bail!("memory.embedding_provider must be one of none, ollama, openai; got: {other}")
            }
        }
        Ok(())
    }
}

impl ValidateConfig for KnowledgeConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "knowledge")?;
        if self.root.is_empty() {
            bail!("knowledge.root must not be empty");
        }
        Ok(())
    }
}

impl ValidateConfig for HookConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "hook")?;
        if self.history_size == 0 {
            bail!("hook.history_size must be greater than 0");
        }
        if let Some(ref url) = self.notify_service_url {
            validate_url(url, "hook.notify_service_url")?;
        }
        Ok(())
    }
}

impl ValidateConfig for MonitorConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "monitor")?;
        if self.collection_interval_secs == 0 {
            bail!("monitor.collection_interval_secs must be greater than 0");
        }
        Ok(())
    }
}

impl ValidateConfig for McpConfig {
    fn validate(&self) -> Result<()> {
        for (name, url) in [
            ("mcp.orchestrator_url", self.orchestrator_url.as_str()),
            ("mcp.notify_url", self.notify_url.as_str()),
            ("mcp.ask_url", self.ask_url.as_str()),
            ("mcp.memory_url", self.memory_url.as_str()),
            ("mcp.communicate_url", self.communicate_url.as_str()),
            ("mcp.wrap_url", self.wrap_url.as_str()),
            ("mcp.monitor_url", self.monitor_url.as_str()),
            ("mcp.hook_url", self.hook_url.as_str()),
            ("mcp.knowledge_url", self.knowledge_url.as_str()),
        ] {
            validate_url(url, name)?;
        }
        Ok(())
    }
}

impl ValidateConfig for UiConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "ui")?;
        if self.ui_dir.is_empty() {
            bail!("ui.ui_dir must not be empty");
        }
        Ok(())
    }
}

impl ValidateConfig for CoreConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "core")?;
        for (name, url) in [
            ("core.orchestrator_url", self.orchestrator_url.as_str()),
            ("core.notify_url", self.notify_url.as_str()),
            ("core.ask_url", self.ask_url.as_str()),
            ("core.wrap_url", self.wrap_url.as_str()),
            ("core.hook_url", self.hook_url.as_str()),
            ("core.monitor_url", self.monitor_url.as_str()),
            ("core.memory_url", self.memory_url.as_str()),
            ("core.communicate_url", self.communicate_url.as_str()),
            ("core.knowledge_url", self.knowledge_url.as_str()),
            ("core.index_url", self.index_url.as_str()),
        ] {
            validate_url(url, name)?;
        }
        Ok(())
    }
}

impl ValidateConfig for CommunicateConfig {
    fn validate(&self) -> Result<()> {
        validate_port(self.port, "communicate")
    }
}

impl ValidateConfig for CliConfig {
    fn validate(&self) -> Result<()> {
        validate_url(&self.core_url, "apps.cli.core_url")
    }
}

impl AgentdConfig {
    /// Validate all service configuration sections.
    ///
    /// Collects errors from every section rather than stopping at the first
    /// failure, so operators see all problems at once.
    ///
    /// # Errors
    ///
    /// Returns an error listing all invalid settings if any section fails
    /// validation.
    pub fn validate(&self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();

        let checks: &[(&str, Result<()>)] = &[
            ("[services.ask]", self.services.ask.validate()),
            ("[services.notify]", self.services.notify.validate()),
            ("[services.orchestrator]", self.services.orchestrator.validate()),
            ("[services.wrap]", self.services.wrap.validate()),
            ("[services.memory]", self.services.memory.validate()),
            ("[services.knowledge]", self.services.knowledge.validate()),
            ("[services.hook]", self.services.hook.validate()),
            ("[services.monitor]", self.services.monitor.validate()),
            ("[services.mcp]", self.services.mcp.validate()),
            ("[services.ui]", self.services.ui.validate()),
            ("[services.core]", self.services.core.validate()),
            ("[services.communicate]", self.services.communicate.validate()),
            ("[apps.cli]", self.apps.cli.validate()),
        ];

        for (section, result) in checks {
            if let Err(e) = result {
                errors.push(format!("{section}: {e}"));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            bail!("configuration validation failed:\n  {}", errors.join("\n  "))
        }
    }
}

// ---------------------------------------------------------------------------
// Config file discovery
// ---------------------------------------------------------------------------

/// Returns the path to the config file, in priority order:
///
/// 1. `AGENTD_CONFIG` env var (if set and non-empty)
/// 2. `$XDG_CONFIG_HOME/agentd/config.toml`
/// 3. `~/.config/agentd/config.toml`
pub fn config_file_path() -> Option<PathBuf> {
    // 1. Explicit override
    if let Ok(p) = env::var("AGENTD_CONFIG") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }

    // 2 & 3. XDG / home fallback
    ProjectDirs::from("", "", "agentd").map(|dirs| dirs.config_dir().join("config.toml"))
}

// ---------------------------------------------------------------------------
// Layered load
// ---------------------------------------------------------------------------

/// Load [`AgentdConfig`] using three-layer precedence:
///
/// ```text
/// compiled defaults  <  TOML config file  <  environment variables
/// ```
///
/// The config file path is resolved via [`config_file_path()`].  A missing
/// file is silently ignored; only a malformed file returns an error.
pub fn load() -> Result<AgentdConfig> {
    load_from_path(config_file_path().as_deref())
}

/// Load [`AgentdConfig`] from an explicit file path (or no file when `None`),
/// then apply environment variable overrides.
///
/// This is the testable entry point — it avoids touching the `AGENTD_CONFIG`
/// env var so tests can call it concurrently without races.
pub fn load_from_path(path: Option<&std::path::Path>) -> Result<AgentdConfig> {
    // Layer 1: compiled defaults
    let mut cfg = AgentdConfig::default();

    // Layer 2: TOML config file (if present)
    if let Some(path) = path {
        if path.exists() {
            let contents = std::fs::read_to_string(path)
                .with_context(|| format!("reading config file {}", path.display()))?;
            let file_cfg: AgentdConfig = toml::from_str(&contents)
                .with_context(|| format!("parsing config file {}", path.display()))?;
            cfg = merge(cfg, file_cfg);
        }
    }

    // Layer 3: environment variables
    apply_env_overrides(&mut cfg);

    Ok(cfg)
}

/// Merge `file` values on top of `base`, preferring `file` for non-default
/// scalar fields.
///
/// Because every struct derives `Default` and uses `#[serde(default)]`, any
/// field that was absent in the TOML file will equal its `Default` value in
/// `file_cfg`.  We therefore treat the file value as an override only when it
/// differs from the compiled default.
///
/// Note: `base` is always `AgentdConfig::default()` at all call sites. The
/// function treats `file == default` as "not set in file" — this means a
/// config file cannot explicitly reset a field back to its compiled default
/// after a prior layer set it to something else.
fn merge(base: AgentdConfig, file: AgentdConfig) -> AgentdConfig {
    let d = AgentdConfig::default();

    AgentdConfig {
        general: GeneralConfig {
            log_level: pick(&base.general.log_level, &file.general.log_level, &d.general.log_level),
            log_format: pick(
                &base.general.log_format,
                &file.general.log_format,
                &d.general.log_format,
            ),
            host: pick(&base.general.host, &file.general.host, &d.general.host),
        },
        services: ServicesConfig {
            ask: AskConfig {
                port: pick_u16(base.services.ask.port, file.services.ask.port, d.services.ask.port),
                orchestrator_url: pick(
                    &base.services.ask.orchestrator_url,
                    &file.services.ask.orchestrator_url,
                    &d.services.ask.orchestrator_url,
                ),
            },
            notify: NotifyConfig {
                port: pick_u16(
                    base.services.notify.port,
                    file.services.notify.port,
                    d.services.notify.port,
                ),
            },
            orchestrator: OrchestratorConfig {
                port: pick_u16(
                    base.services.orchestrator.port,
                    file.services.orchestrator.port,
                    d.services.orchestrator.port,
                ),
                backend: pick(
                    &base.services.orchestrator.backend,
                    &file.services.orchestrator.backend,
                    &d.services.orchestrator.backend,
                ),
                communicate_url: pick(
                    &base.services.orchestrator.communicate_url,
                    &file.services.orchestrator.communicate_url,
                    &d.services.orchestrator.communicate_url,
                ),
                reconcile_interval_secs: pick_u64(
                    base.services.orchestrator.reconcile_interval_secs,
                    file.services.orchestrator.reconcile_interval_secs,
                    d.services.orchestrator.reconcile_interval_secs,
                ),
                subprocess_path: pick(
                    &base.services.orchestrator.subprocess_path,
                    &file.services.orchestrator.subprocess_path,
                    &d.services.orchestrator.subprocess_path,
                ),
            },
            wrap: WrapConfig {
                port: pick_u16(
                    base.services.wrap.port,
                    file.services.wrap.port,
                    d.services.wrap.port,
                ),
                backend: pick(
                    &base.services.wrap.backend,
                    &file.services.wrap.backend,
                    &d.services.wrap.backend,
                ),
            },
            memory: MemoryConfig {
                port: pick_u16(
                    base.services.memory.port,
                    file.services.memory.port,
                    d.services.memory.port,
                ),
                embedding_provider: pick(
                    &base.services.memory.embedding_provider,
                    &file.services.memory.embedding_provider,
                    &d.services.memory.embedding_provider,
                ),
                embedding_model: pick(
                    &base.services.memory.embedding_model,
                    &file.services.memory.embedding_model,
                    &d.services.memory.embedding_model,
                ),
                lance_path: pick(
                    &base.services.memory.lance_path,
                    &file.services.memory.lance_path,
                    &d.services.memory.lance_path,
                ),
            },
            knowledge: KnowledgeConfig {
                port: pick_u16(
                    base.services.knowledge.port,
                    file.services.knowledge.port,
                    d.services.knowledge.port,
                ),
                root: pick(
                    &base.services.knowledge.root,
                    &file.services.knowledge.root,
                    &d.services.knowledge.root,
                ),
            },
            hook: HookConfig {
                port: pick_u16(
                    base.services.hook.port,
                    file.services.hook.port,
                    d.services.hook.port,
                ),
                history_size: pick_usize(
                    base.services.hook.history_size,
                    file.services.hook.history_size,
                    d.services.hook.history_size,
                ),
                notify_service_url: file
                    .services
                    .hook
                    .notify_service_url
                    .or(base.services.hook.notify_service_url),
                notify_on_failure: if file.services.hook.notify_on_failure
                    != d.services.hook.notify_on_failure
                {
                    file.services.hook.notify_on_failure
                } else {
                    base.services.hook.notify_on_failure
                },
                notify_on_long_running: if file.services.hook.notify_on_long_running
                    != d.services.hook.notify_on_long_running
                {
                    file.services.hook.notify_on_long_running
                } else {
                    base.services.hook.notify_on_long_running
                },
                long_running_threshold_ms: pick_u64(
                    base.services.hook.long_running_threshold_ms,
                    file.services.hook.long_running_threshold_ms,
                    d.services.hook.long_running_threshold_ms,
                ),
            },
            monitor: MonitorConfig {
                port: pick_u16(
                    base.services.monitor.port,
                    file.services.monitor.port,
                    d.services.monitor.port,
                ),
                collection_interval_secs: pick_u64(
                    base.services.monitor.collection_interval_secs,
                    file.services.monitor.collection_interval_secs,
                    d.services.monitor.collection_interval_secs,
                ),
                cpu_alert_threshold: pick_f64(
                    base.services.monitor.cpu_alert_threshold,
                    file.services.monitor.cpu_alert_threshold,
                    d.services.monitor.cpu_alert_threshold,
                ),
                memory_alert_threshold: pick_f64(
                    base.services.monitor.memory_alert_threshold,
                    file.services.monitor.memory_alert_threshold,
                    d.services.monitor.memory_alert_threshold,
                ),
                disk_alert_threshold: pick_f64(
                    base.services.monitor.disk_alert_threshold,
                    file.services.monitor.disk_alert_threshold,
                    d.services.monitor.disk_alert_threshold,
                ),
                history_size: pick_usize(
                    base.services.monitor.history_size,
                    file.services.monitor.history_size,
                    d.services.monitor.history_size,
                ),
            },
            communicate: CommunicateConfig {
                port: pick_u16(
                    base.services.communicate.port,
                    file.services.communicate.port,
                    d.services.communicate.port,
                ),
            },
            core: CoreConfig {
                port: pick_u16(
                    base.services.core.port,
                    file.services.core.port,
                    d.services.core.port,
                ),
                orchestrator_url: pick(
                    &base.services.core.orchestrator_url,
                    &file.services.core.orchestrator_url,
                    &d.services.core.orchestrator_url,
                ),
                notify_url: pick(
                    &base.services.core.notify_url,
                    &file.services.core.notify_url,
                    &d.services.core.notify_url,
                ),
                ask_url: pick(
                    &base.services.core.ask_url,
                    &file.services.core.ask_url,
                    &d.services.core.ask_url,
                ),
                wrap_url: pick(
                    &base.services.core.wrap_url,
                    &file.services.core.wrap_url,
                    &d.services.core.wrap_url,
                ),
                hook_url: pick(
                    &base.services.core.hook_url,
                    &file.services.core.hook_url,
                    &d.services.core.hook_url,
                ),
                monitor_url: pick(
                    &base.services.core.monitor_url,
                    &file.services.core.monitor_url,
                    &d.services.core.monitor_url,
                ),
                memory_url: pick(
                    &base.services.core.memory_url,
                    &file.services.core.memory_url,
                    &d.services.core.memory_url,
                ),
                communicate_url: pick(
                    &base.services.core.communicate_url,
                    &file.services.core.communicate_url,
                    &d.services.core.communicate_url,
                ),
                knowledge_url: pick(
                    &base.services.core.knowledge_url,
                    &file.services.core.knowledge_url,
                    &d.services.core.knowledge_url,
                ),
                index_url: pick(
                    &base.services.core.index_url,
                    &file.services.core.index_url,
                    &d.services.core.index_url,
                ),
            },
            mcp: McpConfig {
                orchestrator_url: pick(
                    &base.services.mcp.orchestrator_url,
                    &file.services.mcp.orchestrator_url,
                    &d.services.mcp.orchestrator_url,
                ),
                notify_url: pick(
                    &base.services.mcp.notify_url,
                    &file.services.mcp.notify_url,
                    &d.services.mcp.notify_url,
                ),
                ask_url: pick(
                    &base.services.mcp.ask_url,
                    &file.services.mcp.ask_url,
                    &d.services.mcp.ask_url,
                ),
                memory_url: pick(
                    &base.services.mcp.memory_url,
                    &file.services.mcp.memory_url,
                    &d.services.mcp.memory_url,
                ),
                communicate_url: pick(
                    &base.services.mcp.communicate_url,
                    &file.services.mcp.communicate_url,
                    &d.services.mcp.communicate_url,
                ),
                wrap_url: pick(
                    &base.services.mcp.wrap_url,
                    &file.services.mcp.wrap_url,
                    &d.services.mcp.wrap_url,
                ),
                monitor_url: pick(
                    &base.services.mcp.monitor_url,
                    &file.services.mcp.monitor_url,
                    &d.services.mcp.monitor_url,
                ),
                hook_url: pick(
                    &base.services.mcp.hook_url,
                    &file.services.mcp.hook_url,
                    &d.services.mcp.hook_url,
                ),
                knowledge_url: pick(
                    &base.services.mcp.knowledge_url,
                    &file.services.mcp.knowledge_url,
                    &d.services.mcp.knowledge_url,
                ),
            },
            ui: UiConfig {
                port: pick_u16(base.services.ui.port, file.services.ui.port, d.services.ui.port),
                ui_dir: pick(
                    &base.services.ui.ui_dir,
                    &file.services.ui.ui_dir,
                    &d.services.ui.ui_dir,
                ),
                public_urls: if file.services.ui.public_urls.is_empty() {
                    base.services.ui.public_urls.clone()
                } else {
                    file.services.ui.public_urls.clone()
                },
            },
        },
        apps: AppsConfig {
            cli: CliConfig {
                core_url: pick(
                    &base.apps.cli.core_url,
                    &file.apps.cli.core_url,
                    &d.apps.cli.core_url,
                ),
            },
        },
    }
}

/// Apply environment variable overrides onto a mutable [`AgentdConfig`].
fn apply_env_overrides(cfg: &mut AgentdConfig) {
    // ── General ───────────────────────────────────────────────────────────
    if let Ok(v) = env::var("AGENTD_LOG_LEVEL") {
        cfg.general.log_level = v;
    }
    if let Ok(v) = env::var("AGENTD_LOG_FORMAT") {
        cfg.general.log_format = v;
    }
    if let Ok(v) = env::var("AGENTD_HOST") {
        cfg.general.host = v;
    }

    // ── Ask ───────────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_ASK_PORT") {
        cfg.services.ask.port = p;
    }
    if let Ok(v) = env::var("AGENTD_ASK_ORCHESTRATOR_URL") {
        cfg.services.ask.orchestrator_url = v;
    }

    // ── Notify ────────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_NOTIFY_PORT") {
        cfg.services.notify.port = p;
    }

    // ── Orchestrator ──────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_ORCHESTRATOR_PORT") {
        cfg.services.orchestrator.port = p;
    }
    if let Ok(v) = env::var("AGENTD_ORCHESTRATOR_BACKEND") {
        cfg.services.orchestrator.backend = v;
    }
    if let Ok(v) = env::var("AGENTD_ORCHESTRATOR_COMMUNICATE_URL") {
        cfg.services.orchestrator.communicate_url = v;
    }
    if let Some(s) = parse_u64("AGENTD_RECONCILE_INTERVAL_SECS") {
        cfg.services.orchestrator.reconcile_interval_secs = s;
    }
    if let Ok(v) = env::var("AGENTD_ORCHESTRATOR_SUBPROCESS_PATH") {
        cfg.services.orchestrator.subprocess_path = v;
    }

    // ── Wrap ──────────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_WRAP_PORT") {
        cfg.services.wrap.port = p;
    }
    if let Ok(v) = env::var("AGENTD_WRAP_BACKEND") {
        cfg.services.wrap.backend = v;
    }

    // ── Memory ────────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_MEMORY_PORT") {
        cfg.services.memory.port = p;
    }
    if let Ok(v) = env::var("AGENTD_MEMORY_EMBEDDING_PROVIDER") {
        cfg.services.memory.embedding_provider = v;
    }
    if let Ok(v) = env::var("AGENTD_MEMORY_EMBEDDING_MODEL") {
        cfg.services.memory.embedding_model = v;
    }
    if let Ok(v) = env::var("AGENTD_MEMORY_LANCE_PATH") {
        cfg.services.memory.lance_path = v;
    }

    // ── Knowledge ───────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_KNOWLEDGE_PORT") {
        cfg.services.knowledge.port = p;
    }
    if let Ok(v) = env::var("AGENTD_KNOWLEDGE_ROOT") {
        cfg.services.knowledge.root = v;
    }

    // ── Hook ──────────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_HOOK_PORT") {
        cfg.services.hook.port = p;
    }
    if let Some(s) = parse_usize("AGENTD_HISTORY_SIZE") {
        cfg.services.hook.history_size = s;
    }
    if let Ok(v) = env::var("AGENTD_NOTIFY_SERVICE_URL") {
        if !v.is_empty() {
            cfg.services.hook.notify_service_url = Some(v);
        }
    }
    if let Ok(v) = env::var("AGENTD_NOTIFY_ON_FAILURE") {
        cfg.services.hook.notify_on_failure = v != "false" && v != "0";
    }
    if let Ok(v) = env::var("AGENTD_NOTIFY_ON_LONG_RUNNING") {
        cfg.services.hook.notify_on_long_running = v != "false" && v != "0";
    }
    if let Some(s) = parse_u64("AGENTD_LONG_RUNNING_THRESHOLD_MS") {
        cfg.services.hook.long_running_threshold_ms = s;
    }

    // ── Monitor ───────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_MONITOR_PORT") {
        cfg.services.monitor.port = p;
    }
    if let Some(s) = parse_u64("AGENTD_COLLECTION_INTERVAL_SECS") {
        cfg.services.monitor.collection_interval_secs = s;
    }
    if let Some(v) = parse_f64("AGENTD_CPU_ALERT_THRESHOLD") {
        cfg.services.monitor.cpu_alert_threshold = v;
    }
    if let Some(v) = parse_f64("AGENTD_MEMORY_ALERT_THRESHOLD") {
        cfg.services.monitor.memory_alert_threshold = v;
    }
    if let Some(v) = parse_f64("AGENTD_DISK_ALERT_THRESHOLD") {
        cfg.services.monitor.disk_alert_threshold = v;
    }
    if let Some(s) = parse_usize("AGENTD_MONITOR_HISTORY_SIZE") {
        cfg.services.monitor.history_size = s;
    }

    // ── Communicate ───────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_COMMUNICATE_PORT") {
        cfg.services.communicate.port = p;
    }

    // ── Core ──────────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_CORE_PORT") {
        cfg.services.core.port = p;
    }

    // ── MCP ───────────────────────────────────────────────────────────────
    if let Ok(v) = env::var("AGENTD_MCP_ORCHESTRATOR_URL") {
        cfg.services.mcp.orchestrator_url = v;
    }
    if let Ok(v) = env::var("AGENTD_MCP_NOTIFY_URL") {
        cfg.services.mcp.notify_url = v;
    }
    if let Ok(v) = env::var("AGENTD_MCP_ASK_URL") {
        cfg.services.mcp.ask_url = v;
    }
    if let Ok(v) = env::var("AGENTD_MCP_MEMORY_URL") {
        cfg.services.mcp.memory_url = v;
    }
    if let Ok(v) = env::var("AGENTD_MCP_COMMUNICATE_URL") {
        cfg.services.mcp.communicate_url = v;
    }
    if let Ok(v) = env::var("AGENTD_MCP_WRAP_URL") {
        cfg.services.mcp.wrap_url = v;
    }
    if let Ok(v) = env::var("AGENTD_MCP_MONITOR_URL") {
        cfg.services.mcp.monitor_url = v;
    }
    if let Ok(v) = env::var("AGENTD_MCP_HOOK_URL") {
        cfg.services.mcp.hook_url = v;
    }

    // ── UI ────────────────────────────────────────────────────────────────
    if let Some(p) = parse_port("AGENTD_UI_PORT") {
        cfg.services.ui.port = p;
    }
    if let Ok(v) = env::var("AGENTD_UI_DIR") {
        cfg.services.ui.ui_dir = v;
    }

    // ── Apps: CLI ─────────────────────────────────────────────────────────
    if let Ok(v) = env::var("AGENTD_CORE_SERVICE_URL") {
        if !v.is_empty() {
            cfg.apps.cli.core_url = v;
        }
    }
}

// ---------------------------------------------------------------------------
// Merge helpers
// ---------------------------------------------------------------------------

/// Return `file` if it differs from `default`, otherwise return `base`.
#[inline]
fn pick(base: &str, file: &str, default: &str) -> String {
    if file != default {
        file.to_string()
    } else {
        base.to_string()
    }
}

#[inline]
fn pick_u16(base: u16, file: u16, default: u16) -> u16 {
    if file != default {
        file
    } else {
        base
    }
}

#[inline]
fn pick_u64(base: u64, file: u64, default: u64) -> u64 {
    if file != default {
        file
    } else {
        base
    }
}

#[inline]
fn pick_usize(base: usize, file: usize, default: usize) -> usize {
    if file != default {
        file
    } else {
        base
    }
}

#[inline]
fn pick_f64(base: f64, file: f64, default: f64) -> f64 {
    if (file - default).abs() > f64::EPSILON {
        file
    } else {
        base
    }
}

// ---------------------------------------------------------------------------
// Env-var parse helpers
// ---------------------------------------------------------------------------

fn parse_port(var: &str) -> Option<u16> {
    env::var(var).ok()?.parse::<u16>().ok()
}

fn parse_u64(var: &str) -> Option<u64> {
    env::var(var).ok()?.parse::<u64>().ok()
}

fn parse_usize(var: &str) -> Option<usize> {
    env::var(var).ok()?.parse::<usize>().ok()
}

fn parse_f64(var: &str) -> Option<f64> {
    env::var(var).ok()?.parse::<f64>().ok()
}

// ---------------------------------------------------------------------------
// Default path helpers
// ---------------------------------------------------------------------------

fn default_memory_lance_path() -> String {
    ProjectDirs::from("", "", "agentd-memory")
        .map(|d| d.data_dir().join("lancedb").to_string_lossy().to_string())
        .unwrap_or_else(|| "lancedb".to_string())
}

fn default_knowledge_docs_path() -> String {
    ProjectDirs::from("", "", "agentd-knowledge")
        .map(|d| d.data_dir().join("docs").to_string_lossy().to_string())
        .unwrap_or_else(|| "docs".to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    /// Serialises tests that touch environment variables so they don't race
    /// when the test harness runs them on multiple threads concurrently.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ── Default values ─────────────────────────────────────────────────────

    #[test]
    fn test_default_general() {
        let cfg = AgentdConfig::default();
        assert_eq!(cfg.general.log_level, "info");
        assert_eq!(cfg.general.log_format, "text");
        assert_eq!(cfg.general.host, "127.0.0.1");
    }

    #[test]
    fn test_default_service_ports() {
        let cfg = AgentdConfig::default();
        assert_eq!(cfg.services.ask.port, 17001);
        assert_eq!(cfg.services.notify.port, 17004);
        assert_eq!(cfg.services.orchestrator.port, 17006);
        assert_eq!(cfg.services.wrap.port, 17005);
        assert_eq!(cfg.services.memory.port, 17008);
        assert_eq!(cfg.services.knowledge.port, 17011);
        assert_eq!(cfg.services.hook.port, 17002);
        assert_eq!(cfg.services.monitor.port, 17003);
        assert_eq!(cfg.services.communicate.port, 17010);
        assert_eq!(cfg.services.core.port, 17000);
        assert_eq!(cfg.services.ui.port, 17009);
    }

    #[test]
    fn test_default_orchestrator_backend() {
        let cfg = AgentdConfig::default();
        assert_eq!(cfg.services.orchestrator.backend, "tmux");
    }

    #[test]
    fn test_default_memory_embedding_provider() {
        let cfg = AgentdConfig::default();
        assert_eq!(cfg.services.memory.embedding_provider, "none");
    }

    #[test]
    fn test_default_hook_history_size() {
        let cfg = AgentdConfig::default();
        assert_eq!(cfg.services.hook.history_size, 500);
        assert!(cfg.services.hook.notify_service_url.is_none());
    }

    #[test]
    fn test_default_mcp_urls() {
        let cfg = AgentdConfig::default();
        assert_eq!(cfg.services.mcp.orchestrator_url, "http://localhost:17006");
        assert_eq!(cfg.services.mcp.notify_url, "http://localhost:17004");
    }

    // ── TOML round-trip ────────────────────────────────────────────────────

    #[test]
    fn test_toml_roundtrip_defaults() {
        let original = AgentdConfig::default();
        let serialized = toml::to_string(&original).expect("serialization failed");
        let parsed: AgentdConfig = toml::from_str(&serialized).expect("parse failed");
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_toml_partial_parse() {
        let toml_str = r#"
[general]
log_level = "debug"
log_format = "json"

[services.notify]
port = 19004
"#;
        let cfg: AgentdConfig = toml::from_str(toml_str).expect("parse failed");
        assert_eq!(cfg.general.log_level, "debug");
        assert_eq!(cfg.general.log_format, "json");
        assert_eq!(cfg.services.notify.port, 19004);
        // Fields absent from TOML fall back to compiled defaults
        assert_eq!(cfg.services.ask.port, 17001);
        assert_eq!(cfg.general.host, "127.0.0.1");
    }

    // ── File-based load (uses load_from_path — no global env var needed) ───

    #[test]
    fn test_load_with_config_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"
[general]
log_level = "warn"

[services.hook]
history_size = 1000
"#
        )
        .unwrap();

        let cfg = load_from_path(Some(f.path())).expect("load failed");

        assert_eq!(cfg.general.log_level, "warn");
        assert_eq!(cfg.services.hook.history_size, 1000);
        // Unmentioned fields keep defaults
        assert_eq!(cfg.services.ask.port, 17001);
    }

    #[test]
    fn test_load_missing_file_uses_defaults() {
        // load_from_path reads env vars, so serialise against env-mutating tests.
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let cfg =
            load_from_path(Some(std::path::Path::new("/tmp/agentd-nonexistent-config-test.toml")))
                .expect("load should not fail for missing file");
        // Env vars in the test environment may differ from defaults, so just
        // check the file-layer fields (ports are not overridden by common vars).
        assert_eq!(cfg.services.ask.port, 17001);
        assert_eq!(cfg.services.notify.port, 17004);
    }

    #[test]
    fn test_load_none_path_uses_defaults() {
        // load_from_path reads env vars, so serialise against env-mutating tests.
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let cfg = load_from_path(None).expect("load failed");
        assert_eq!(cfg.services.ask.port, 17001);
    }

    #[test]
    fn test_load_malformed_file_returns_error() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "this is not valid toml {{ }}").unwrap();
        let result = load_from_path(Some(f.path()));
        assert!(result.is_err());
    }

    // ── Env var overlay (serialised via ENV_LOCK) ─────────────────────────

    #[test]
    fn test_env_override_log_level() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_LOG_LEVEL", "trace");
        let cfg = load_from_path(None).expect("load failed");
        env::remove_var("AGENTD_LOG_LEVEL");
        assert_eq!(cfg.general.log_level, "trace");
    }

    #[test]
    fn test_env_override_notify_port() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_NOTIFY_PORT", "19004");
        let cfg = load_from_path(None).expect("load failed");
        env::remove_var("AGENTD_NOTIFY_PORT");
        assert_eq!(cfg.services.notify.port, 19004);
    }

    #[test]
    fn test_env_override_hook_notify_url() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_NOTIFY_SERVICE_URL", "http://notify:9004");
        let cfg = load_from_path(None).expect("load failed");
        env::remove_var("AGENTD_NOTIFY_SERVICE_URL");
        assert_eq!(cfg.services.hook.notify_service_url, Some("http://notify:9004".to_string()),);
    }

    // ── Merge precedence ──────────────────────────────────────────────────

    #[test]
    fn test_precedence_env_beats_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "[general]\nlog_level = \"warn\"").unwrap();

        env::set_var("AGENTD_LOG_LEVEL", "error");
        let cfg = load_from_path(Some(f.path())).expect("load failed");
        env::remove_var("AGENTD_LOG_LEVEL");

        assert_eq!(cfg.general.log_level, "error");
    }

    #[test]
    fn test_precedence_file_beats_default() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "[services.core]\nport = 19000").unwrap();

        let cfg = load_from_path(Some(f.path())).expect("load failed");

        assert_eq!(cfg.services.core.port, 19000);
        // Other ports untouched
        assert_eq!(cfg.services.ask.port, 17001);
    }

    // ── core upstream URLs ─────────────────────────────────────────────────

    #[test]
    fn test_default_core_upstream_urls() {
        let cfg = AgentdConfig::default();
        assert_eq!(cfg.services.core.orchestrator_url, "http://localhost:17006");
        assert_eq!(cfg.services.core.memory_url, "http://localhost:17008");
        assert_eq!(cfg.services.core.knowledge_url, "http://localhost:17011");
    }

    #[test]
    fn test_core_upstream_urls_from_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::remove_var("AGENTD_CORE_SERVICE_URL");
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            f,
            "[services.core]\nport = 7000\norchestrator_url = \"http://localhost:7006\"\nmemory_url = \"http://localhost:7008\""
        )
        .unwrap();

        let cfg = load_from_path(Some(f.path())).expect("load failed");

        assert_eq!(cfg.services.core.port, 7000);
        assert_eq!(cfg.services.core.orchestrator_url, "http://localhost:7006");
        assert_eq!(cfg.services.core.memory_url, "http://localhost:7008");
        // Unspecified upstreams stay at their defaults.
        assert_eq!(cfg.services.core.notify_url, "http://localhost:17004");
    }

    // ── apps.cli.core_url ──────────────────────────────────────────────────

    #[test]
    fn test_default_cli_core_url() {
        let cfg = AgentdConfig::default();
        assert_eq!(cfg.apps.cli.core_url, "http://localhost:17000");
    }

    #[test]
    fn test_cli_core_url_file_beats_default() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::remove_var("AGENTD_CORE_SERVICE_URL");
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "[apps.cli]\ncore_url = \"https://agentd.example.com\"").unwrap();

        let cfg = load_from_path(Some(f.path())).expect("load failed");

        assert_eq!(cfg.apps.cli.core_url, "https://agentd.example.com");
    }

    #[test]
    fn test_cli_core_url_env_beats_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "[apps.cli]\ncore_url = \"https://from-file.example.com\"").unwrap();
        env::set_var("AGENTD_CORE_SERVICE_URL", "https://from-env.example.com");

        let cfg = load_from_path(Some(f.path())).expect("load failed");
        env::remove_var("AGENTD_CORE_SERVICE_URL");

        assert_eq!(cfg.apps.cli.core_url, "https://from-env.example.com");
    }

    // ── Config file path discovery (uses ENV_LOCK for env var access) ──────

    #[test]
    fn test_config_file_path_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_CONFIG", "/custom/path/config.toml");
        let path = config_file_path().unwrap();
        env::remove_var("AGENTD_CONFIG");
        assert_eq!(path, PathBuf::from("/custom/path/config.toml"));
    }

    #[test]
    fn test_config_file_path_empty_env_falls_through() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_CONFIG", "");
        let path = config_file_path();
        env::remove_var("AGENTD_CONFIG");
        // Should fall through to XDG path — just ensure it's non-empty if present
        if let Some(p) = path {
            assert!(!p.to_string_lossy().is_empty());
        }
    }

    // ── Clone and PartialEq ────────────────────────────────────────────────

    #[test]
    fn test_clone() {
        let cfg = AgentdConfig::default();
        let cloned = cfg.clone();
        assert_eq!(cfg, cloned);
    }

    // ── New field tests ────────────────────────────────────────────────────

    #[test]
    fn test_env_override_reconcile_interval() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_RECONCILE_INTERVAL_SECS", "60");
        let cfg = load_from_path(None).expect("load failed");
        env::remove_var("AGENTD_RECONCILE_INTERVAL_SECS");
        assert_eq!(cfg.services.orchestrator.reconcile_interval_secs, 60);
    }

    #[test]
    fn test_env_override_collection_interval() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_COLLECTION_INTERVAL_SECS", "60");
        let cfg = load_from_path(None).expect("load failed");
        env::remove_var("AGENTD_COLLECTION_INTERVAL_SECS");
        assert_eq!(cfg.services.monitor.collection_interval_secs, 60);
    }

    #[test]
    fn test_env_override_monitor_history_size() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_MONITOR_HISTORY_SIZE", "200");
        let cfg = load_from_path(None).expect("load failed");
        env::remove_var("AGENTD_MONITOR_HISTORY_SIZE");
        assert_eq!(cfg.services.monitor.history_size, 200);
    }

    #[test]
    fn test_default_monitor_fields() {
        let cfg = AgentdConfig::default();
        assert!((cfg.services.monitor.cpu_alert_threshold - 90.0).abs() < f64::EPSILON);
        assert!((cfg.services.monitor.memory_alert_threshold - 90.0).abs() < f64::EPSILON);
        assert!((cfg.services.monitor.disk_alert_threshold - 90.0).abs() < f64::EPSILON);
        assert_eq!(cfg.services.monitor.history_size, 120);
    }

    #[test]
    fn test_default_hook_fields() {
        let cfg = AgentdConfig::default();
        assert!(cfg.services.hook.notify_on_failure);
        assert!(cfg.services.hook.notify_on_long_running);
        assert_eq!(cfg.services.hook.long_running_threshold_ms, 30_000);
    }

    // ── Edge cases: empty / unknown keys / concurrent load ─────────────────

    #[test]
    fn test_load_empty_file_uses_defaults() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let f = tempfile::NamedTempFile::new().unwrap();
        // File exists but contains no content
        let cfg = load_from_path(Some(f.path())).expect("load failed on empty file");
        assert_eq!(cfg.services.ask.port, 17001);
        assert_eq!(cfg.general.log_level, "info");
    }

    #[test]
    fn test_load_unknown_keys_are_ignored() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // TOML with unknown top-level and nested keys must not cause an error.
        // serde's default behaviour is to ignore unknown fields for structs
        // annotated with `#[serde(default)]`.
        let toml_str = r#"
[general]
log_level = "debug"
totally_unknown_field = "should be ignored"

[services.ask]
port = 19001
another_unknown = 42

[unknown_section]
foo = "bar"
"#;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(f, "{}", toml_str).unwrap();

        let cfg = load_from_path(Some(f.path())).expect("unknown keys should not error");
        assert_eq!(cfg.general.log_level, "debug");
        assert_eq!(cfg.services.ask.port, 19001);
        // Unmentioned fields keep defaults
        assert_eq!(cfg.services.notify.port, 17004);
    }

    #[test]
    fn test_full_precedence_chain() {
        // Verify all three layers: default < file < env var
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let mut f = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        // File sets notify port to a non-default value
        writeln!(f, "[services.notify]\nport = 19004").unwrap();
        writeln!(f, "[services.ask]\nport = 19001").unwrap();

        // Env var overrides ask port (beats file), notify port is file-only
        env::set_var("AGENTD_ASK_PORT", "29001");
        let cfg = load_from_path(Some(f.path())).expect("load failed");
        // Remove env var BEFORE asserting so a panic can't leave it set
        env::remove_var("AGENTD_ASK_PORT");

        // File beats default: notify port is 19004 (not 17004)
        assert_eq!(cfg.services.notify.port, 19004);
        // Env beats file: ask port is 29001 (not 19001 from file)
        assert_eq!(cfg.services.ask.port, 29001);
        // Untouched service uses compiled default: wrap port is 17005
        assert_eq!(cfg.services.wrap.port, 17005);
    }

    #[test]
    fn test_env_absent_preserves_file_value() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // When the env var is not set the file value should survive unchanged.
        let mut f = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        writeln!(f, "[general]\nlog_level = \"warn\"").unwrap();

        let cfg = load_from_path(Some(f.path())).expect("load failed");
        assert_eq!(cfg.general.log_level, "warn");
    }

    #[test]
    fn test_partial_toml_leaves_other_services_at_defaults() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let toml_str = r#"
[services.monitor]
port = 13003
"#;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(f, "{}", toml_str).unwrap();

        let cfg = load_from_path(Some(f.path())).expect("load failed");
        // Changed service
        assert_eq!(cfg.services.monitor.port, 13003);
        // Everything else unchanged
        assert_eq!(cfg.services.ask.port, 17001);
        assert_eq!(cfg.services.notify.port, 17004);
        assert_eq!(cfg.services.orchestrator.port, 17006);
        assert_eq!(cfg.services.wrap.port, 17005);
        assert_eq!(cfg.services.memory.port, 17008);
        assert_eq!(cfg.services.hook.port, 17002);
        assert_eq!(cfg.services.communicate.port, 17010);
        assert_eq!(cfg.services.core.port, 17000);
        assert_eq!(cfg.services.ui.port, 17009);
    }

    #[test]
    fn test_all_default_ports_match_spec() {
        let cfg = AgentdConfig::default();
        let ports = [
            ("core", cfg.services.core.port, 17000u16),
            ("ask", cfg.services.ask.port, 17001),
            ("hook", cfg.services.hook.port, 17002),
            ("monitor", cfg.services.monitor.port, 17003),
            ("notify", cfg.services.notify.port, 17004),
            ("wrap", cfg.services.wrap.port, 17005),
            ("orchestrator", cfg.services.orchestrator.port, 17006),
            ("memory", cfg.services.memory.port, 17008),
            ("ui", cfg.services.ui.port, 17009),
            ("communicate", cfg.services.communicate.port, 17010),
            ("knowledge", cfg.services.knowledge.port, 17011),
        ];
        for (name, actual, expected) in ports {
            assert_eq!(actual, expected, "{} port mismatch", name);
        }
    }

    #[test]
    fn test_concurrent_load_returns_consistent_results() {
        // Spin up multiple threads all calling load_from_path(None) concurrently
        // and verify each gets the same default result.
        use std::thread;

        let mut handles = vec![];
        for _ in 0..8 {
            handles.push(thread::spawn(|| load_from_path(None).expect("concurrent load failed")));
        }
        let results: Vec<AgentdConfig> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let first = &results[0];
        for r in &results[1..] {
            assert_eq!(r.services.ask.port, first.services.ask.port);
            assert_eq!(r.general.log_level, first.general.log_level);
        }
    }

    #[test]
    fn test_hook_notify_url_from_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The ambient environment (e.g. direnv) may set this, and env beats
        // file — remove it so we actually exercise the file layer.
        env::remove_var("AGENTD_NOTIFY_SERVICE_URL");
        let mut f = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        writeln!(f, "[services.hook]\nnotify_service_url = \"http://notify:9004\"").unwrap();
        let cfg = load_from_path(Some(f.path())).expect("load failed");
        assert_eq!(cfg.services.hook.notify_service_url, Some("http://notify:9004".to_string()));
    }

    #[test]
    fn test_mcp_urls_from_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let toml_str = r#"
[services.mcp]
orchestrator_url = "http://orch:7006"
notify_url = "http://ntf:7004"
"#;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(f, "{}", toml_str).unwrap();
        let cfg = load_from_path(Some(f.path())).expect("load failed");
        assert_eq!(cfg.services.mcp.orchestrator_url, "http://orch:7006");
        assert_eq!(cfg.services.mcp.notify_url, "http://ntf:7004");
        // Unset MCP URLs keep defaults
        assert_eq!(cfg.services.mcp.ask_url, "http://localhost:17001");
    }
}
