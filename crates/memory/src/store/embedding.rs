//! Embedding service implementations for the agentd-memory service.
//!
//! Provides text-to-vector conversion via an OpenAI-compatible HTTP API
//! ([`OpenAIEmbedding`]) and a no-op fallback ([`NoOpEmbedding`]) used when
//! embeddings are not configured.
//!
//! Use [`create_embedding_service`] as the primary entry point — it reads an
//! [`EmbeddingConfig`] and returns the appropriate boxed [`EmbeddingService`].
//!
//! # Provider selection
//!
//! | `config.provider` | Result                              |
//! |-------------------|-------------------------------------|
//! | `"openai"`        | [`OpenAIEmbedding`] (also Ollama)   |
//! | `"none"` / `""`   | [`NoOpEmbedding`] (always errors)   |
//! | anything else     | [`StoreError::InitializationFailed`]|
//!
//! # Ollama (local) usage
//!
//! Point `config.base_url` at `http://localhost:11434/v1` and leave
//! `config.api_key` as `None` — no auth header will be sent.
//!
//! # Example
//!
//! ```rust,no_run
//! use memory::config::EmbeddingConfig;
//! use memory::store::create_embedding_service;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let config = EmbeddingConfig {
//!     provider: "openai".to_string(),
//!     model: "text-embedding-3-small".to_string(),
//!     api_key: Some("sk-...".to_string()),
//!     base_url: None,
//! };
//! let svc = create_embedding_service(&config)?;
//! println!("Dimension: {}", svc.dimension("text-embedding-3-small"));
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::config::EmbeddingConfig;
use crate::error::{StoreError, StoreResult};
use crate::store::EmbeddingService;

// ---------------------------------------------------------------------------
// Dimension lookup
// ---------------------------------------------------------------------------

/// Return the known vector dimension for `model`, or 1536 as a safe default.
///
/// Covers the most common OpenAI and Ollama models. Unknown models fall back
/// to 1536 (the OpenAI default), which is compatible with most downstream
/// tooling.
pub fn model_dimension(model: &str) -> usize {
    match model {
        // OpenAI
        "text-embedding-3-small" => 1536,
        "text-embedding-3-large" => 3072,
        "text-embedding-ada-002" => 1536,
        // Ollama / open-source
        "nomic-embed-text" => 768,
        "mxbai-embed-large" => 1024,
        "all-minilm" => 384,
        "snowflake-arctic-embed" => 1024,
        _ => 1536, // OpenAI-compatible default
    }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible embedding provider
// ---------------------------------------------------------------------------

/// Embedding provider that calls any OpenAI-compatible `/embeddings` endpoint.
///
/// Works with:
/// - **OpenAI** — pass an API key; default base URL is used.
/// - **Ollama** — point `base_url` at `http://localhost:11434/v1`; no API key needed.
/// - **Other compatible APIs** — set `base_url` to your endpoint.
///
/// # Authentication
///
/// An `Authorization: Bearer <key>` header is sent only when `api_key` is
/// non-empty. For `localhost` / `127.0.0.1` URLs the key can be omitted.
pub struct OpenAIEmbedding {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIEmbedding {
    /// Construct a new provider from `config`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InitializationFailed`] when the API key is absent
    /// and the base URL is not a localhost address.
    pub fn new(config: &EmbeddingConfig) -> StoreResult<Self> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        let is_local =
            base_url.contains("localhost") || base_url.contains("127.0.0.1");

        let api_key = match config.api_key.clone() {
            Some(key) => key,
            None if is_local => String::new(),
            None => {
                return Err(StoreError::InitializationFailed(
                    "OpenAI API key required for embeddings. \
                     Set AGENTD_MEMORY_EMBEDDING_API_KEY or use a localhost endpoint."
                        .to_string(),
                ))
            }
        };

        Ok(Self {
            client: Client::new(),
            api_key,
            model: config.model.clone(),
            base_url,
        })
    }
}

// Internal request / response shapes for the OpenAI embeddings endpoint.
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[async_trait]
impl EmbeddingService for OpenAIEmbedding {
    /// Call the `/embeddings` endpoint and return one vector per input text.
    ///
    /// Returns an empty `Vec` when `texts` is empty without making an HTTP
    /// request.
    async fn embed(&self, texts: &[String]) -> StoreResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/embeddings", self.base_url);

        debug!(
            "Generating embeddings for {} texts using model {} at {}",
            texts.len(),
            self.model,
            self.base_url
        );

        let body = EmbeddingRequest {
            model: self.model.clone(),
            input: texts.to_vec(),
        };

        let mut req = self.client.post(&url).header("Content-Type", "application/json");

        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req.json(&body).send().await.map_err(|e| {
            StoreError::QueryFailed(format!("Embedding request failed: {}", e))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(StoreError::QueryFailed(format!(
                "Embedding API error (HTTP {}): {}",
                status, body
            )));
        }

        let parsed: EmbeddingResponse = resp.json().await.map_err(|e| {
            StoreError::InvalidData(format!("Failed to parse embedding response: {}", e))
        })?;

        let embeddings: Vec<Vec<f32>> =
            parsed.data.into_iter().map(|d| d.embedding).collect();

        debug!(
            "Generated {} embeddings with dimension {}",
            embeddings.len(),
            embeddings.first().map(|e| e.len()).unwrap_or(0)
        );

        Ok(embeddings)
    }

    /// Return the vector dimension for `model`.
    ///
    /// When `model` is empty the instance's configured model is used.
    fn dimension(&self, model: &str) -> usize {
        let m = if model.is_empty() { self.model.as_str() } else { model };
        model_dimension(m)
    }
}

// ---------------------------------------------------------------------------
// No-op fallback
// ---------------------------------------------------------------------------

/// Embedding service that always returns an error.
///
/// Used when no provider is configured (`provider = "none"`). All calls to
/// [`embed`] fail with [`StoreError::InitializationFailed`], clearly
/// indicating that the service needs to be configured.
pub struct NoOpEmbedding;

impl NoOpEmbedding {
    /// Create a new `NoOpEmbedding`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoOpEmbedding {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingService for NoOpEmbedding {
    async fn embed(&self, _texts: &[String]) -> StoreResult<Vec<Vec<f32>>> {
        Err(StoreError::InitializationFailed(
            "Embedding service not configured. \
             Set AGENTD_MEMORY_EMBEDDING_PROVIDER and related environment variables."
                .to_string(),
        ))
    }

    /// Always returns `0` — no embeddings are produced by this provider.
    fn dimension(&self, _model: &str) -> usize {
        0
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Build an [`EmbeddingService`] from `config`.
///
/// # Errors
///
/// - [`StoreError::InitializationFailed`] if `config.provider` is `"openai"`
///   but no API key is provided for a remote endpoint.
/// - [`StoreError::InitializationFailed`] for unknown provider names.
pub fn create_embedding_service(
    config: &EmbeddingConfig,
) -> StoreResult<Box<dyn EmbeddingService>> {
    match config.provider.to_lowercase().as_str() {
        "openai" => {
            let svc = OpenAIEmbedding::new(config)?;
            Ok(Box::new(svc))
        }
        "none" | "" => {
            tracing::warn!(
                "No embedding provider configured — semantic search will not work. \
                 Set AGENTD_MEMORY_EMBEDDING_PROVIDER=openai to enable."
            );
            Ok(Box::new(NoOpEmbedding::new()))
        }
        other => Err(StoreError::InitializationFailed(format!(
            "Unknown embedding provider: '{}'. Supported providers: openai, none",
            other
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn openai_config(model: &str) -> EmbeddingConfig {
        EmbeddingConfig {
            provider: "openai".to_string(),
            model: model.to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
        }
    }

    fn local_config(model: &str) -> EmbeddingConfig {
        EmbeddingConfig {
            provider: "openai".to_string(),
            model: model.to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434/v1".to_string()),
        }
    }

    // ── model_dimension lookup ─────────────────────────────────────────────

    #[test]
    fn test_model_dimension_text_embedding_3_small() {
        assert_eq!(model_dimension("text-embedding-3-small"), 1536);
    }

    #[test]
    fn test_model_dimension_text_embedding_3_large() {
        assert_eq!(model_dimension("text-embedding-3-large"), 3072);
    }

    #[test]
    fn test_model_dimension_ada_002() {
        assert_eq!(model_dimension("text-embedding-ada-002"), 1536);
    }

    #[test]
    fn test_model_dimension_nomic_embed_text() {
        assert_eq!(model_dimension("nomic-embed-text"), 768);
    }

    #[test]
    fn test_model_dimension_mxbai_embed_large() {
        assert_eq!(model_dimension("mxbai-embed-large"), 1024);
    }

    #[test]
    fn test_model_dimension_all_minilm() {
        assert_eq!(model_dimension("all-minilm"), 384);
    }

    #[test]
    fn test_model_dimension_snowflake_arctic_embed() {
        assert_eq!(model_dimension("snowflake-arctic-embed"), 1024);
    }

    #[test]
    fn test_model_dimension_unknown_defaults_to_1536() {
        assert_eq!(model_dimension("some-unknown-model"), 1536);
    }

    // ── OpenAIEmbedding construction ───────────────────────────────────────

    #[test]
    fn test_openai_construction_with_api_key() {
        let config = openai_config("text-embedding-3-small");
        assert!(OpenAIEmbedding::new(&config).is_ok());
    }

    #[test]
    fn test_openai_requires_api_key_for_remote() {
        let config = EmbeddingConfig {
            provider: "openai".to_string(),
            model: "text-embedding-3-small".to_string(),
            api_key: None,
            base_url: None,
        };
        match OpenAIEmbedding::new(&config) {
            Err(e) => assert!(e.to_string().contains("API key required")),
            Ok(_) => panic!("Expected error when API key missing for remote service"),
        }
    }

    #[test]
    fn test_openai_no_key_required_for_localhost() {
        let config = local_config("nomic-embed-text");
        assert!(OpenAIEmbedding::new(&config).is_ok());
    }

    #[test]
    fn test_openai_no_key_required_for_127_0_0_1() {
        let config = EmbeddingConfig {
            provider: "openai".to_string(),
            model: "nomic-embed-text".to_string(),
            api_key: None,
            base_url: Some("http://127.0.0.1:11434/v1".to_string()),
        };
        assert!(OpenAIEmbedding::new(&config).is_ok());
    }

    // ── OpenAIEmbedding::dimension ─────────────────────────────────────────

    #[test]
    fn test_dimension_configured_model_when_arg_empty() {
        let svc = OpenAIEmbedding::new(&openai_config("text-embedding-3-large")).unwrap();
        assert_eq!(svc.dimension(""), 3072);
    }

    #[test]
    fn test_dimension_uses_arg_model_when_provided() {
        let svc = OpenAIEmbedding::new(&openai_config("text-embedding-3-small")).unwrap();
        assert_eq!(svc.dimension("text-embedding-3-large"), 3072);
    }

    #[test]
    fn test_dimension_small_via_trait() {
        let svc = OpenAIEmbedding::new(&openai_config("text-embedding-3-small")).unwrap();
        assert_eq!(svc.dimension("text-embedding-3-small"), 1536);
    }

    #[test]
    fn test_dimension_ollama_nomic() {
        let svc = OpenAIEmbedding::new(&local_config("nomic-embed-text")).unwrap();
        assert_eq!(svc.dimension("nomic-embed-text"), 768);
    }

    #[test]
    fn test_dimension_ollama_mxbai() {
        let svc = OpenAIEmbedding::new(&local_config("mxbai-embed-large")).unwrap();
        assert_eq!(svc.dimension("mxbai-embed-large"), 1024);
    }

    #[test]
    fn test_dimension_all_minilm() {
        let svc = OpenAIEmbedding::new(&local_config("all-minilm")).unwrap();
        assert_eq!(svc.dimension("all-minilm"), 384);
    }

    // ── NoOpEmbedding ──────────────────────────────────────────────────────

    #[test]
    fn test_noop_dimension_is_zero() {
        let svc = NoOpEmbedding::new();
        assert_eq!(svc.dimension("text-embedding-3-small"), 0);
    }

    #[test]
    fn test_noop_default_dimension_is_zero() {
        let svc = NoOpEmbedding::default();
        assert_eq!(svc.dimension(""), 0);
    }

    #[tokio::test]
    async fn test_noop_embed_returns_error() {
        let svc = NoOpEmbedding::new();
        let result = svc.embed(&["hello".to_string()]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not configured"));
    }

    // ── create_embedding_service factory ──────────────────────────────────

    #[test]
    fn test_factory_openai_provider() {
        let config = openai_config("text-embedding-3-small");
        let result = create_embedding_service(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().dimension("text-embedding-3-small"), 1536);
    }

    #[test]
    fn test_factory_none_provider() {
        let config = EmbeddingConfig {
            provider: "none".to_string(),
            model: String::new(),
            api_key: None,
            base_url: None,
        };
        let result = create_embedding_service(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().dimension(""), 0);
    }

    #[test]
    fn test_factory_empty_provider_treated_as_none() {
        let config = EmbeddingConfig {
            provider: String::new(),
            model: String::new(),
            api_key: None,
            base_url: None,
        };
        let result = create_embedding_service(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().dimension(""), 0);
    }

    #[test]
    fn test_factory_unknown_provider_errors() {
        let config = EmbeddingConfig {
            provider: "unknown_provider".to_string(),
            model: String::new(),
            api_key: None,
            base_url: None,
        };
        match create_embedding_service(&config) {
            Err(e) => assert!(e.to_string().contains("Unknown embedding provider")),
            Ok(_) => panic!("Expected error for unknown provider"),
        }
    }

    #[test]
    fn test_factory_provider_case_insensitive() {
        let config = EmbeddingConfig {
            provider: "OpenAI".to_string(),
            model: "text-embedding-3-small".to_string(),
            api_key: Some("key".to_string()),
            base_url: None,
        };
        assert!(create_embedding_service(&config).is_ok());
    }

    #[test]
    fn test_factory_openai_dimension_large() {
        let config = openai_config("text-embedding-3-large");
        let svc = create_embedding_service(&config).unwrap();
        assert_eq!(svc.dimension("text-embedding-3-large"), 3072);
    }

    #[tokio::test]
    async fn test_openai_embed_empty_slice_returns_empty() {
        let svc = OpenAIEmbedding::new(&openai_config("text-embedding-3-small")).unwrap();
        let result = svc.embed(&[]).await.unwrap();
        assert!(result.is_empty());
    }
}
