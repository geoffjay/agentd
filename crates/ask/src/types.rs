//! Request and response types for the ask service (redesigned for agent-driven Q&A).
//!
//! This module defines all data structures used in API requests and responses.
//! The ask service is a purpose-built agent-to-human question/answer system.
//!
//! # Type Categories
//!
//! - **Question Types**: Represent questions agents ask the human user
//! - **Request/Response Types**: API endpoint request and response structures
//!
//! # Examples
//!
//! ## Creating a question request
//!
//! ```
//! use ask::types::{CreateQuestionRequest, QuestionPriority};
//!
//! let request = CreateQuestionRequest {
//!     agent_id: "dietician".to_string(),
//!     workflow_id: None,
//!     dispatch_id: None,
//!     category: Some("health".to_string()),
//!     question: "What did you eat yesterday?".to_string(),
//!     context: Some("Daily nutrition tracking".to_string()),
//!     priority: Some(QuestionPriority::Normal),
//!     expires_in_seconds: Some(86400),
//! };
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Priority of a question.
///
/// Determines urgency and display ordering for the human user.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuestionPriority {
    /// Low priority — informational, can be answered later.
    Low,
    /// Normal priority — standard questions (default).
    #[default]
    Normal,
    /// High priority — should be answered soon.
    High,
    /// Urgent priority — requires prompt attention.
    Urgent,
}

impl QuestionPriority {
    /// Returns the string representation for storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionPriority::Low => "low",
            QuestionPriority::Normal => "normal",
            QuestionPriority::High => "high",
            QuestionPriority::Urgent => "urgent",
        }
    }
}

impl std::fmt::Display for QuestionPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for QuestionPriority {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(QuestionPriority::Low),
            "normal" => Ok(QuestionPriority::Normal),
            "high" => Ok(QuestionPriority::High),
            "urgent" => Ok(QuestionPriority::Urgent),
            other => anyhow::bail!("Unknown question priority: {}", other),
        }
    }
}

/// Status of a question in its lifecycle.
///
/// Tracks the state from creation through resolution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum QuestionStatus {
    /// Question is awaiting user response.
    #[default]
    Pending,
    /// User has provided an answer.
    Answered,
    /// User dismissed the question without answering.
    Dismissed,
    /// Question expired before being answered.
    Expired,
}

impl QuestionStatus {
    /// Returns the string representation for storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionStatus::Pending => "Pending",
            QuestionStatus::Answered => "Answered",
            QuestionStatus::Dismissed => "Dismissed",
            QuestionStatus::Expired => "Expired",
        }
    }
}

impl std::fmt::Display for QuestionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for QuestionStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(QuestionStatus::Pending),
            "Answered" => Ok(QuestionStatus::Answered),
            "Dismissed" => Ok(QuestionStatus::Dismissed),
            "Expired" => Ok(QuestionStatus::Expired),
            other => anyhow::bail!("Unknown question status: {}", other),
        }
    }
}

/// A question from an agent to the human user.
///
/// Represents the full state of a question from creation through resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    /// Unique ID for this question.
    pub id: Uuid,
    /// Which agent asked this question.
    pub agent_id: String,
    /// Which workflow triggered the question (if any).
    pub workflow_id: Option<Uuid>,
    /// Which dispatch triggered the question (if any).
    pub dispatch_id: Option<Uuid>,
    /// Optional category for filtering (e.g. "health", "productivity", "deployment").
    pub category: Option<String>,
    /// The question text.
    pub question: String,
    /// Additional context for the human.
    pub context: Option<String>,
    /// Priority level.
    pub priority: QuestionPriority,
    /// Current lifecycle status.
    pub status: QuestionStatus,
    /// Human's response (if provided).
    pub answer: Option<String>,
    /// When the question was asked.
    pub asked_at: DateTime<Utc>,
    /// When the question was answered or dismissed.
    pub answered_at: Option<DateTime<Utc>>,
    /// When the question expires (optional TTL).
    pub expires_at: Option<DateTime<Utc>>,
}

/// Request to create a new question (sent by an agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateQuestionRequest {
    /// Which agent is asking.
    pub agent_id: String,
    /// Workflow that triggered this question (optional).
    pub workflow_id: Option<Uuid>,
    /// Dispatch that triggered this question (optional).
    pub dispatch_id: Option<Uuid>,
    /// Category for filtering (optional).
    pub category: Option<String>,
    /// The question text (required, non-empty).
    pub question: String,
    /// Additional context for the human (optional).
    pub context: Option<String>,
    /// Priority level (default: Normal).
    pub priority: Option<QuestionPriority>,
    /// Time-to-live in seconds (optional).
    pub expires_in_seconds: Option<u64>,
}

/// Request to submit an answer to a question (sent by the human).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerQuestionRequest {
    /// The human's answer text.
    pub answer: String,
}

/// Query parameters for `GET /questions`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuestionsQuery {
    /// Filter by status: `"Pending"`, `"Answered"`, `"Dismissed"`, or `"Expired"`.
    pub status: Option<String>,
    /// Filter by agent ID.
    pub agent_id: Option<String>,
    /// Filter by category.
    pub category: Option<String>,
    /// Maximum number of results to return (default: 50).
    pub limit: Option<u64>,
    /// Offset for pagination (default: 0).
    pub offset: Option<u64>,
}

/// Response for a single question (used for create, answer, get endpoints).
pub type QuestionResponse = Question;

/// Response from the `GET /questions` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionsListResponse {
    /// Questions matching the query.
    pub questions: Vec<Question>,
    /// Total count of matching questions.
    pub total: usize,
}

// Re-export shared HealthResponse from agentd-common.
pub use agentd_common::types::HealthResponse;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_question_priority_as_str() {
        assert_eq!(QuestionPriority::Low.as_str(), "low");
        assert_eq!(QuestionPriority::Normal.as_str(), "normal");
        assert_eq!(QuestionPriority::High.as_str(), "high");
        assert_eq!(QuestionPriority::Urgent.as_str(), "urgent");
    }

    #[test]
    fn test_question_priority_ordering() {
        assert!(QuestionPriority::Low < QuestionPriority::Normal);
        assert!(QuestionPriority::Normal < QuestionPriority::High);
        assert!(QuestionPriority::High < QuestionPriority::Urgent);
    }

    #[test]
    fn test_question_priority_from_str() {
        assert_eq!("low".parse::<QuestionPriority>().unwrap(), QuestionPriority::Low);
        assert_eq!("normal".parse::<QuestionPriority>().unwrap(), QuestionPriority::Normal);
        assert_eq!("high".parse::<QuestionPriority>().unwrap(), QuestionPriority::High);
        assert_eq!("urgent".parse::<QuestionPriority>().unwrap(), QuestionPriority::Urgent);
        assert!("invalid".parse::<QuestionPriority>().is_err());
    }

    #[test]
    fn test_question_priority_serialization() {
        let p = QuestionPriority::High;
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "\"high\"");
        let deserialized: QuestionPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, p);
    }

    #[test]
    fn test_question_status_as_str() {
        assert_eq!(QuestionStatus::Pending.as_str(), "Pending");
        assert_eq!(QuestionStatus::Answered.as_str(), "Answered");
        assert_eq!(QuestionStatus::Dismissed.as_str(), "Dismissed");
        assert_eq!(QuestionStatus::Expired.as_str(), "Expired");
    }

    #[test]
    fn test_question_status_from_str() {
        assert_eq!("Pending".parse::<QuestionStatus>().unwrap(), QuestionStatus::Pending);
        assert_eq!("Answered".parse::<QuestionStatus>().unwrap(), QuestionStatus::Answered);
        assert_eq!("Dismissed".parse::<QuestionStatus>().unwrap(), QuestionStatus::Dismissed);
        assert_eq!("Expired".parse::<QuestionStatus>().unwrap(), QuestionStatus::Expired);
        assert!("invalid".parse::<QuestionStatus>().is_err());
    }

    #[test]
    fn test_question_serialization() {
        let q = Question {
            id: Uuid::new_v4(),
            agent_id: "dietician".to_string(),
            workflow_id: None,
            dispatch_id: None,
            category: Some("health".to_string()),
            question: "What did you eat yesterday?".to_string(),
            context: None,
            priority: QuestionPriority::Normal,
            status: QuestionStatus::Pending,
            answer: None,
            asked_at: Utc::now(),
            answered_at: None,
            expires_at: None,
        };

        let json = serde_json::to_string(&q).unwrap();
        let deserialized: Question = serde_json::from_str(&json).unwrap();
        assert_eq!(q.id, deserialized.id);
        assert_eq!(q.agent_id, deserialized.agent_id);
        assert_eq!(q.status, deserialized.status);
        assert_eq!(q.priority, deserialized.priority);
    }

    #[test]
    fn test_create_question_request_serialization() {
        let req = CreateQuestionRequest {
            agent_id: "test-agent".to_string(),
            workflow_id: None,
            dispatch_id: None,
            category: Some("deployment".to_string()),
            question: "Should I proceed with the deploy?".to_string(),
            context: Some("Staging environment is ready.".to_string()),
            priority: Some(QuestionPriority::High),
            expires_in_seconds: Some(3600),
        };

        let json = serde_json::to_string(&req).unwrap();
        let deserialized: CreateQuestionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.agent_id, deserialized.agent_id);
        assert_eq!(req.question, deserialized.question);
        assert_eq!(req.priority, deserialized.priority);
    }

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse::ok("agentd-ask", "0.1.0");
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.status, deserialized.status);
        assert_eq!(response.service, deserialized.service);
    }
}
