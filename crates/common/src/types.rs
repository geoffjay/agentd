//! Shared API types used across agentd services.
//!
//! - [`PaginatedResponse<T>`] — generic paginated list response envelope
//! - [`clamp_limit()`] — pagination limit clamping utility
//! - [`DEFAULT_PAGE_LIMIT`], [`MAX_PAGE_LIMIT`] — pagination constants

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Service status value indicating all systems are operational.
///
/// Used by [`HealthResponse::ok`] and [`HealthResponse::is_healthy`].
/// Exposed as `pub` so downstream crates (CLI, integration tests) can
/// reference the constant rather than comparing against a bare `"ok"` literal.
pub const STATUS_OK: &str = "ok";

/// Service status value indicating the service is running but degraded.
///
/// Used by [`HealthResponse::degraded`].
/// Exposed as `pub` for the same reason as [`STATUS_OK`].
pub const STATUS_DEGRADED: &str = "degraded";

/// Standard health check response returned by all agentd services.
///
/// Provides a uniform schema for health endpoints, with service-specific
/// details in an extensible `details` map.
///
/// # Examples
///
/// ```rust
/// use agentd_common::types::HealthResponse;
///
/// let resp = HealthResponse::ok("agentd-wrap", "0.1.0");
/// assert_eq!(resp.status, "ok");
/// assert_eq!(resp.service, "agentd-wrap");
/// assert!(resp.details.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status: `"ok"` or `"degraded"`.
    pub status: String,
    /// Service name (e.g., `"agentd-notify"`).
    pub service: String,
    /// Crate version from `Cargo.toml`.
    pub version: String,
    /// Service-specific details (optional, varies per service).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, serde_json::Value>,
}

impl HealthResponse {
    /// Create a standard "ok" health response for a service.
    ///
    /// The `version` is populated at compile time from the *calling* crate's
    /// `CARGO_PKG_VERSION`. Since this is a library function, callers should
    /// pass their own version:
    ///
    /// ```rust,ignore
    /// HealthResponse::ok("agentd-notify", env!("CARGO_PKG_VERSION"))
    /// ```
    pub fn ok(service: &str, version: &str) -> Self {
        Self {
            status: STATUS_OK.to_string(),
            service: service.to_string(),
            version: version.to_string(),
            details: HashMap::new(),
        }
    }

    /// Create a "degraded" health response for a service.
    ///
    /// Use this when the service is running but one or more dependencies are
    /// unavailable or returning errors.  The `reason` should briefly describe
    /// what is degraded (e.g., `"database unreachable"`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agentd_common::types::HealthResponse;
    ///
    /// let resp = HealthResponse::degraded("agentd-notify", "0.2.0", "database unreachable");
    /// assert_eq!(resp.status, "degraded");
    /// assert!(!resp.is_healthy());
    /// ```
    pub fn degraded(service: &str, version: &str, reason: &str) -> Self {
        Self {
            status: STATUS_DEGRADED.to_string(),
            service: service.to_string(),
            version: version.to_string(),
            details: HashMap::new(),
        }
        .with_detail("reason", serde_json::Value::String(reason.to_string()))
    }

    /// Returns `true` if the service status is `"ok"`.
    ///
    /// Convenience predicate for callers that need to branch on health without
    /// comparing strings directly.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agentd_common::types::HealthResponse;
    ///
    /// assert!(HealthResponse::ok("svc", "1.0").is_healthy());
    /// assert!(!HealthResponse::degraded("svc", "1.0", "db down").is_healthy());
    /// ```
    pub fn is_healthy(&self) -> bool {
        self.status == STATUS_OK
    }

    /// Add a service-specific detail to the response.
    pub fn with_detail(mut self, key: &str, value: serde_json::Value) -> Self {
        self.details.insert(key.to_string(), value);
        self
    }
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

    #[test]
    fn test_health_response_ok() {
        let resp = HealthResponse::ok("agentd-test", "1.0.0");
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.service, "agentd-test");
        assert_eq!(resp.version, "1.0.0");
        assert!(resp.details.is_empty());
    }

    #[test]
    fn test_health_response_with_details() {
        let resp = HealthResponse::ok("agentd-test", "1.0.0")
            .with_detail("agents_active", serde_json::json!(5));
        assert_eq!(resp.details.len(), 1);
        assert_eq!(resp.details["agents_active"], serde_json::json!(5));
    }

    #[test]
    fn test_health_response_serde_no_details() {
        let resp = HealthResponse::ok("svc", "1.0");
        let json = serde_json::to_string(&resp).unwrap();
        // details should be omitted when empty
        assert!(!json.contains("details"));
        let deserialized: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, "ok");
    }

    #[test]
    fn test_health_response_serde_with_details() {
        let resp = HealthResponse::ok("svc", "1.0").with_detail("count", serde_json::json!(42));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"count\":42"));
    }

    #[test]
    fn test_health_response_is_healthy_ok() {
        let resp = HealthResponse::ok("agentd-test", "1.0.0");
        assert!(resp.is_healthy());
    }

    #[test]
    fn test_health_response_is_healthy_degraded() {
        let resp = HealthResponse::degraded("agentd-test", "1.0.0", "db down");
        assert!(!resp.is_healthy());
    }

    #[test]
    fn test_health_response_degraded_status() {
        let resp = HealthResponse::degraded("agentd-test", "1.0.0", "cache unavailable");
        assert_eq!(resp.status, "degraded");
        assert_eq!(resp.service, "agentd-test");
        assert_eq!(resp.version, "1.0.0");
    }

    #[test]
    fn test_health_response_degraded_includes_reason() {
        let resp = HealthResponse::degraded("agentd-test", "1.0.0", "redis timeout");
        assert_eq!(resp.details["reason"], serde_json::json!("redis timeout"));
    }

    #[test]
    fn test_health_response_degraded_serde() {
        let resp = HealthResponse::degraded("svc", "1.0", "db down");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"degraded\""));
        assert!(json.contains("\"db down\""));
        let deserialized: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, "degraded");
        assert!(!deserialized.is_healthy());
    }
}
