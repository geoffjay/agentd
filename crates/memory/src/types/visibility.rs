//! [`VisibilityLevel`] enum for access-control on memory records.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Access-control tier for a memory record.
///
/// Controls who can read or search a memory. Defaults to
/// [`VisibilityLevel::Public`] when not specified.
///
/// # Three-tier Model
///
/// | Level     | Who can read                                      |
/// |-----------|---------------------------------------------------|
/// | `Public`  | Anyone (no actor required)                        |
/// | `Shared`  | Creator, owner, and actors listed in `shared_with`|
/// | `Private` | Creator and owner only                            |
///
/// # Serialization
///
/// Serializes as lowercase strings: `"public"`, `"shared"`, `"private"`.
///
/// # Examples
///
/// ```rust
/// use memory::types::VisibilityLevel;
///
/// assert_eq!(VisibilityLevel::default(), VisibilityLevel::Public);
///
/// let json = serde_json::to_string(&VisibilityLevel::Private).unwrap();
/// assert_eq!(json, "\"private\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VisibilityLevel {
    /// Readable only by the creator and the owner.
    Private,
    /// Readable by the creator, owner, and actors listed in `shared_with`.
    Shared,
    /// Readable by anyone, regardless of actor identity (default).
    #[default]
    Public,
}

impl fmt::Display for VisibilityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VisibilityLevel::Private => write!(f, "private"),
            VisibilityLevel::Shared => write!(f, "shared"),
            VisibilityLevel::Public => write!(f, "public"),
        }
    }
}

impl FromStr for VisibilityLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "private" => Ok(VisibilityLevel::Private),
            "shared" => Ok(VisibilityLevel::Shared),
            "public" => Ok(VisibilityLevel::Public),
            _ => Err(format!(
                "invalid visibility level: '{s}'. expected private, shared, or public"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_public() {
        assert_eq!(VisibilityLevel::default(), VisibilityLevel::Public);
    }

    #[test]
    fn test_display() {
        assert_eq!(VisibilityLevel::Private.to_string(), "private");
        assert_eq!(VisibilityLevel::Shared.to_string(), "shared");
        assert_eq!(VisibilityLevel::Public.to_string(), "public");
    }

    #[test]
    fn test_from_str_lowercase() {
        assert_eq!(VisibilityLevel::from_str("private").unwrap(), VisibilityLevel::Private);
        assert_eq!(VisibilityLevel::from_str("shared").unwrap(), VisibilityLevel::Shared);
        assert_eq!(VisibilityLevel::from_str("public").unwrap(), VisibilityLevel::Public);
    }

    #[test]
    fn test_from_str_case_insensitive() {
        assert_eq!(VisibilityLevel::from_str("PRIVATE").unwrap(), VisibilityLevel::Private);
        assert_eq!(VisibilityLevel::from_str("Shared").unwrap(), VisibilityLevel::Shared);
        assert_eq!(VisibilityLevel::from_str("PUBLIC").unwrap(), VisibilityLevel::Public);
    }

    #[test]
    fn test_from_str_invalid() {
        let err = VisibilityLevel::from_str("unknown").unwrap_err();
        assert!(err.contains("invalid visibility level"));
    }

    #[test]
    fn test_serde_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&VisibilityLevel::Private).unwrap(), "\"private\"");
        assert_eq!(serde_json::to_string(&VisibilityLevel::Shared).unwrap(), "\"shared\"");
        assert_eq!(serde_json::to_string(&VisibilityLevel::Public).unwrap(), "\"public\"");
    }

    #[test]
    fn test_serde_deserializes() {
        let v: VisibilityLevel = serde_json::from_str("\"private\"").unwrap();
        assert_eq!(v, VisibilityLevel::Private);

        let v: VisibilityLevel = serde_json::from_str("\"shared\"").unwrap();
        assert_eq!(v, VisibilityLevel::Shared);
    }

    #[test]
    fn test_roundtrip() {
        for variant in [VisibilityLevel::Private, VisibilityLevel::Shared, VisibilityLevel::Public]
        {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: VisibilityLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, variant);
        }
    }
}
