//! [`MemoryType`] enum for classifying stored memories.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The category of a memory record.
///
/// Controls how the memory is indexed and retrieved. Defaults to
/// [`MemoryType::Information`] when not specified.
///
/// # Serialization
///
/// Serializes as lowercase strings: `"question"`, `"request"`, `"information"`.
///
/// # Examples
///
/// ```rust
/// use memory::types::MemoryType;
///
/// assert_eq!(MemoryType::default(), MemoryType::Information);
///
/// let json = serde_json::to_string(&MemoryType::Question).unwrap();
/// assert_eq!(json, "\"question\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    /// A question that was asked by an agent or user.
    Question,
    /// A request or instruction that was issued.
    Request,
    /// General factual or contextual information (default).
    #[default]
    Information,
}

impl fmt::Display for MemoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryType::Question => write!(f, "question"),
            MemoryType::Request => write!(f, "request"),
            MemoryType::Information => write!(f, "information"),
        }
    }
}

impl FromStr for MemoryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "question" => Ok(MemoryType::Question),
            "request" => Ok(MemoryType::Request),
            "information" => Ok(MemoryType::Information),
            _ => Err(format!("invalid memory type: '{s}'. expected question, request, or information")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_information() {
        assert_eq!(MemoryType::default(), MemoryType::Information);
    }

    #[test]
    fn test_display() {
        assert_eq!(MemoryType::Question.to_string(), "question");
        assert_eq!(MemoryType::Request.to_string(), "request");
        assert_eq!(MemoryType::Information.to_string(), "information");
    }

    #[test]
    fn test_from_str_lowercase() {
        assert_eq!(MemoryType::from_str("question").unwrap(), MemoryType::Question);
        assert_eq!(MemoryType::from_str("request").unwrap(), MemoryType::Request);
        assert_eq!(MemoryType::from_str("information").unwrap(), MemoryType::Information);
    }

    #[test]
    fn test_from_str_case_insensitive() {
        assert_eq!(MemoryType::from_str("QUESTION").unwrap(), MemoryType::Question);
        assert_eq!(MemoryType::from_str("Request").unwrap(), MemoryType::Request);
        assert_eq!(MemoryType::from_str("INFORMATION").unwrap(), MemoryType::Information);
    }

    #[test]
    fn test_from_str_invalid() {
        let err = MemoryType::from_str("invalid").unwrap_err();
        assert!(err.contains("invalid memory type"));
    }

    #[test]
    fn test_serde_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&MemoryType::Question).unwrap(), "\"question\"");
        assert_eq!(serde_json::to_string(&MemoryType::Request).unwrap(), "\"request\"");
        assert_eq!(serde_json::to_string(&MemoryType::Information).unwrap(), "\"information\"");
    }

    #[test]
    fn test_serde_deserializes() {
        let t: MemoryType = serde_json::from_str("\"question\"").unwrap();
        assert_eq!(t, MemoryType::Question);

        let t: MemoryType = serde_json::from_str("\"request\"").unwrap();
        assert_eq!(t, MemoryType::Request);
    }

    #[test]
    fn test_roundtrip() {
        for variant in [MemoryType::Question, MemoryType::Request, MemoryType::Information] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: MemoryType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, variant);
        }
    }
}
