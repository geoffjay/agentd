//! HTTP client for interacting with the ask service Q&A API.
//!
//! # Examples
//!
//! ```no_run
//! use ask::client::AskClient;
//! use ask::types::{CreateQuestionRequest, QuestionPriority};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = AskClient::new("http://localhost:7001");
//!
//! let req = CreateQuestionRequest {
//!     agent_id: "dietician".to_string(),
//!     workflow_id: None,
//!     dispatch_id: None,
//!     category: Some("health".to_string()),
//!     question: "What did you eat yesterday?".to_string(),
//!     context: None,
//!     priority: Some(QuestionPriority::Normal),
//!     expires_in_seconds: Some(86400),
//! };
//!
//! let question = client.create_question(&req).await?;
//! println!("Question ID: {}", question.id);
//! # Ok(())
//! # }
//! ```

use crate::types::{
    AnswerQuestionRequest, CreateQuestionRequest, HealthResponse, ListQuestionsQuery, Question,
    QuestionResponse, QuestionsListResponse,
};
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

/// Client for the ask service REST API.
#[derive(Clone)]
pub struct AskClient {
    client: reqwest::Client,
    pub base_url: String,
}

impl AskClient {
    /// Create a new ask service client.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// Create a new question (called by agents during workflow execution).
    pub async fn create_question(&self, req: &CreateQuestionRequest) -> Result<QuestionResponse> {
        self.post_expecting_status("/questions", req, reqwest::StatusCode::CREATED).await
    }

    /// Answer a question (called by the human user).
    pub async fn answer_question(&self, id: Uuid, answer: &str) -> Result<QuestionResponse> {
        let req = AnswerQuestionRequest { answer: answer.to_string() };
        self.post(&format!("/questions/{id}/answer"), &req).await
    }

    /// Dismiss a question (called by the human user).
    pub async fn dismiss_question(&self, id: Uuid) -> Result<QuestionResponse> {
        self.post(&format!("/questions/{id}/dismiss"), &()).await
    }

    /// List questions with optional filters.
    pub async fn list_questions(
        &self,
        filters: &ListQuestionsQuery,
    ) -> Result<QuestionsListResponse> {
        let mut params = Vec::new();
        if let Some(ref s) = filters.status {
            params.push(format!("status={s}"));
        }
        if let Some(ref a) = filters.agent_id {
            params.push(format!("agent_id={a}"));
        }
        if let Some(ref c) = filters.category {
            params.push(format!("category={c}"));
        }
        if let Some(l) = filters.limit {
            params.push(format!("limit={l}"));
        }
        if let Some(o) = filters.offset {
            params.push(format!("offset={o}"));
        }

        let path = if params.is_empty() {
            "/questions".to_string()
        } else {
            format!("/questions?{}", params.join("&"))
        };
        self.get(&path).await
    }

    /// Get a specific question by UUID.
    pub async fn get_question(&self, id: Uuid) -> Result<Question> {
        self.get(&format!("/questions/{id}")).await
    }

    /// Check the health of the ask service.
    pub async fn health(&self) -> Result<HealthResponse> {
        self.get("/health").await
    }

    // --- Internal helpers ---

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.get(&url).send().await.context(format!("Failed to GET {url}"))?;
        self.handle_response(response).await
    }

    async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to POST {url}"))?;
        self.handle_response(response).await
    }

    async fn post_expecting_status<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
        expected: reqwest::StatusCode,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to POST {url}"))?;

        let status = response.status();
        if status == expected || status.is_success() {
            return response.json().await.context("Failed to parse response JSON");
        }

        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Request failed with status {status}: {body}");
    }

    async fn handle_response<T: DeserializeOwned>(&self, response: reqwest::Response) -> Result<T> {
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }
        response.json().await.context("Failed to parse response JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = AskClient::new("http://localhost:7001");
        assert_eq!(client.base_url, "http://localhost:7001");
    }

    #[test]
    fn test_client_clone() {
        let client1 = AskClient::new("http://localhost:7001");
        let client2 = client1.clone();
        assert_eq!(client1.base_url, client2.base_url);
    }

    #[test]
    fn test_list_questions_query_default() {
        let q = ListQuestionsQuery::default();
        assert!(q.status.is_none());
        assert!(q.agent_id.is_none());
        assert!(q.category.is_none());
    }
}
