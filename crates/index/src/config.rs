//! Configuration types for the agentd-index service.
//!
//! Provides [`IndexConfig`] and its sub-configs which describe all service
//! configuration and can be loaded from environment variables via
//! [`IndexConfig::from_env`].
//!
//! # Environment Variables
//!
//! | Variable                              | Default                              | Description                          |
//! |---------------------------------------|--------------------------------------|--------------------------------------|
//! | `AGENTD_PORT`                         | `17012`                              | HTTP listen port                     |
//! | `AGENTD_INDEX_EMBEDDING_PROVIDER`     | `ollama`                             | Embedding provider                   |
//! | `AGENTD_INDEX_EMBEDDING_MODEL`        | `nomic-embed-text`                   | Embedding model name                 |
//! | `AGENTD_INDEX_EMBEDDING_ENDPOINT`     | `http://localhost:11434/v1`          | Ollama API endpoint                  |
//! | `AGENTD_INDEX_LANCE_PATH`             | XDG data dir / `lancedb`            | LanceDB directory path               |
//! | `AGENTD_INDEX_LANCE_TABLE`            | `code_chunks`                        | LanceDB table name                   |
//! | `AGENTD_INDEX_WATCH_INTERVAL`         | `30`                                 | File watch poll interval (seconds)   |
//! | `AGENTD_INDEX_LANGUAGES`              | `rust,python,javascript,typescript`  | Comma-separated supported languages  |
//! | `AGENTD_INDEX_IGNORE_PATTERNS`        | `.git,target,node_modules,dist`      | Comma-separated glob patterns        |

use agentd_common::config::IndexConfig as SharedIndexConfig;
use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// EmbeddingConfig
// ---------------------------------------------------------------------------

/// Configuration for the embedding provider used by the index service.
///
/// Defaults to a local Ollama instance with the `nomic-embed-text` model.
///
/// # Example
///
/// ```rust
/// use index::config::EmbeddingConfig;
///
/// let config = EmbeddingConfig::default();
/// assert_eq!(config.provider, "ollama");
/// assert_eq!(config.model, "nomic-embed-text");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// Embedding provider: `"ollama"` (default) or `"openai"`.
    pub provider: String,

    /// Model name understood by the provider.
    ///
    /// Defaults to `"nomic-embed-text"` for Ollama.
    pub model: String,

    /// API endpoint for the embedding provider.
    ///
    /// Defaults to `"http://localhost:11434/v1"` (local Ollama).
    pub endpoint: String,

    /// API key (required for remote OpenAI; omit for Ollama).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            endpoint: "http://localhost:11434/v1".to_string(),
            api_key: None,
        }
    }
}

impl EmbeddingConfig {
    /// Load embedding configuration from environment variables.
    ///
    /// | Variable                              | Default                        |
    /// |---------------------------------------|--------------------------------|
    /// | `AGENTD_INDEX_EMBEDDING_PROVIDER`     | `"ollama"`                     |
    /// | `AGENTD_INDEX_EMBEDDING_MODEL`        | `"nomic-embed-text"`           |
    /// | `AGENTD_INDEX_EMBEDDING_ENDPOINT`     | `"http://localhost:11434/v1"`  |
    /// | `AGENTD_INDEX_EMBEDDING_API_KEY`      | `None`                         |
    pub fn from_env() -> Self {
        Self {
            provider: env::var("AGENTD_INDEX_EMBEDDING_PROVIDER")
                .unwrap_or_else(|_| "ollama".to_string()),
            model: env::var("AGENTD_INDEX_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            endpoint: env::var("AGENTD_INDEX_EMBEDDING_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:11434/v1".to_string()),
            api_key: env::var("AGENTD_INDEX_EMBEDDING_API_KEY").ok(),
        }
    }

    /// Load embedding configuration using shared config base values as fallbacks.
    ///
    /// Uses the shared [`SharedIndexConfig`] for `provider` and `model` defaults,
    /// while keeping `endpoint` and `api_key` as env-var-only settings.
    pub fn load_with_base(base: &SharedIndexConfig) -> Self {
        Self {
            provider: env::var("AGENTD_INDEX_EMBEDDING_PROVIDER")
                .unwrap_or_else(|_| base.embedding_provider.clone()),
            model: env::var("AGENTD_INDEX_EMBEDDING_MODEL")
                .unwrap_or_else(|_| base.embedding_model.clone()),
            endpoint: env::var("AGENTD_INDEX_EMBEDDING_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:11434/v1".to_string()),
            api_key: env::var("AGENTD_INDEX_EMBEDDING_API_KEY").ok(),
        }
    }
}

// ---------------------------------------------------------------------------
// LanceConfig
// ---------------------------------------------------------------------------

/// Configuration for the LanceDB vector store used by the index service.
///
/// # Example
///
/// ```rust
/// use index::config::LanceConfig;
///
/// let config = LanceConfig::default();
/// assert_eq!(config.table, "code_chunks");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LanceConfig {
    /// Filesystem path to the LanceDB directory.
    ///
    /// Defaults to the XDG-compliant data directory for `agentd-index/lancedb`.
    pub path: String,

    /// Table name for code chunk records.
    ///
    /// Defaults to `"code_chunks"`.
    pub table: String,
}

impl Default for LanceConfig {
    fn default() -> Self {
        let path = Self::default_path().to_string_lossy().to_string();
        Self { path, table: "code_chunks".to_string() }
    }
}

impl LanceConfig {
    /// Returns the platform-specific default LanceDB directory path.
    ///
    /// - **Linux**: `~/.local/share/agentd-index/lancedb`
    /// - **macOS**: `~/Library/Application Support/agentd-index/lancedb`
    pub fn default_path() -> PathBuf {
        directories::ProjectDirs::from("", "", "agentd-index")
            .map(|dirs| dirs.data_dir().join("lancedb"))
            .unwrap_or_else(|| PathBuf::from("lancedb"))
    }

    /// Load LanceDB configuration from environment variables.
    ///
    /// | Variable                    | Default                           |
    /// |-----------------------------|-----------------------------------|
    /// | `AGENTD_INDEX_LANCE_PATH`   | XDG data dir / `lancedb`          |
    /// | `AGENTD_INDEX_LANCE_TABLE`  | `"code_chunks"`                   |
    pub fn from_env() -> Self {
        Self {
            path: env::var("AGENTD_INDEX_LANCE_PATH")
                .unwrap_or_else(|_| Self::default_path().to_string_lossy().to_string()),
            table: env::var("AGENTD_INDEX_LANCE_TABLE")
                .unwrap_or_else(|_| "code_chunks".to_string()),
        }
    }

    /// Load LanceDB configuration using shared config base values as fallbacks.
    ///
    /// Uses the shared [`SharedIndexConfig`] for `path` default, while `table`
    /// has no shared equivalent and falls back to `"code_chunks"`.
    pub fn load_with_base(base: &SharedIndexConfig) -> Self {
        Self {
            path: env::var("AGENTD_INDEX_LANCE_PATH").unwrap_or_else(|_| base.lance_path.clone()),
            table: env::var("AGENTD_INDEX_LANCE_TABLE")
                .unwrap_or_else(|_| "code_chunks".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// WatchConfig
// ---------------------------------------------------------------------------

/// Configuration for the file-system watcher used to detect source changes.
///
/// # Example
///
/// ```rust
/// use index::config::WatchConfig;
///
/// let config = WatchConfig::default();
/// assert_eq!(config.interval_secs, 30);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchConfig {
    /// Polling interval in seconds for file change detection.
    ///
    /// Defaults to `30`.
    pub interval_secs: u64,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self { interval_secs: 30 }
    }
}

impl WatchConfig {
    /// Load watch configuration from environment variables.
    ///
    /// | Variable                       | Default |
    /// |--------------------------------|---------|
    /// | `AGENTD_INDEX_WATCH_INTERVAL`  | `30`    |
    pub fn from_env() -> Self {
        let interval_secs = env::var("AGENTD_INDEX_WATCH_INTERVAL")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);
        Self { interval_secs }
    }
}

// ---------------------------------------------------------------------------
// SummaryConfig
// ---------------------------------------------------------------------------

/// Configuration for LLM-generated code summaries.
///
/// # Example
///
/// ```rust
/// use index::config::SummaryConfig;
///
/// let config = SummaryConfig::default();
/// assert!(!config.enabled);
/// assert_eq!(config.model, "qwen2.5-coder:7b");
/// assert_eq!(config.concurrency, 4);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SummaryConfig {
    /// Whether LLM summary generation is enabled.
    ///
    /// Defaults to `false` to avoid blocking initial indexing.
    pub enabled: bool,

    /// Ollama model used for generating summaries.
    ///
    /// Defaults to `"qwen2.5-coder:7b"`.
    pub model: String,

    /// Maximum number of concurrent summary requests to Ollama.
    ///
    /// Defaults to `4`.
    pub concurrency: usize,
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self { enabled: false, model: "qwen2.5-coder:7b".to_string(), concurrency: 4 }
    }
}

impl SummaryConfig {
    /// Load summary configuration from environment variables.
    ///
    /// | Variable                           | Default              |
    /// |------------------------------------|----------------------|
    /// | `AGENTD_INDEX_SUMMARY_ENABLED`     | `false`              |
    /// | `AGENTD_INDEX_SUMMARY_MODEL`       | `"qwen2.5-coder:7b"` |
    /// | `AGENTD_INDEX_SUMMARY_CONCURRENCY` | `4`                  |
    pub fn from_env() -> Self {
        let enabled = env::var("AGENTD_INDEX_SUMMARY_ENABLED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);
        let model = env::var("AGENTD_INDEX_SUMMARY_MODEL")
            .unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());
        let concurrency = env::var("AGENTD_INDEX_SUMMARY_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4);
        Self { enabled, model, concurrency }
    }
}

// ---------------------------------------------------------------------------
// RerankConfig
// ---------------------------------------------------------------------------

/// Configuration for cross-encoder reranking of search results.
///
/// Reranking is disabled by default to avoid adding latency.  Enable it when
/// higher precision of the top-K results is more important than speed.
///
/// # Example
///
/// ```rust
/// use index::config::RerankConfig;
///
/// let config = RerankConfig::default();
/// assert!(!config.enabled);
/// assert_eq!(config.candidates, 30);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RerankConfig {
    /// Whether cross-encoder reranking is enabled.
    ///
    /// Defaults to `false`.
    pub enabled: bool,

    /// Ollama model used for reranking (chat/completion endpoint).
    ///
    /// Defaults to `"qwen2.5-coder:7b"`.
    pub model: String,

    /// Number of candidate results to fetch before reranking.
    ///
    /// The reranker scores this many candidates and returns the top-K.
    /// Defaults to `30`.
    pub candidates: usize,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self { enabled: false, model: "qwen2.5-coder:7b".to_string(), candidates: 30 }
    }
}

impl RerankConfig {
    /// Load rerank configuration from environment variables.
    ///
    /// | Variable                           | Default              |
    /// |------------------------------------|----------------------|
    /// | `AGENTD_INDEX_RERANK_ENABLED`      | `false`              |
    /// | `AGENTD_INDEX_RERANK_MODEL`        | `"qwen2.5-coder:7b"` |
    /// | `AGENTD_INDEX_RERANK_CANDIDATES`   | `30`                 |
    pub fn from_env() -> Self {
        let enabled = env::var("AGENTD_INDEX_RERANK_ENABLED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);
        let model = env::var("AGENTD_INDEX_RERANK_MODEL")
            .unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());
        let candidates = env::var("AGENTD_INDEX_RERANK_CANDIDATES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(30);
        Self { enabled, model, candidates }
    }
}

// ---------------------------------------------------------------------------
// IndexConfig
// ---------------------------------------------------------------------------

/// Top-level configuration for the agentd-index service.
///
/// # Example
///
/// ```rust
/// use index::config::IndexConfig;
///
/// let config = IndexConfig::from_env();
/// assert_eq!(config.port, 17012);
/// assert_eq!(config.lance.table, "code_chunks");
/// assert_eq!(config.embedding.provider, "ollama");
/// assert_eq!(config.watch.interval_secs, 30);
/// ```
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// HTTP listen port (default: 17012).
    pub port: u16,

    /// Embedding provider configuration.
    pub embedding: EmbeddingConfig,

    /// LanceDB storage configuration.
    pub lance: LanceConfig,

    /// File watcher configuration.
    pub watch: WatchConfig,

    /// LLM summary generation configuration.
    pub summary: SummaryConfig,

    /// Cross-encoder reranking configuration.
    pub rerank: RerankConfig,

    /// List of supported programming languages for indexing.
    ///
    /// Defaults to `["rust", "python", "javascript", "typescript"]`.
    pub languages: Vec<String>,

    /// Glob patterns of paths to ignore during indexing.
    ///
    /// Defaults to `[".git", "target", "node_modules", "dist"]`.
    pub ignore_patterns: Vec<String>,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            port: 17012,
            embedding: EmbeddingConfig::default(),
            lance: LanceConfig::default(),
            watch: WatchConfig::default(),
            summary: SummaryConfig::default(),
            rerank: RerankConfig::default(),
            languages: default_languages(),
            ignore_patterns: default_ignore_patterns(),
        }
    }
}

impl IndexConfig {
    /// Load configuration from the shared config file and environment variables.
    ///
    /// Loads base values from [`agentd_common::config::load`], then overlays
    /// legacy service-specific environment variables for backward compatibility.
    ///
    /// | Variable                           | Default                             |
    /// |------------------------------------|-------------------------------------|
    /// | `AGENTD_PORT`                      | `17012`                             |
    /// | `AGENTD_INDEX_LANGUAGES`           | `rust,python,javascript,typescript` |
    /// | `AGENTD_INDEX_IGNORE_PATTERNS`     | `.git,target,node_modules,dist`     |
    /// | `AGENTD_INDEX_SUMMARY_ENABLED`     | `false`                             |
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_default();
        let base = shared.services.index;

        let port =
            env::var("AGENTD_PORT").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(base.port);

        let languages = env::var("AGENTD_INDEX_LANGUAGES")
            .map(|v| parse_csv(&v))
            .unwrap_or_else(|_| base.languages.clone());

        let ignore_patterns = env::var("AGENTD_INDEX_IGNORE_PATTERNS")
            .map(|v| parse_csv(&v))
            .unwrap_or_else(|_| default_ignore_patterns());

        Self {
            port,
            embedding: EmbeddingConfig::load_with_base(&base),
            lance: LanceConfig::load_with_base(&base),
            watch: WatchConfig::from_env(),
            summary: SummaryConfig::from_env(),
            rerank: RerankConfig::from_env(),
            languages,
            ignore_patterns,
        }
    }

    /// Load configuration from environment variables, falling back to defaults.
    #[deprecated(note = "Use load() instead")]
    pub fn from_env() -> Self {
        Self::load()
    }
}

impl ValidateConfig for IndexConfig {
    /// Validate the configuration, returning an error for invalid values.
    ///
    /// Checks:
    /// - Port must be non-zero.
    /// - At least one language must be configured.
    /// - Watch interval must be non-zero.
    /// - Embedding provider must be `"ollama"` or `"openai"`.
    fn validate(&self) -> Result<()> {
        if self.port == 0 {
            bail!("port must be non-zero");
        }
        if self.languages.is_empty() {
            bail!("at least one language must be configured");
        }
        if self.watch.interval_secs == 0 {
            bail!("watch interval must be non-zero");
        }
        match self.embedding.provider.as_str() {
            "ollama" | "openai" => {}
            other => {
                bail!("unsupported embedding provider: {other}; expected 'ollama' or 'openai'")
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_csv(s: &str) -> Vec<String> {
    s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

fn default_languages() -> Vec<String> {
    vec![
        "rust".to_string(),
        "python".to_string(),
        "javascript".to_string(),
        "typescript".to_string(),
    ]
}

fn default_ignore_patterns() -> Vec<String> {
    vec![".git".to_string(), "target".to_string(), "node_modules".to_string(), "dist".to_string()]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentd_common::config::ValidateConfig;
    use std::sync::Mutex;

    /// Serialises tests that call `load()` / `from_env()` so env var mutations
    /// from concurrent tests cannot bleed across.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ── EmbeddingConfig ────────────────────────────────────────────────────

    #[test]
    fn test_embedding_default_provider() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.provider, "ollama");
    }

    #[test]
    fn test_embedding_default_model() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model, "nomic-embed-text");
    }

    #[test]
    fn test_embedding_default_endpoint() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.endpoint, "http://localhost:11434/v1");
    }

    #[test]
    fn test_embedding_default_api_key_is_none() {
        let config = EmbeddingConfig::default();
        assert!(config.api_key.is_none());
    }

    #[allow(deprecated)]
    #[test]
    fn test_embedding_from_env_defaults() {
        let config = EmbeddingConfig::from_env();
        assert!(!config.provider.is_empty());
        assert!(!config.model.is_empty());
        assert!(!config.endpoint.is_empty());
    }

    #[test]
    fn test_embedding_serialization_omits_none_api_key() {
        let config = EmbeddingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("api_key"));
    }

    #[test]
    fn test_embedding_serialization_includes_api_key_when_set() {
        let config = EmbeddingConfig {
            provider: "openai".to_string(),
            model: "text-embedding-3-small".to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            api_key: Some("sk-test".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("sk-test"));
    }

    #[test]
    fn test_embedding_clone() {
        let config = EmbeddingConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.provider, config.provider);
        assert_eq!(cloned.model, config.model);
        assert_eq!(cloned.endpoint, config.endpoint);
    }

    // ── LanceConfig ────────────────────────────────────────────────────────

    #[test]
    fn test_lance_default_table() {
        let config = LanceConfig::default();
        assert_eq!(config.table, "code_chunks");
    }

    #[test]
    fn test_lance_default_path_not_empty() {
        let config = LanceConfig::default();
        assert!(!config.path.is_empty());
    }

    #[test]
    fn test_lance_default_path_contains_agentd_index() {
        let path = LanceConfig::default_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("agentd-index") || path_str.contains("lancedb"));
    }

    #[allow(deprecated)]
    #[test]
    fn test_lance_from_env_defaults() {
        let config = LanceConfig::from_env();
        assert_eq!(config.table, "code_chunks");
        assert!(!config.path.is_empty());
    }

    #[test]
    fn test_lance_serialization_roundtrip() {
        let config =
            LanceConfig { path: "/tmp/test-lance".to_string(), table: "code_chunks".to_string() };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: LanceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, "/tmp/test-lance");
        assert_eq!(parsed.table, "code_chunks");
    }

    #[test]
    fn test_lance_clone() {
        let config = LanceConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.path, config.path);
        assert_eq!(cloned.table, config.table);
    }

    // ── WatchConfig ────────────────────────────────────────────────────────

    #[test]
    fn test_watch_default_interval() {
        let config = WatchConfig::default();
        assert_eq!(config.interval_secs, 30);
    }

    #[test]
    fn test_watch_from_env_defaults() {
        let config = WatchConfig::from_env();
        assert!(config.interval_secs > 0);
    }

    #[test]
    fn test_watch_clone() {
        let config = WatchConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.interval_secs, config.interval_secs);
    }

    // ── SummaryConfig ──────────────────────────────────────────────────────

    #[test]
    fn test_summary_default_disabled() {
        let config = SummaryConfig::default();
        assert!(!config.enabled);
    }

    #[test]
    fn test_summary_default_model() {
        let config = SummaryConfig::default();
        assert_eq!(config.model, "qwen2.5-coder:7b");
    }

    #[test]
    fn test_summary_default_concurrency() {
        let config = SummaryConfig::default();
        assert_eq!(config.concurrency, 4);
    }

    #[test]
    fn test_summary_from_env_defaults() {
        let config = SummaryConfig::from_env();
        assert!(!config.enabled);
        assert!(!config.model.is_empty());
        assert!(config.concurrency > 0);
    }

    #[test]
    fn test_summary_clone() {
        let config = SummaryConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.enabled, config.enabled);
        assert_eq!(cloned.model, config.model);
        assert_eq!(cloned.concurrency, config.concurrency);
    }

    // ── IndexConfig ────────────────────────────────────────────────────────

    #[test]
    fn test_index_default_port() {
        let config = IndexConfig::default();
        assert_eq!(config.port, 17012);
    }

    #[test]
    fn test_index_default_languages() {
        let config = IndexConfig::default();
        assert!(config.languages.contains(&"rust".to_string()));
        assert!(config.languages.contains(&"python".to_string()));
        assert!(config.languages.contains(&"javascript".to_string()));
        assert!(config.languages.contains(&"typescript".to_string()));
    }

    #[test]
    fn test_index_default_ignore_patterns() {
        let config = IndexConfig::default();
        assert!(config.ignore_patterns.contains(&".git".to_string()));
        assert!(config.ignore_patterns.contains(&"target".to_string()));
        assert!(config.ignore_patterns.contains(&"node_modules".to_string()));
        assert!(config.ignore_patterns.contains(&"dist".to_string()));
    }

    #[allow(deprecated)]
    #[test]
    fn test_index_from_env_defaults() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Clear env vars that could override defaults (e.g. AGENTD_PORT set by agentd itself)
        let port_saved = env::var("AGENTD_PORT").ok();
        let index_port_saved = env::var("AGENTD_INDEX_PORT").ok();
        env::remove_var("AGENTD_PORT");
        env::remove_var("AGENTD_INDEX_PORT");

        let config = IndexConfig::from_env();

        if let Some(v) = port_saved {
            env::set_var("AGENTD_PORT", v);
        }
        if let Some(v) = index_port_saved {
            env::set_var("AGENTD_INDEX_PORT", v);
        }

        assert_eq!(config.port, 17012);
        assert!(!config.languages.is_empty());
        assert!(!config.ignore_patterns.is_empty());
    }

    #[test]
    fn test_index_validate_default_passes() {
        let config = IndexConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_index_validate_zero_port_fails() {
        let config = IndexConfig { port: 0, ..Default::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_index_validate_empty_languages_fails() {
        let config = IndexConfig { languages: vec![], ..Default::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_index_validate_zero_watch_interval_fails() {
        let mut config = IndexConfig::default();
        config.watch.interval_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_index_validate_invalid_provider_fails() {
        let mut config = IndexConfig::default();
        config.embedding.provider = "unknown-provider".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_index_validate_openai_provider_passes() {
        let mut config = IndexConfig::default();
        config.embedding.provider = "openai".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_index_clone() {
        let config = IndexConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.port, config.port);
        assert_eq!(cloned.languages, config.languages);
        assert_eq!(cloned.ignore_patterns, config.ignore_patterns);
    }

    // ── parse_csv helper ───────────────────────────────────────────────────

    #[test]
    fn test_parse_csv_basic() {
        let result = parse_csv("rust,python,javascript");
        assert_eq!(result, vec!["rust", "python", "javascript"]);
    }

    #[test]
    fn test_parse_csv_trims_whitespace() {
        let result = parse_csv("rust, python , javascript");
        assert_eq!(result, vec!["rust", "python", "javascript"]);
    }

    #[test]
    fn test_parse_csv_filters_empty() {
        let result = parse_csv("rust,,python");
        assert_eq!(result, vec!["rust", "python"]);
    }
}
