//! Config management command implementations.
//!
//! This module implements the `agent config` subcommands for generating and
//! inspecting the agentd TOML configuration file.
//!
//! # Available Commands
//!
//! - **init**: Generate a commented default `config.toml` at the XDG config path
//! - **show**: Display the fully resolved configuration (defaults + file + env vars)
//!
//! # Examples
//!
//! ```bash
//! # Generate a default config file
//! agent config init
//!
//! # Force-overwrite an existing config file
//! agent config init --force
//!
//! # Show the resolved config in TOML format
//! agent config show
//!
//! # Show the resolved config as JSON
//! agent config show --json
//!
//! # Show only the raw config file contents (no env var overlay)
//! agent config show --raw
//! ```

use agentd_common::config::config_file_path;
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::fs;

/// Config management subcommands.
#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Generate a default config.toml at the XDG config path.
    ///
    /// Creates `~/.config/agentd/config.toml` with all sections present,
    /// documented comments for every setting, and compiled-default values.
    /// Refuses to overwrite an existing file unless `--force` is given.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent config init
    /// agent config init --force
    /// ```
    Init {
        /// Overwrite an existing config file without prompting.
        #[arg(long)]
        force: bool,
    },

    /// Display the fully resolved configuration.
    ///
    /// Merges compiled defaults, the config file (if present), and any
    /// `AGENTD_*` environment variables, then prints the result.
    ///
    /// Use `--raw` to print only the config file on disk without applying
    /// environment variable overrides.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent config show
    /// agent config show --json
    /// agent config show --raw
    /// ```
    Show {
        /// Print only the on-disk config file without env var overlay.
        #[arg(long)]
        raw: bool,
    },
}

impl ConfigCommand {
    pub fn execute(&self, json: bool) -> Result<()> {
        match self {
            ConfigCommand::Init { force } => cmd_init(*force),
            ConfigCommand::Show { raw } => cmd_show(*raw, json),
        }
    }
}

// ---------------------------------------------------------------------------
// config init
// ---------------------------------------------------------------------------

pub(crate) fn cmd_init(force: bool) -> Result<()> {
    let path = config_file_path().context(
        "could not determine config file path — \
         set AGENTD_CONFIG or ensure a home directory is available",
    )?;

    if path.exists() && !force {
        bail!(
            "config file already exists at {}\n\
             Run `agent config init --force` to overwrite it.",
            path.display()
        );
    }

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }

    fs::write(&path, TEMPLATE_CONFIG)
        .with_context(|| format!("writing config file {}", path.display()))?;

    println!("Created {}", path.display().to_string().green().bold());
    Ok(())
}

// ---------------------------------------------------------------------------
// config show
// ---------------------------------------------------------------------------

fn cmd_show(raw: bool, json: bool) -> Result<()> {
    if raw {
        // Print only the file on disk, no merging
        let path = config_file_path();
        match path.as_ref().filter(|p| p.exists()) {
            Some(p) => {
                let contents = fs::read_to_string(p)
                    .with_context(|| format!("reading config file {}", p.display()))?;
                print!("{contents}");
            }
            None => {
                eprintln!("{}", "No config file found.".yellow());
                eprintln!(
                    "Run `agent config init` to create one at {}",
                    path.as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "~/.config/agentd/config.toml".to_string())
                );
            }
        }
        return Ok(());
    }

    // Full resolved config
    let cfg = agentd_common::config::load().context("failed to load configuration")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&cfg).context("serialising config to JSON")?);
    } else {
        let toml_str = toml::to_string_pretty(&cfg).context("serialising config to TOML")?;
        print!("{toml_str}");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Commented default config template
// ---------------------------------------------------------------------------

const TEMPLATE_CONFIG: &str = r#"# agentd configuration file
# Generated by `agent config init`
#
# Precedence (highest wins): environment variables > this file > compiled defaults
# All settings are optional — omit a key to keep the compiled default.

# ---------------------------------------------------------------------------
# [general] — cross-cutting settings that apply to all services
# ---------------------------------------------------------------------------

[general]

# Minimum log level filter: trace, debug, info, warn, error
# Environment variable: AGENTD_LOG_LEVEL
log_level = "info"

# Log output format: "text" (human-readable) or "json" (structured)
# Environment variable: AGENTD_LOG_FORMAT
log_format = "text"

# Default bind address for all services
# Environment variable: AGENTD_HOST
host = "127.0.0.1"

# ---------------------------------------------------------------------------
# [services.ask] — question-and-answer service (port 17001)
# ---------------------------------------------------------------------------

[services.ask]

# HTTP listen port
# Environment variable: AGENTD_ASK_PORT
port = 17001

# Orchestrator callback URL
# Environment variable: AGENTD_ASK_ORCHESTRATOR_URL
orchestrator_url = "http://localhost:17006"

# ---------------------------------------------------------------------------
# [services.notify] — notification service (port 17004)
# ---------------------------------------------------------------------------

[services.notify]

# HTTP listen port
# Environment variable: AGENTD_NOTIFY_PORT
port = 17004

# ---------------------------------------------------------------------------
# [services.orchestrator] — agent orchestration service (port 17006)
# ---------------------------------------------------------------------------

[services.orchestrator]

# HTTP listen port
# Environment variable: AGENTD_ORCHESTRATOR_PORT
port = 17006

# Execution backend: tmux, docker, pty, subprocess
# Environment variable: AGENTD_ORCHESTRATOR_BACKEND
backend = "tmux"

# Communicate service URL for agent message delivery
# Environment variable: AGENTD_ORCHESTRATOR_COMMUNICATE_URL
communicate_url = "http://localhost:17010"

# Agent reconciliation interval in seconds
# Environment variable: AGENTD_RECONCILE_INTERVAL_SECS
reconcile_interval_secs = 30

# ---------------------------------------------------------------------------
# [services.wrap] — agent session wrapper service (port 17005)
# ---------------------------------------------------------------------------

[services.wrap]

# HTTP listen port
# Environment variable: AGENTD_WRAP_PORT
port = 17005

# Execution backend: tmux, docker, pty, subprocess
# Environment variable: AGENTD_WRAP_BACKEND
backend = "tmux"

# ---------------------------------------------------------------------------
# [services.memory] — vector memory service (port 17008)
# ---------------------------------------------------------------------------

[services.memory]

# HTTP listen port
# Environment variable: AGENTD_MEMORY_PORT
port = 17008

# Embedding provider: none, ollama, openai
# Environment variable: AGENTD_MEMORY_EMBEDDING_PROVIDER
embedding_provider = "none"

# Embedding model name
# Environment variable: AGENTD_MEMORY_EMBEDDING_MODEL
embedding_model = "text-embedding-3-small"

# LanceDB storage path (~ is expanded by the OS, not agentd)
# Environment variable: AGENTD_MEMORY_LANCE_PATH
# lance_path = "~/.local/share/agentd-memory/lancedb"

# ---------------------------------------------------------------------------
# [services.hook] — shell-hook daemon (port 17002)
# ---------------------------------------------------------------------------

[services.hook]

# HTTP listen port
# Environment variable: AGENTD_HOOK_PORT
port = 17002

# Maximum shell-event history retained in memory
# Environment variable: AGENTD_HISTORY_SIZE
history_size = 500

# Optional notify service URL — forward notable hook events as notifications
# Environment variable: AGENTD_NOTIFY_SERVICE_URL
# notify_service_url = "http://localhost:17004"

# ---------------------------------------------------------------------------
# [services.monitor] — system-metrics daemon (port 17003)
# ---------------------------------------------------------------------------

[services.monitor]

# HTTP listen port
# Environment variable: AGENTD_MONITOR_PORT
port = 17003

# Metrics collection interval in seconds
# Environment variable: AGENTD_COLLECTION_INTERVAL_SECS
collection_interval_secs = 15

# ---------------------------------------------------------------------------
# [services.communicate] — inter-agent messaging service (port 17010)
# ---------------------------------------------------------------------------

[services.communicate]

# HTTP listen port
# Environment variable: AGENTD_COMMUNICATE_PORT
port = 17010

# ---------------------------------------------------------------------------
# [services.core] — agentd core API service / gateway (port 17000)
# ---------------------------------------------------------------------------

[services.core]

# HTTP listen port
# Environment variable: AGENTD_CORE_PORT
port = 17000

# Upstream URLs the gateway reverse-proxies to. Set these when the upstream
# services do not listen on their default 127.0.0.1:17xxx addresses (e.g. when
# you run the whole stack on a different port range). A matching bare env var
# (ORCHESTRATOR_URL, NOTIFY_URL, ...) still overrides the value here when set.
orchestrator_url = "http://localhost:17006"
notify_url       = "http://localhost:17004"
ask_url          = "http://localhost:17001"
wrap_url         = "http://localhost:17005"
hook_url         = "http://localhost:17002"
monitor_url      = "http://localhost:17003"
memory_url       = "http://localhost:17008"
communicate_url  = "http://localhost:17010"
knowledge_url    = "http://localhost:17011"

# ---------------------------------------------------------------------------
# [services.mcp] — MCP server (no dedicated port — uses stdio transport)
# ---------------------------------------------------------------------------

[services.mcp]

# Service URLs used by the MCP tools
# Environment variables: AGENTD_MCP_*_URL
orchestrator_url = "http://127.0.0.1:17006"
notify_url       = "http://127.0.0.1:17004"
ask_url          = "http://127.0.0.1:17001"
memory_url       = "http://127.0.0.1:17008"
communicate_url  = "http://127.0.0.1:17010"
wrap_url         = "http://127.0.0.1:17005"
monitor_url      = "http://127.0.0.1:17003"
hook_url         = "http://127.0.0.1:17002"

# ---------------------------------------------------------------------------
# [services.ui] — web UI server (port 17009)
# ---------------------------------------------------------------------------

[services.ui]

# HTTP listen port
# Environment variable: AGENTD_UI_PORT
port = 17009

# Directory containing compiled frontend assets
# Environment variable: AGENTD_UI_DIR
ui_dir = "./ui/dist"

# ---------------------------------------------------------------------------
# [apps.cli] — the `agent` command-line interface (this tool)
# ---------------------------------------------------------------------------

[apps.cli]

# Base URL of the core auth gateway that fronts all services. Every `agent`
# command is routed through it as <core_url>/api/v1/<service>, so set this once
# to point the CLI at a remote deployment instead of exporting the environment
# variable in every shell.
# Environment variable (takes precedence): AGENTD_CORE_SERVICE_URL
core_url = "http://localhost:17000"
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialise tests that set `AGENTD_CONFIG` so env-var mutations don't bleed
    /// across concurrent test threads.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn template_is_valid_toml() {
        let result: Result<toml::Value, _> = toml::from_str(TEMPLATE_CONFIG);
        assert!(result.is_ok(), "TEMPLATE_CONFIG is not valid TOML: {:?}", result.err());
    }

    #[test]
    fn template_deserialises_to_agentd_config() {
        let cfg: agentd_common::config::AgentdConfig =
            toml::from_str(TEMPLATE_CONFIG).expect("template should deserialise to AgentdConfig");
        assert_eq!(cfg.services.ask.port, 17001);
        assert_eq!(cfg.services.notify.port, 17004);
        assert_eq!(cfg.services.hook.history_size, 500);
        assert_eq!(cfg.general.log_level, "info");
        assert_eq!(cfg.apps.cli.core_url, "http://localhost:17000");
    }

    #[test]
    fn init_refuses_overwrite_without_force() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# existing").unwrap();
        std::env::set_var("AGENTD_CONFIG", path.to_str().unwrap());

        let result = cmd_init(false);
        std::env::remove_var("AGENTD_CONFIG");

        assert!(result.is_err(), "cmd_init should fail when file exists and force=false");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("--force"), "error should mention --force, got: {msg}");
    }

    #[test]
    fn init_allows_overwrite_with_force() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# existing").unwrap();
        std::env::set_var("AGENTD_CONFIG", path.to_str().unwrap());

        let result = cmd_init(true);
        std::env::remove_var("AGENTD_CONFIG");

        assert!(result.is_ok(), "cmd_init should succeed when force=true, got: {:?}", result.err());
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("[general]"), "template should have been written, got: {written}");
    }
}
