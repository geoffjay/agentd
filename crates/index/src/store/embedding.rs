//! Embedding service implementations for the agentd-index service.
//!
//! Provides text-to-vector conversion via an OpenAI-compatible HTTP API
//! ([`OllamaEmbedding`] / [`OpenAIEmbedding`]) and a no-op fallback
//! ([`NoOpEmbedding`]) used when embeddings are not configured.
//!
//! Use [`create_embedding_service`] as the primary entry point — it reads an
//! [`EmbeddingConfig`] and returns the appropriate boxed [`EmbeddingService`].
//!
//! # Ollama usage (default)
//!
//! The default configuration points at `http://localhost:11434/v1` with
//! `nomic-embed-code` (768 dimensions).  No API key is required.
//!
//! # OpenAI usage
//!
//! Set `provider = "openai"` and supply `api_key`.  The standard OpenAI base
//! URL is used unless overridden with `endpoint`.
//!
//! # Example
//!
//! ```rust,no_run
//! use index::config::EmbeddingConfig;
//! use index::store::create_embedding_service;
//!
//! let config = EmbeddingConfig::default();
//! let svc = create_embedding_service(&config).unwrap();
//! assert_eq!(svc.dimension("nomic-embed-code"), 768);
//! ```

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::config::EmbeddingConfig;
use crate::store::error::{StoreError, StoreResult};
use crate::store::traits::EmbeddingService;

// ---------------------------------------------------------------------------
// Dimension lookup
// ---------------------------------------------------------------------------

/// Return the known vector dimension for `model`, or 768 as a default.
///
/// Code-specialised models default to 768 (nomic-embed-code). Unknown models
/// fall back to 768.
pub fn model_dimension(model: &str) -> usize {
    match model {
        // Ollama / code-specialised
        "nomic-embed-code" => 768,
        "nomic-embed-text" => 768,
        "mxbai-embed-large" => 1024,
        "all-minilm" => 384,
        "snowflake-arctic-embed" => 1024,
        // OpenAI
        "text-embedding-3-small" => 1536,
        "text-embedding-3-large" => 3072,
        "text-embedding-ada-002" => 1536,
        _ => 768, // nomic-embed-code default
    }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible embedding provider (covers Ollama + OpenAI)
// ---------------------------------------------------------------------------

/// Embedding provider that calls any OpenAI-compatible `/embeddings` endpoint.
///
/// Works with:
/// - **Ollama** — point `endpoint` at `http://localhost:11434/v1`; no API key.
/// - **OpenAI** — pass an API key; default base URL is used.
/// - **Other compatible APIs** — set `endpoint` to your service URL.
pub struct OllamaEmbedding {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OllamaEmbedding {
    /// Construct a new provider from `config`.
    ///
    /// An API key is only required when the endpoint is not localhost/127.0.0.1
    /// and the provider is `"openai"`.
    pub fn new(config: &EmbeddingConfig) -> StoreResult<Self> {
        let base_url = config.endpoint.clone();
        let is_local = base_url.contains("localhost") || base_url.contains("127.0.0.1");

        let api_key = match config.api_key.clone() {
            Some(key) => key,
            None if is_local => String::new(),
            None if config.provider == "ollama" => String::new(),
            None => {
                return Err(StoreError::InitializationFailed(
                    "API key required for remote embedding endpoint. \
                     Set AGENTD_INDEX_EMBEDDING_API_KEY or use a localhost endpoint."
                        .to_string(),
                ))
            }
        };

        Ok(Self { client: Client::new(), api_key, model: config.model.clone(), base_url })
    }
}

// Internal shapes for the OpenAI-compatible embeddings API.
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
impl EmbeddingService for OllamaEmbedding {
    /// Call the `/embeddings` endpoint and return one vector per input text.
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

        let body = EmbeddingRequest { model: self.model.clone(), input: texts.to_vec() };

        let mut req = self.client.post(&url).header("Content-Type", "application/json");

        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Embedding request failed: {}", e)))?;

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

        let embeddings: Vec<Vec<f32>> = parsed.data.into_iter().map(|d| d.embedding).collect();

        debug!(
            "Generated {} embeddings with dimension {}",
            embeddings.len(),
            embeddings.first().map(|e| e.len()).unwrap_or(0)
        );

        Ok(embeddings)
    }

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
/// Used when no provider is configured (`provider = "none"`).
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
             Set AGENTD_INDEX_EMBEDDING_PROVIDER and related environment variables."
                .to_string(),
        ))
    }

    fn dimension(&self, _model: &str) -> usize {
        0
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Build an [`EmbeddingService`] from `config`.
///
/// | `config.provider` | Result                                |
/// |-------------------|---------------------------------------|
/// | `"ollama"`        | [`OllamaEmbedding`] (no key needed)   |
/// | `"openai"`        | [`OllamaEmbedding`] (key required)    |
/// | `"none"` / `""`   | [`NoOpEmbedding`] (always errors)     |
/// | anything else     | [`StoreError::InitializationFailed`]  |
pub fn create_embedding_service(
    config: &EmbeddingConfig,
) -> StoreResult<Box<dyn EmbeddingService>> {
    match config.provider.to_lowercase().as_str() {
        "ollama" | "openai" => {
            let svc = OllamaEmbedding::new(config)?;
            Ok(Box::new(svc))
        }
        "none" | "" => {
            tracing::warn!(
                "No embedding provider configured — vector search will not work. \
                 Set AGENTD_INDEX_EMBEDDING_PROVIDER=ollama to enable."
            );
            Ok(Box::new(NoOpEmbedding::new()))
        }
        other => Err(StoreError::InitializationFailed(format!(
            "Unknown embedding provider: '{}'. Supported: ollama, openai, none",
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

    fn ollama_config(model: &str) -> EmbeddingConfig {
        EmbeddingConfig {
            provider: "ollama".to_string(),
            model: model.to_string(),
            endpoint: "http://localhost:11434/v1".to_string(),
            api_key: None,
        }
    }

    fn openai_config(model: &str) -> EmbeddingConfig {
        EmbeddingConfig {
            provider: "openai".to_string(),
            model: model.to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            api_key: Some("sk-test".to_string()),
        }
    }

    // ── model_dimension ────────────────────────────────────────────────────

    #[test]
    fn nomic_embed_code_is_768() {
        assert_eq!(model_dimension("nomic-embed-code"), 768);
    }

    #[test]
    fn nomic_embed_text_is_768() {
        assert_eq!(model_dimension("nomic-embed-text"), 768);
    }

    #[test]
    fn mxbai_is_1024() {
        assert_eq!(model_dimension("mxbai-embed-large"), 1024);
    }

    #[test]
    fn all_minilm_is_384() {
        assert_eq!(model_dimension("all-minilm"), 384);
    }

    #[test]
    fn openai_small_is_1536() {
        assert_eq!(model_dimension("text-embedding-3-small"), 1536);
    }

    #[test]
    fn unknown_model_defaults_to_768() {
        assert_eq!(model_dimension("some-unknown-model"), 768);
    }

    // ── OllamaEmbedding construction ───────────────────────────────────────

    #[test]
    fn ollama_no_key_for_localhost() {
        let config = ollama_config("nomic-embed-code");
        assert!(OllamaEmbedding::new(&config).is_ok());
    }

    #[test]
    fn openai_with_api_key_ok() {
        let config = openai_config("text-embedding-3-small");
        assert!(OllamaEmbedding::new(&config).is_ok());
    }

    #[test]
    fn openai_no_key_for_remote_errors() {
        let config = EmbeddingConfig {
            provider: "openai".to_string(),
            model: "text-embedding-3-small".to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            api_key: None,
        };
        assert!(OllamaEmbedding::new(&config).is_err());
    }

    // ── dimension via trait ────────────────────────────────────────────────

    #[test]
    fn dimension_uses_configured_model_when_arg_empty() {
        let svc = OllamaEmbedding::new(&ollama_config("nomic-embed-code")).unwrap();
        assert_eq!(svc.dimension(""), 768);
    }

    #[test]
    fn dimension_uses_arg_when_provided() {
        let svc = OllamaEmbedding::new(&ollama_config("nomic-embed-code")).unwrap();
        assert_eq!(svc.dimension("mxbai-embed-large"), 1024);
    }

    // ── NoOpEmbedding ──────────────────────────────────────────────────────

    #[test]
    fn noop_dimension_is_zero() {
        assert_eq!(NoOpEmbedding::new().dimension("nomic-embed-code"), 0);
    }

    #[tokio::test]
    async fn noop_embed_returns_error() {
        let svc = NoOpEmbedding::new();
        let result = svc.embed(&["hello".to_string()]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not configured"));
    }

    #[tokio::test]
    async fn embed_empty_returns_empty() {
        let svc = OllamaEmbedding::new(&ollama_config("nomic-embed-code")).unwrap();
        // Empty slice should short-circuit without network call.
        let result = svc.embed(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    // ── factory ────────────────────────────────────────────────────────────

    #[test]
    fn factory_ollama_provider() {
        let config = ollama_config("nomic-embed-code");
        let svc = create_embedding_service(&config).unwrap();
        assert_eq!(svc.dimension("nomic-embed-code"), 768);
    }

    #[test]
    fn factory_openai_provider() {
        let config = openai_config("text-embedding-3-small");
        let svc = create_embedding_service(&config).unwrap();
        assert_eq!(svc.dimension("text-embedding-3-small"), 1536);
    }

    #[test]
    fn factory_none_provider() {
        let config = EmbeddingConfig {
            provider: "none".to_string(),
            model: String::new(),
            endpoint: String::new(),
            api_key: None,
        };
        let svc = create_embedding_service(&config).unwrap();
        assert_eq!(svc.dimension(""), 0);
    }

    #[test]
    fn factory_unknown_provider_errors() {
        let config = EmbeddingConfig {
            provider: "chroma".to_string(),
            model: String::new(),
            endpoint: String::new(),
            api_key: None,
        };
        assert!(create_embedding_service(&config).is_err());
    }

    #[test]
    fn factory_case_insensitive() {
        let config = EmbeddingConfig {
            provider: "Ollama".to_string(),
            model: "nomic-embed-code".to_string(),
            endpoint: "http://localhost:11434/v1".to_string(),
            api_key: None,
        };
        assert!(create_embedding_service(&config).is_ok());
    }
}
