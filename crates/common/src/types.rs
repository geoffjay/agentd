//! Shared API types used across agentd services.
//!
//! - [`PaginatedResponse<T>`] — generic paginated list response envelope
//! - [`clamp_limit()`] — pagination limit clamping utility
//! - [`DEFAULT_PAGE_LIMIT`], [`MAX_PAGE_LIMIT`] — pagination constants

use serde::{Deserialize, Serialize};

/// Paginated response envelope for list endpoints.
///
/// Used by all services that return paginated lists (notify, orchestrator).
///
/// # Examples
///
/// ```rust
/// use agentd_common::types::PaginatedResponse;
///
/// let response = PaginatedResponse {
///     items: vec!["a", "b", "c"],
///     total: 10,
///     limit: 3,
///     offset: 0,
/// };
/// assert_eq!(response.items.len(), 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

/// Default page size for list endpoints.
pub const DEFAULT_PAGE_LIMIT: usize = 50;

/// Maximum page size for list endpoints.
pub const MAX_PAGE_LIMIT: usize = 200;

/// Clamp a requested pagination limit to valid bounds.
///
/// Returns `DEFAULT_PAGE_LIMIT` if `None`, otherwise clamps to `[1, MAX_PAGE_LIMIT]`.
///
/// # Examples
///
/// ```rust
/// use agentd_common::types::clamp_limit;
///
/// assert_eq!(clamp_limit(None), 50);
/// assert_eq!(clamp_limit(Some(10)), 10);
/// assert_eq!(clamp_limit(Some(0)), 1);
/// assert_eq!(clamp_limit(Some(999)), 200);
/// ```
pub fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_limit_default() {
        assert_eq!(clamp_limit(None), DEFAULT_PAGE_LIMIT);
    }

    #[test]
    fn test_clamp_limit_within_bounds() {
        assert_eq!(clamp_limit(Some(10)), 10);
        assert_eq!(clamp_limit(Some(100)), 100);
    }

    #[test]
    fn test_clamp_limit_below_minimum() {
        assert_eq!(clamp_limit(Some(0)), 1);
    }

    #[test]
    fn test_clamp_limit_above_maximum() {
        assert_eq!(clamp_limit(Some(500)), MAX_PAGE_LIMIT);
    }

    #[test]
    fn test_paginated_response_serde() {
        let response = PaginatedResponse { items: vec![1, 2, 3], total: 10, limit: 3, offset: 0 };
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: PaginatedResponse<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.items, vec![1, 2, 3]);
        assert_eq!(deserialized.total, 10);
    }
}
