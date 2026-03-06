//! Request and response types for the monitor service.
//!
//! This module defines all data structures used in API requests and responses,
//! including system metrics, thresholds, and alert information.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use agentd_common::types::HealthResponse;

/// A complete snapshot of system metrics at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// When these metrics were collected
    pub collected_at: DateTime<Utc>,
    /// CPU utilisation metrics
    pub cpu: CpuMetrics,
    /// Memory usage metrics
    pub memory: MemoryMetrics,
    /// Per-disk usage metrics
    pub disks: Vec<DiskMetrics>,
    /// System load averages (1, 5, 15 minutes)
    pub load_average: LoadAverage,
}

/// CPU utilisation metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    /// Global CPU usage as a percentage (0.0–100.0)
    pub usage_percent: f32,
    /// Number of logical CPU cores
    pub core_count: usize,
    /// Per-core usage percentages
    pub per_core: Vec<f32>,
}

/// Memory usage metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Total physical memory in bytes
    pub total_bytes: u64,
    /// Used memory in bytes
    pub used_bytes: u64,
    /// Available memory in bytes
    pub available_bytes: u64,
    /// Memory usage as a percentage (0.0–100.0)
    pub usage_percent: f32,
}

/// Disk usage metrics for a single mount point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    /// Disk device name or label
    pub name: String,
    /// Mount point path
    pub mount_point: String,
    /// Total disk space in bytes
    pub total_bytes: u64,
    /// Available (free) space in bytes
    pub available_bytes: u64,
    /// Used space in bytes
    pub used_bytes: u64,
    /// Usage as a percentage (0.0–100.0)
    pub usage_percent: f32,
}

/// System load averages over 1, 5, and 15 minute windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadAverage {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

/// Overall system health status derived from threshold checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// All metrics within acceptable thresholds
    Healthy,
    /// One or more metrics approaching or exceeding thresholds
    Degraded,
    /// Critical condition — immediate attention required
    Critical,
}

/// Health assessment response from the `/status` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    /// Overall health status
    pub status: HealthStatus,
    /// Latest metrics snapshot (may be None if no collection has occurred)
    pub metrics: Option<SystemMetrics>,
    /// List of active alerts
    pub alerts: Vec<Alert>,
    /// Timestamp of the last successful collection
    pub last_collected_at: Option<DateTime<Utc>>,
}

/// An alert triggered by a threshold breach.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Metric that triggered the alert (e.g., "cpu", "memory", "disk:/")
    pub metric: String,
    /// Current value of the metric
    pub current_value: f32,
    /// Threshold that was exceeded
    pub threshold: f32,
    /// Human-readable description
    pub message: String,
    /// When the alert was raised
    pub raised_at: DateTime<Utc>,
}

/// Response from the `/collect` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectResponse {
    /// The freshly collected metrics
    pub metrics: SystemMetrics,
    /// Any alerts triggered by this collection
    pub alerts: Vec<Alert>,
}

/// Request body for the `/analyze` endpoint (log analysis via BAML).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    /// Name of the service whose logs are being analyzed
    pub service_name: String,
    /// Log lines to analyze
    pub log_entries: Vec<String>,
    /// Human-readable description of the time window (e.g., "last 5 minutes")
    pub time_window: String,
}

/// Response from the `/analyze` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeResponse {
    /// Whether analysis completed successfully
    pub success: bool,
    /// Raw analysis result as JSON value (schema depends on BAML output)
    pub result: serde_json::Value,
    /// Human-readable summary
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_metrics_serialization() {
        let metrics = SystemMetrics {
            collected_at: Utc::now(),
            cpu: CpuMetrics { usage_percent: 42.5, core_count: 8, per_core: vec![40.0, 45.0] },
            memory: MemoryMetrics {
                total_bytes: 16_000_000_000,
                used_bytes: 8_000_000_000,
                available_bytes: 8_000_000_000,
                usage_percent: 50.0,
            },
            disks: vec![DiskMetrics {
                name: "disk0".to_string(),
                mount_point: "/".to_string(),
                total_bytes: 500_000_000_000,
                available_bytes: 200_000_000_000,
                used_bytes: 300_000_000_000,
                usage_percent: 60.0,
            }],
            load_average: LoadAverage { one: 1.5, five: 1.2, fifteen: 0.9 },
        };

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: SystemMetrics = serde_json::from_str(&json).unwrap();
        assert!((deserialized.cpu.usage_percent - 42.5).abs() < 0.01);
        assert_eq!(deserialized.cpu.core_count, 8);
        assert_eq!(deserialized.disks.len(), 1);
    }

    #[test]
    fn test_health_status_serialization() {
        let status = HealthStatus::Healthy;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""healthy""#);

        let status = HealthStatus::Degraded;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""degraded""#);

        let status = HealthStatus::Critical;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""critical""#);
    }

    #[test]
    fn test_alert_serialization() {
        let alert = Alert {
            metric: "cpu".to_string(),
            current_value: 95.0,
            threshold: 90.0,
            message: "CPU usage is critical: 95.0% (threshold: 90.0%)".to_string(),
            raised_at: Utc::now(),
        };

        let json = serde_json::to_string(&alert).unwrap();
        let deserialized: Alert = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.metric, "cpu");
        assert!((deserialized.current_value - 95.0).abs() < 0.01);
    }

    #[test]
    fn test_health_response_creation() {
        let resp = HealthResponse::ok("agentd-monitor", "0.2.0");
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.service, "agentd-monitor");
    }
}
