//! LLM-generated code summary enrichment pipeline.
//!
//! [`EnrichmentService`] generates natural language descriptions of code chunks
//! using Ollama's chat completion API. Generated summaries are stored in the
//! `summary` column of the LanceDB index to bridge the semantic gap between
//! natural language queries and raw code.
//!
//! Summary generation is **disabled by default** and must be opted-in via
//! [`SummaryConfig::enabled`].
//!
//! # Example
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use index::enrichment::{EnrichmentService, OllamaEnrichment};
//! use index::config::SummaryConfig;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let config = SummaryConfig::default(); // enabled = false
//! let svc = OllamaEnrichment::new(config, "http://localhost:11434/v1".to_string());
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::Semaphore;
use tracing::{debug, warn};

use crate::chunking::types::CodeChunk;
use crate::config::SummaryConfig;

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT: &str =
    "Summarize this code in 1-2 sentences describing what it does, its inputs, and its outputs.";

// ---------------------------------------------------------------------------
// EnrichmentService trait
// ---------------------------------------------------------------------------

/// Async trait for LLM-based code chunk enrichment.
#[async_trait]
pub trait EnrichmentService: Send + Sync {
    /// Generate a natural language summary for a single code chunk.
    ///
    /// Returns `Ok(None)` when summary generation is disabled or the request fails
    /// after all retries.
    async fn summarize(&self, chunk: &CodeChunk) -> Result<Option<String>>;

    /// Summarize a batch of chunks concurrently.
    ///
    /// Returns one `Option<String>` per input chunk in the same order.
    async fn summarize_batch(&self, chunks: &[&CodeChunk]) -> Vec<Option<String>> {
        let futs = chunks.iter().map(|c| self.summarize(c));
        futures::future::join_all(futs).await.into_iter().map(|r| r.unwrap_or(None)).collect()
    }
}

// ---------------------------------------------------------------------------
// OllamaEnrichment
// ---------------------------------------------------------------------------

/// [`EnrichmentService`] implementation that calls the Ollama chat completion API.
///
/// Respects [`SummaryConfig::concurrency`] via a semaphore and retries up to 3
/// times with exponential backoff on transient failures.
pub struct OllamaEnrichment {
    config: SummaryConfig,
    endpoint: String,
    semaphore: Arc<Semaphore>,
    client: reqwest::Client,
}

impl OllamaEnrichment {
    /// Create a new [`OllamaEnrichment`].
    ///
    /// `endpoint` should be the base API URL (e.g. `"http://localhost:11434/v1"`).
    pub fn new(config: SummaryConfig, endpoint: String) -> Self {
        let concurrency = config.concurrency.max(1);
        Self {
            config,
            endpoint,
            semaphore: Arc::new(Semaphore::new(concurrency)),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("failed to build HTTP client"),
        }
    }
}

#[async_trait]
impl EnrichmentService for OllamaEnrichment {
    async fn summarize(&self, chunk: &CodeChunk) -> Result<Option<String>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let _permit = self.semaphore.acquire().await?;

        let user_content = format!(
            "File: {}\nLanguage: {}\n\n```\n{}\n```",
            chunk.file_path, chunk.language, chunk.content
        );

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": user_content},
            ],
            "stream": false,
        });

        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));

        let mut backoff = Duration::from_millis(500);
        for attempt in 0..3u32 {
            match self.client.post(&url).json(&body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp.json().await?;
                    let summary = json["choices"][0]["message"]["content"]
                        .as_str()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    debug!(
                        "Generated summary for '{}' ({} chars)",
                        chunk.file_path,
                        summary.as_ref().map(|s| s.len()).unwrap_or(0)
                    );
                    return Ok(summary);
                }
                Ok(resp) => {
                    warn!(
                        "Summary request failed (attempt {}): HTTP {}",
                        attempt + 1,
                        resp.status()
                    );
                }
                Err(e) => {
                    warn!("Summary request error (attempt {}): {}", attempt + 1, e);
                }
            }
            if attempt < 2 {
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
        }

        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// NoOpEnrichment
// ---------------------------------------------------------------------------

/// [`EnrichmentService`] that never generates summaries.
///
/// Used when summary generation is disabled or in tests that do not need HTTP.
pub struct NoOpEnrichment;

#[async_trait]
impl EnrichmentService for NoOpEnrichment {
    async fn summarize(&self, _chunk: &CodeChunk) -> Result<Option<String>> {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ── MockEnrichment for tests ──────────────────────────────────────────

    /// A mock [`EnrichmentService`] that returns pre-configured summaries.
    pub struct MockEnrichment {
        /// Map from `symbol_name` to the summary to return.
        pub summaries: HashMap<String, String>,
    }

    impl MockEnrichment {
        pub fn new(entries: &[(&str, &str)]) -> Self {
            Self {
                summaries: entries.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            }
        }
    }

    #[async_trait]
    impl EnrichmentService for MockEnrichment {
        async fn summarize(&self, chunk: &CodeChunk) -> Result<Option<String>> {
            Ok(chunk.symbol_name.as_deref().and_then(|name| self.summaries.get(name).cloned()))
        }
    }

    fn make_chunk(symbol_name: &str) -> CodeChunk {
        use crate::chunking::types::{ChunkType, HierarchyLevel, Language};
        CodeChunk {
            content: format!("fn {}() {{}}", symbol_name),
            file_path: "src/lib.rs".to_string(),
            language: Language::Rust,
            chunk_type: ChunkType::Function,
            start_line: 1,
            end_line: 1,
            symbol_name: Some(symbol_name.to_string()),
            parent_symbol: None,
            hierarchy_level: HierarchyLevel::Symbol,
            metadata: Default::default(),
        }
    }

    #[tokio::test]
    async fn noop_enrichment_returns_none() {
        let svc = NoOpEnrichment;
        let chunk = make_chunk("my_fn");
        let result = svc.summarize(&chunk).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn noop_enrichment_batch_all_none() {
        let svc = NoOpEnrichment;
        let c1 = make_chunk("fn_a");
        let c2 = make_chunk("fn_b");
        let results = svc.summarize_batch(&[&c1, &c2]).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_none()));
    }

    #[tokio::test]
    async fn mock_enrichment_returns_configured_summary() {
        let svc = MockEnrichment::new(&[("my_fn", "Adds two numbers together.")]);
        let chunk = make_chunk("my_fn");
        let result = svc.summarize(&chunk).await.unwrap();
        assert_eq!(result.as_deref(), Some("Adds two numbers together."));
    }

    #[tokio::test]
    async fn mock_enrichment_returns_none_for_unknown() {
        let svc = MockEnrichment::new(&[]);
        let chunk = make_chunk("unknown");
        let result = svc.summarize(&chunk).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn mock_enrichment_batch() {
        let svc =
            MockEnrichment::new(&[("add", "Adds two values."), ("sub", "Subtracts two values.")]);
        let c_add = make_chunk("add");
        let c_sub = make_chunk("sub");
        let c_unk = make_chunk("unknown");
        let results = svc.summarize_batch(&[&c_add, &c_sub, &c_unk]).await;
        assert_eq!(results[0].as_deref(), Some("Adds two values."));
        assert_eq!(results[1].as_deref(), Some("Subtracts two values."));
        assert!(results[2].is_none());
    }

    #[tokio::test]
    async fn ollama_enrichment_disabled_returns_none() {
        let config = SummaryConfig { enabled: false, ..SummaryConfig::default() };
        let svc = OllamaEnrichment::new(config, "http://localhost:11434/v1".to_string());
        let chunk = make_chunk("my_fn");
        let result = svc.summarize(&chunk).await.unwrap();
        assert!(result.is_none(), "disabled enrichment should return None");
    }

    #[test]
    fn ollama_enrichment_respects_concurrency() {
        let config = SummaryConfig { concurrency: 2, ..SummaryConfig::default() };
        let svc = OllamaEnrichment::new(config, "http://localhost:11434/v1".to_string());
        assert_eq!(svc.semaphore.available_permits(), 2);
    }
}
