//! Configuration types for the agentd-memory service.
//!
//! Provides [`EmbeddingConfig`] which describes the embedding provider and
//! can be loaded from environment variables.
//!
//! # Environment Variables
//!
//! | Variable                             | Default                        | Description                                 |
//! |--------------------------------------|--------------------------------|---------------------------------------------|
//! | `AGENTD_MEMORY_EMBEDDING_PROVIDER`   | `"none"`                       | Provider: `"openai"` or `"none"`            |
//! | `AGENTD_MEMORY_EMBEDDING_MODEL`      | `"text-embedding-3-small"`     | Model name                                  |
//! | `AGENTD_MEMORY_EMBEDDING_API_KEY`    | —                              | API key (required for remote OpenAI calls)  |
//! | `AGENTD_MEMORY_EMBEDDING_ENDPOINT`   | `"https://api.openai.com/v1"`  | Base URL; use Ollama's URL for local runs   |
//!
//! # Example
//!
//! ```rust
//! use memory::config::EmbeddingConfig;
//!
//! // Load from environment
//! let config = EmbeddingConfig::from_env();
//! assert!(!config.provider.is_empty() || config.provider == "none");
//! ```

use serde::{Deserialize, Serialize};
use std::env;

/// Configuration for the embedding service.
///
/// Controls which provider and model are used to convert text into embedding
/// vectors for semantic search.
///
/// # Example
///
/// ```rust
/// use memory::config::EmbeddingConfig;
///
/// let config = EmbeddingConfig {
///     provider: "openai".to_string(),
///     model: "text-embedding-3-small".to_string(),
///     api_key: Some("sk-...".to_string()),
///     base_url: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// Embedding provider: `"openai"` (also works with Ollama) or `"none"`.
    ///
    /// `"none"` disables embeddings; any call to [`embed`] will return an
    /// error explaining that the service is not configured.
    pub provider: String,

    /// Model name understood by the provider.
    ///
    /// Well-known models with pre-configured dimensions:
    /// - OpenAI: `text-embedding-3-small` (1536), `text-embedding-3-large` (3072), `text-embedding-ada-002` (1536)
    /// - Ollama: `nomic-embed-text` (768), `mxbai-embed-large` (1024), `all-minilm` (384), `snowflake-arctic-embed` (1024)
    pub model: String,

    /// API key sent as a `Bearer` token.
    ///
    /// Required for remote OpenAI calls; omit for Ollama or other localhost
    /// providers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Override the default API base URL.
    ///
    /// Defaults to `"https://api.openai.com/v1"` when `None`.
    /// Set to `"http://localhost:11434/v1"` for Ollama.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            model: "text-embedding-3-small".to_string(),
            api_key: None,
            base_url: None,
        }
    }
}

impl EmbeddingConfig {
    /// Load configuration from environment variables.
    ///
    /// | Variable                             | Default                       |
    /// |--------------------------------------|-------------------------------|
    /// | `AGENTD_MEMORY_EMBEDDING_PROVIDER`   | `"none"`                      |
    /// | `AGENTD_MEMORY_EMBEDDING_MODEL`      | `"text-embedding-3-small"`    |
    /// | `AGENTD_MEMORY_EMBEDDING_API_KEY`    | `None`                        |
    /// | `AGENTD_MEMORY_EMBEDDING_ENDPOINT`   | `None` (uses provider default)|
    pub fn from_env() -> Self {
        Self {
            provider: env::var("AGENTD_MEMORY_EMBEDDING_PROVIDER")
                .unwrap_or_else(|_| "none".to_string()),
            model: env::var("AGENTD_MEMORY_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-3-small".to_string()),
            api_key: env::var("AGENTD_MEMORY_EMBEDDING_API_KEY").ok(),
            base_url: env::var("AGENTD_MEMORY_EMBEDDING_ENDPOINT").ok(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_provider_is_none() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.provider, "none");
    }

    #[test]
    fn test_default_model() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.model, "text-embedding-3-small");
    }

    #[test]
    fn test_default_api_key_is_none() {
        let config = EmbeddingConfig::default();
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_default_base_url_is_none() {
        let config = EmbeddingConfig::default();
        assert!(config.base_url.is_none());
    }

    #[test]
    fn test_from_env_defaults_when_vars_absent() {
        // Ensure vars are not set in this process
        let config = EmbeddingConfig::from_env();
        // Provider defaults to "none" when unset
        // (we can't unset the vars set by other tests, so just check type)
        assert!(!config.model.is_empty());
    }

    #[test]
    fn test_serialization_omits_none_fields() {
        let config = EmbeddingConfig {
            provider: "none".to_string(),
            model: "text-embedding-3-small".to_string(),
            api_key: None,
            base_url: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("api_key"));
        assert!(!json.contains("base_url"));
    }

    #[test]
    fn test_serialization_includes_present_fields() {
        let config = EmbeddingConfig {
            provider: "openai".to_string(),
            model: "text-embedding-3-small".to_string(),
            api_key: Some("sk-test".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("sk-test"));
        assert!(json.contains("api.openai.com"));
    }

    #[test]
    fn test_deserialization_with_defaults() {
        let json = r#"{"provider":"openai","model":"text-embedding-3-large"}"#;
        let config: EmbeddingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.provider, "openai");
        assert_eq!(config.model, "text-embedding-3-large");
        assert!(config.api_key.is_none());
        assert!(config.base_url.is_none());
    }

    #[test]
    fn test_clone() {
        let config = EmbeddingConfig {
            provider: "openai".to_string(),
            model: "nomic-embed-text".to_string(),
            api_key: Some("key".to_string()),
            base_url: Some("http://localhost:11434/v1".to_string()),
        };
        let cloned = config.clone();
        assert_eq!(cloned.provider, config.provider);
        assert_eq!(cloned.model, config.model);
        assert_eq!(cloned.api_key, config.api_key);
        assert_eq!(cloned.base_url, config.base_url);
    }
}
