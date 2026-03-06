//! Request and response types for the monitor service.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use agentd_common::types::HealthResponse;

/// A metric report submitted to or queried from the monitor service.
///
/// Captures a named numeric measurement at a point in time with an optional
/// unit label and arbitrary metadata tags.
///
/// # JSON Example
///
/// ```json
/// {
///   "name": "cpu.usage_percent",
///   "value": 42.5,
///   "unit": "percent",
///   "tags": { "host": "dev-machine" }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricReport {
    /// Metric name (e.g. `"cpu.usage_percent"`, `"memory.used_bytes"`)
    pub name: String,
    /// Numeric value of the metric
    pub value: f64,
    /// Optional unit label (e.g. `"percent"`, `"bytes"`, `"ms"`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// When the metric was observed; defaults to now if omitted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    /// Arbitrary key-value tags for grouping / filtering
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub tags: std::collections::HashMap<String, String>,
}

/// An active alert produced when a metric breaches a threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Unique alert identifier
    pub id: Uuid,
    /// Metric that triggered the alert
    pub metric: String,
    /// Current value of the metric
    pub current_value: f64,
    /// Threshold that was exceeded
    pub threshold: f64,
    /// Human-readable description
    pub message: String,
    /// When the alert was raised
    pub raised_at: DateTime<Utc>,
}

/// Severity level for an alert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Response from `GET /metrics`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    /// All currently held metric reports
    pub metrics: Vec<MetricReport>,
    /// Total count
    pub count: usize,
}

/// Response from `GET /alerts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsResponse {
    /// All currently active alerts
    pub alerts: Vec<Alert>,
    /// Total count
    pub count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_report_serialization() {
        let report = MetricReport {
            name: "cpu.usage_percent".to_string(),
            value: 42.5,
            unit: Some("percent".to_string()),
            observed_at: Some(Utc::now()),
            tags: std::collections::HashMap::new(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let decoded: MetricReport = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, "cpu.usage_percent");
        assert!((decoded.value - 42.5).abs() < 0.001);
    }

    #[test]
    fn test_metric_report_optional_fields_omitted() {
        let report = MetricReport {
            name: "disk.free".to_string(),
            value: 100.0,
            unit: None,
            observed_at: None,
            tags: std::collections::HashMap::new(),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("unit"), "unit should be omitted when None");
        assert!(!json.contains("observed_at"), "observed_at should be omitted when None");
    }

    #[test]
    fn test_alert_severity_serialization() {
        assert_eq!(serde_json::to_string(&AlertSeverity::Info).unwrap(), r#""info""#);
        assert_eq!(serde_json::to_string(&AlertSeverity::Warning).unwrap(), r#""warning""#);
        assert_eq!(serde_json::to_string(&AlertSeverity::Critical).unwrap(), r#""critical""#);
    }

    #[test]
    fn test_metrics_response_serialization() {
        let resp = MetricsResponse { metrics: vec![], count: 0 };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: MetricsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.count, 0);
        assert!(decoded.metrics.is_empty());
    }

    #[test]
    fn test_alerts_response_serialization() {
        let resp = AlertsResponse { alerts: vec![], count: 0 };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: AlertsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.count, 0);
    }

    #[test]
    fn test_health_response_creation() {
        let resp = HealthResponse::ok("agentd-monitor", "0.2.0");
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.service, "agentd-monitor");
    }
}
