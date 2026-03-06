//! Thread-safe application state for the monitor service.
//!
//! Maintains a ring buffer of metric snapshots and evaluates alerts against
//! configured thresholds. The [`AppState`] struct is cheaply cloneable and
//! safe to share across async tasks.

use crate::{
    config::MonitorConfig,
    types::{Alert, HealthStatus, SystemMetrics, SystemStatus},
};
use chrono::Utc;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// Thread-safe application state container.
///
/// Wraps a `RwLock` protected inner state. Cloning is cheap — it only clones
/// the `Arc` pointer.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<AppStateInner>>,
}

struct AppStateInner {
    /// Ring buffer of collected metric snapshots (oldest first)
    metrics_history: VecDeque<SystemMetrics>,
    /// Maximum number of snapshots to retain
    history_size: usize,
    /// Service configuration (thresholds etc.)
    config: MonitorConfig,
}

impl AppState {
    /// Create a new state container with the given configuration.
    pub fn new(config: MonitorConfig) -> Self {
        let history_size = config.history_size;
        Self {
            inner: Arc::new(RwLock::new(AppStateInner {
                metrics_history: VecDeque::with_capacity(history_size),
                history_size,
                config,
            })),
        }
    }

    /// Push a new metrics snapshot into the ring buffer.
    ///
    /// If the buffer is at capacity the oldest snapshot is discarded.
    pub async fn push_metrics(&self, metrics: SystemMetrics) {
        let mut state = self.inner.write().await;
        if state.metrics_history.len() >= state.history_size {
            state.metrics_history.pop_front();
        }
        debug!("Stored metrics snapshot at {}", metrics.collected_at);
        state.metrics_history.push_back(metrics);
    }

    /// Return the most recent metrics snapshot, if any.
    pub async fn latest_metrics(&self) -> Option<SystemMetrics> {
        let state = self.inner.read().await;
        state.metrics_history.back().cloned()
    }

    /// Return all retained metrics snapshots (oldest first).
    pub async fn all_metrics(&self) -> Vec<SystemMetrics> {
        let state = self.inner.read().await;
        state.metrics_history.iter().cloned().collect()
    }

    /// Return the number of snapshots currently held.
    pub async fn metrics_count(&self) -> usize {
        let state = self.inner.read().await;
        state.metrics_history.len()
    }

    /// Evaluate the latest metrics against configured thresholds and return a
    /// [`SystemStatus`] describing the current health.
    pub async fn evaluate_status(&self) -> SystemStatus {
        let state = self.inner.read().await;
        let latest = state.metrics_history.back().cloned();

        let Some(metrics) = latest else {
            return SystemStatus {
                status: HealthStatus::Healthy,
                metrics: None,
                alerts: vec![],
                last_collected_at: None,
            };
        };

        let mut alerts = Vec::new();
        let now = Utc::now();

        // CPU threshold check
        if metrics.cpu.usage_percent >= state.config.cpu_alert_threshold {
            alerts.push(Alert {
                metric: "cpu".to_string(),
                current_value: metrics.cpu.usage_percent,
                threshold: state.config.cpu_alert_threshold,
                message: format!(
                    "CPU usage is critical: {:.1}% (threshold: {:.1}%)",
                    metrics.cpu.usage_percent, state.config.cpu_alert_threshold
                ),
                raised_at: now,
            });
        }

        // Memory threshold check
        if metrics.memory.usage_percent >= state.config.memory_alert_threshold {
            alerts.push(Alert {
                metric: "memory".to_string(),
                current_value: metrics.memory.usage_percent,
                threshold: state.config.memory_alert_threshold,
                message: format!(
                    "Memory usage is critical: {:.1}% (threshold: {:.1}%)",
                    metrics.memory.usage_percent, state.config.memory_alert_threshold
                ),
                raised_at: now,
            });
        }

        // Disk threshold checks
        for disk in &metrics.disks {
            if disk.usage_percent >= state.config.disk_alert_threshold {
                alerts.push(Alert {
                    metric: format!("disk:{}", disk.mount_point),
                    current_value: disk.usage_percent,
                    threshold: state.config.disk_alert_threshold,
                    message: format!(
                        "Disk {} ({}) usage is critical: {:.1}% (threshold: {:.1}%)",
                        disk.name,
                        disk.mount_point,
                        disk.usage_percent,
                        state.config.disk_alert_threshold
                    ),
                    raised_at: now,
                });
            }
        }

        let status = if alerts.is_empty() {
            HealthStatus::Healthy
        } else if alerts.len() == 1 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Critical
        };

        let last_collected_at = Some(metrics.collected_at);
        SystemStatus { status, metrics: Some(metrics), alerts, last_collected_at }
    }

    /// Return a clone of the current configuration.
    pub async fn config(&self) -> MonitorConfig {
        let state = self.inner.read().await;
        state.config.clone()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(MonitorConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CpuMetrics, DiskMetrics, LoadAverage, MemoryMetrics};

    fn make_metrics(cpu_pct: f32, mem_pct: f32, disk_pct: f32) -> SystemMetrics {
        let total_mem = 16_000_000_000u64;
        let used_mem = (total_mem as f32 * mem_pct / 100.0) as u64;
        let total_disk = 500_000_000_000u64;
        let used_disk = (total_disk as f32 * disk_pct / 100.0) as u64;

        SystemMetrics {
            collected_at: Utc::now(),
            cpu: CpuMetrics { usage_percent: cpu_pct, core_count: 4, per_core: vec![cpu_pct; 4] },
            memory: MemoryMetrics {
                total_bytes: total_mem,
                used_bytes: used_mem,
                available_bytes: total_mem - used_mem,
                usage_percent: mem_pct,
            },
            disks: vec![DiskMetrics {
                name: "disk0".to_string(),
                mount_point: "/".to_string(),
                total_bytes: total_disk,
                available_bytes: total_disk - used_disk,
                used_bytes: used_disk,
                usage_percent: disk_pct,
            }],
            load_average: LoadAverage { one: 1.0, five: 0.8, fifteen: 0.6 },
        }
    }

    #[tokio::test]
    async fn test_initial_state_has_no_metrics() {
        let state = AppState::default();
        assert!(state.latest_metrics().await.is_none());
        assert_eq!(state.metrics_count().await, 0);
    }

    #[tokio::test]
    async fn test_push_and_retrieve_metrics() {
        let state = AppState::default();
        let metrics = make_metrics(50.0, 50.0, 50.0);
        let expected_at = metrics.collected_at;

        state.push_metrics(metrics).await;

        let latest = state.latest_metrics().await.unwrap();
        assert_eq!(latest.collected_at, expected_at);
        assert_eq!(state.metrics_count().await, 1);
    }

    #[tokio::test]
    async fn test_ring_buffer_evicts_oldest() {
        let mut config = MonitorConfig::default();
        config.history_size = 3;
        let state = AppState::new(config);

        for i in 0..5u32 {
            let mut m = make_metrics(i as f32, 50.0, 50.0);
            m.cpu.usage_percent = i as f32;
            state.push_metrics(m).await;
        }

        assert_eq!(state.metrics_count().await, 3);
        let all = state.all_metrics().await;
        // Most recent 3 should be retained (cpu 2, 3, 4)
        assert!((all[0].cpu.usage_percent - 2.0).abs() < 0.01);
        assert!((all[2].cpu.usage_percent - 4.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_healthy_status_when_below_thresholds() {
        let state = AppState::default();
        state.push_metrics(make_metrics(50.0, 50.0, 50.0)).await;

        let status = state.evaluate_status().await;
        assert_eq!(status.status, HealthStatus::Healthy);
        assert!(status.alerts.is_empty());
    }

    #[tokio::test]
    async fn test_degraded_on_single_threshold_breach() {
        let mut config = MonitorConfig::default();
        config.cpu_alert_threshold = 80.0;
        let state = AppState::new(config);
        state.push_metrics(make_metrics(95.0, 50.0, 50.0)).await;

        let status = state.evaluate_status().await;
        assert_eq!(status.status, HealthStatus::Degraded);
        assert_eq!(status.alerts.len(), 1);
        assert_eq!(status.alerts[0].metric, "cpu");
    }

    #[tokio::test]
    async fn test_critical_on_multiple_threshold_breaches() {
        let mut config = MonitorConfig::default();
        config.cpu_alert_threshold = 80.0;
        config.memory_alert_threshold = 80.0;
        let state = AppState::new(config);
        state.push_metrics(make_metrics(95.0, 95.0, 50.0)).await;

        let status = state.evaluate_status().await;
        assert_eq!(status.status, HealthStatus::Critical);
        assert!(status.alerts.len() >= 2);
    }

    #[tokio::test]
    async fn test_healthy_status_when_no_metrics() {
        let state = AppState::default();
        let status = state.evaluate_status().await;
        assert_eq!(status.status, HealthStatus::Healthy);
        assert!(status.metrics.is_none());
    }

    #[tokio::test]
    async fn test_all_metrics_returns_history_in_order() {
        let state = AppState::default();
        for _ in 0..3 {
            state.push_metrics(make_metrics(50.0, 50.0, 50.0)).await;
        }
        let all = state.all_metrics().await;
        assert_eq!(all.len(), 3);
    }
}
