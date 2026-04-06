//! Cross-encoder reranking for search results.
//!
//! [`Reranker`] wraps any [`SearchStrategy`] and re-scores the initial
//! candidate set using an Ollama language model as a cross-encoder: it sends
//! a (query, code-snippet) pair to the model and asks for a relevance score
//! from 0–10.  Results are then re-sorted by this score before being returned.
//!
//! # Configuration
//!
//! Reranking is **disabled by default** to avoid adding latency.  Enable it
//! via [`RerankConfig`]:
//!
//! | Env variable                     | Default          |
//! |----------------------------------|------------------|
//! | `AGENTD_INDEX_RERANK_ENABLED`    | `false`          |
//! | `AGENTD_INDEX_RERANK_MODEL`      | `qwen2.5-coder:7b` |
//! | `AGENTD_INDEX_RERANK_CANDIDATES` | `30`             |
//!
//! # Prompt
//!
//! ```text
//! You are a code relevance judge.
//! Query: <query>
//! Code:
//! <content>
//!
//! Rate how relevant this code is to the query on a scale of 0 to 10.
//! Respond with a single integer only.
//! ```

use std::time::Instant;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::config::RerankConfig;

use super::{SearchError, SearchRequest, SearchResponse, SearchResultItem, SearchStrategy};

// ---------------------------------------------------------------------------
// Ollama chat types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
}

// ---------------------------------------------------------------------------
// Reranker
// ---------------------------------------------------------------------------

/// Cross-encoder reranker that wraps an inner [`SearchStrategy`].
///
/// When `config.enabled` is `false`, this is a transparent pass-through.
pub struct Reranker {
    inner: Box<dyn SearchStrategy>,
    config: RerankConfig,
    client: Client,
    ollama_base_url: String,
}

impl Reranker {
    /// Create a new `Reranker` wrapping `inner`.
    ///
    /// `ollama_base_url` should be the Ollama API base (e.g.
    /// `"http://localhost:11434/v1"`).  When empty, defaults to the standard
    /// local Ollama address.
    pub fn new(
        inner: Box<dyn SearchStrategy>,
        config: RerankConfig,
        ollama_base_url: impl Into<String>,
    ) -> Self {
        let base = {
            let s = ollama_base_url.into();
            if s.is_empty() {
                "http://localhost:11434/v1".to_string()
            } else {
                s
            }
        };
        Self { inner, config, client: Client::new(), ollama_base_url: base }
    }

    /// Score `content` against `query` using the configured cross-encoder model.
    ///
    /// Returns a value in `[0.0, 10.0]`, or `None` if the model response
    /// could not be parsed.
    async fn score_candidate(&self, query: &str, content: &str) -> Option<f32> {
        let prompt = format!(
            "You are a code relevance judge.\n\
             Query: {query}\n\
             Code:\n{content}\n\n\
             Rate how relevant this code is to the query on a scale of 0 to 10.\n\
             Respond with a single integer only."
        );

        let url = format!("{}/chat/completions", self.ollama_base_url);
        let body = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![ChatMessage { role: "user".to_string(), content: prompt }],
            stream: false,
        };

        let resp = self.client.post(&url).json(&body).send().await.ok()?;

        if !resp.status().is_success() {
            warn!(status = %resp.status(), "Reranker: Ollama returned non-2xx status");
            return None;
        }

        let parsed: ChatResponse = resp.json().await.ok()?;
        let text = parsed.message?.content;

        // Extract the first integer token from the response.
        parse_score(&text)
    }
}

#[async_trait]
impl SearchStrategy for Reranker {
    async fn search(&self, request: &SearchRequest) -> Result<SearchResponse, SearchError> {
        if !self.config.enabled {
            // Pass-through when reranking is disabled.
            return self.inner.search(request).await;
        }

        let start = Instant::now();
        let final_limit = request.limit.unwrap_or(10).clamp(1, 100);

        // Over-fetch candidates for the reranker.
        let mut candidate_request = request.clone();
        candidate_request.limit = Some(self.config.candidates.max(final_limit));

        let candidates = self.inner.search(&candidate_request).await?;

        if candidates.results.is_empty() {
            return Ok(candidates);
        }

        debug!("Reranking {} candidates for query {:?}", candidates.results.len(), &request.query);

        // Score each candidate; fall back to original score on failure.
        let mut scored: Vec<(f32, SearchResultItem)> = Vec::with_capacity(candidates.results.len());
        for item in candidates.results {
            let cross_score = self
                .score_candidate(&request.query, &item.content)
                .await
                .unwrap_or(item.score * 10.0); // scale original to 0–10 range
            scored.push((cross_score, item));
        }

        // Re-sort descending by cross-encoder score.
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-`final_limit` and normalise scores back to [0, 1].
        let results: Vec<SearchResultItem> = scored
            .into_iter()
            .take(final_limit)
            .map(|(score, mut item)| {
                item.score = score / 10.0; // normalise to [0, 1]
                item
            })
            .collect();

        let total = results.len();
        let query_time_ms = start.elapsed().as_millis() as u64;

        Ok(SearchResponse { results, total, query_time_ms })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the first integer in the range 0–10 from `text`.
///
/// The model may respond with text like `"7"`, `"Score: 8"`, or even prose.
/// We scan for the first numeric token and clamp it to `[0.0, 10.0]`.
fn parse_score(text: &str) -> Option<f32> {
    for token in text.split_whitespace() {
        // Strip trailing punctuation.
        let clean: String = token.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
        if let Ok(v) = clean.parse::<f32>() {
            return Some(v.clamp(0.0, 10.0));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{SearchMode, SearchResponse, SearchResultItem};
    use async_trait::async_trait;

    // ── parse_score ────────────────────────────────────────────────────────

    #[test]
    fn parse_score_plain_integer() {
        assert_eq!(parse_score("7"), Some(7.0));
    }

    #[test]
    fn parse_score_with_prose() {
        assert_eq!(parse_score("Score: 8 out of 10"), Some(8.0));
    }

    #[test]
    fn parse_score_clamps_above_10() {
        assert_eq!(parse_score("15"), Some(10.0));
    }

    #[test]
    fn parse_score_negative_digit_stripped_to_positive() {
        // The digit filter strips '-', so "-3" → token "3" → Some(3.0).
        assert_eq!(parse_score("-3"), Some(3.0));
    }

    #[test]
    fn parse_score_no_number() {
        assert_eq!(parse_score("excellent"), None);
    }

    #[test]
    fn parse_score_decimal() {
        assert_eq!(parse_score("7.5"), Some(7.5));
    }

    // ── Reranker pass-through ──────────────────────────────────────────────

    struct MockStrategy {
        response: SearchResponse,
    }

    #[async_trait]
    impl SearchStrategy for MockStrategy {
        async fn search(&self, _request: &SearchRequest) -> Result<SearchResponse, SearchError> {
            Ok(self.response.clone())
        }
    }

    fn make_item(id: &str, score: f32) -> SearchResultItem {
        SearchResultItem {
            id: id.to_string(),
            file_path: format!("src/{id}.rs"),
            language: "rust".to_string(),
            chunk_type: "function".to_string(),
            symbol_name: Some(id.to_string()),
            start_line: 1,
            end_line: 5,
            content: format!("fn {id}() {{}}"),
            summary: None,
            score,
            repo_id: "repo1".to_string(),
        }
    }

    fn disabled_config() -> RerankConfig {
        RerankConfig { enabled: false, ..Default::default() }
    }

    #[tokio::test]
    async fn pass_through_when_disabled() {
        let inner_response = SearchResponse {
            results: vec![make_item("fn_a", 0.9), make_item("fn_b", 0.7)],
            total: 2,
            query_time_ms: 10,
        };
        let strategy = MockStrategy { response: inner_response };
        let reranker =
            Reranker::new(Box::new(strategy), disabled_config(), "http://localhost:11434/v1");

        let req = SearchRequest {
            query: "test".to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: Some(10),
            search_mode: SearchMode::Vector,
        };
        let resp = reranker.search(&req).await.unwrap();
        // Order should be unchanged (pass-through)
        assert_eq!(resp.results[0].id, "fn_a");
        assert_eq!(resp.results[1].id, "fn_b");
    }

    #[tokio::test]
    async fn pass_through_preserves_total() {
        let inner_response =
            SearchResponse { results: vec![make_item("fn_x", 0.5)], total: 1, query_time_ms: 5 };
        let strategy = MockStrategy { response: inner_response };
        let reranker =
            Reranker::new(Box::new(strategy), disabled_config(), "http://localhost:11434/v1");

        let req = SearchRequest {
            query: "something".to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: Some(5),
            search_mode: SearchMode::Vector,
        };
        let resp = reranker.search(&req).await.unwrap();
        assert_eq!(resp.total, 1);
    }
}
