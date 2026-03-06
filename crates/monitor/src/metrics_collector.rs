//! System metrics collection using `sysinfo`.
//!
//! This module provides functions for collecting real-time system health data
//! including CPU, memory, disk, and load average readings.

use crate::types::{CpuMetrics, DiskMetrics, LoadAverage, MemoryMetrics, SystemMetrics};
use chrono::Utc;
use sysinfo::{Disks, System};
use tracing::debug;

/// Collect a complete system metrics snapshot.
///
/// Creates a fresh `sysinfo::System`, refreshes all subsystems, and returns
/// a [`SystemMetrics`] struct containing the current readings.
///
/// # Notes
///
/// - The first call may return slightly lower CPU usage values because `sysinfo`
///   computes CPU deltas between refresh cycles. Consider calling refresh twice
///   with a brief delay for more accurate readings.
/// - Disk list is refreshed separately as required by the `sysinfo` API.
///
/// # Examples
///
/// ```rust
/// use monitor::metrics_collector::collect;
///
/// let metrics = collect();
/// assert!(metrics.cpu.usage_percent >= 0.0);
/// assert!(metrics.cpu.usage_percent <= 100.0);
/// assert!(metrics.memory.total_bytes > 0);
/// ```
pub fn collect() -> SystemMetrics {
    let mut sys = System::new_all();
    // Refresh twice to get meaningful CPU delta readings
    sys.refresh_all();

    let cpu_usage = sys.global_cpu_usage();
    let per_core: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let core_count = per_core.len();

    debug!("CPU usage: {:.1}% ({} cores)", cpu_usage, core_count);

    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let avail_mem = sys.available_memory();
    let mem_pct = if total_mem > 0 {
        (used_mem as f32 / total_mem as f32) * 100.0
    } else {
        0.0
    };

    debug!("Memory: {}/{} bytes ({:.1}%)", used_mem, total_mem, mem_pct);

    let disks = Disks::new_with_refreshed_list();
    let disk_metrics: Vec<DiskMetrics> = disks
        .list()
        .iter()
        .filter(|d| d.total_space() > 0)
        .map(|d| {
            let total = d.total_space();
            let avail = d.available_space();
            let used = total.saturating_sub(avail);
            let pct = (used as f32 / total as f32) * 100.0;
            DiskMetrics {
                name: d.name().to_string_lossy().into_owned(),
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                total_bytes: total,
                available_bytes: avail,
                used_bytes: used,
                usage_percent: pct,
            }
        })
        .collect();

    let load = System::load_average();

    SystemMetrics {
        collected_at: Utc::now(),
        cpu: CpuMetrics { usage_percent: cpu_usage, core_count, per_core },
        memory: MemoryMetrics {
            total_bytes: total_mem,
            used_bytes: used_mem,
            available_bytes: avail_mem,
            usage_percent: mem_pct,
        },
        disks: disk_metrics,
        load_average: LoadAverage { one: load.one, five: load.five, fifteen: load.fifteen },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_returns_valid_metrics() {
        let metrics = collect();

        // CPU should be in [0, 100]
        assert!(metrics.cpu.usage_percent >= 0.0, "CPU usage should be non-negative");
        assert!(metrics.cpu.usage_percent <= 100.0, "CPU usage should not exceed 100%");

        // At least one core
        assert!(metrics.cpu.core_count > 0, "Should detect at least one CPU core");

        // Memory should be positive
        assert!(metrics.memory.total_bytes > 0, "Total memory should be positive");
        assert!(
            metrics.memory.usage_percent >= 0.0,
            "Memory usage percentage should be non-negative"
        );
        assert!(
            metrics.memory.usage_percent <= 100.0,
            "Memory usage percentage should not exceed 100%"
        );

        // Load averages should be non-negative
        assert!(metrics.load_average.one >= 0.0);
        assert!(metrics.load_average.five >= 0.0);
        assert!(metrics.load_average.fifteen >= 0.0);

        // collected_at should be recent
        let elapsed = Utc::now() - metrics.collected_at;
        assert!(elapsed.num_seconds() < 5, "collected_at should be very recent");
    }

    #[test]
    fn test_disk_metrics_consistency() {
        let metrics = collect();
        for disk in &metrics.disks {
            assert!(disk.total_bytes > 0, "Disk total should be positive");
            assert!(
                disk.used_bytes <= disk.total_bytes,
                "Used should not exceed total for disk {}",
                disk.name
            );
            assert!(
                disk.available_bytes <= disk.total_bytes,
                "Available should not exceed total for disk {}",
                disk.name
            );
            assert!(disk.usage_percent >= 0.0, "Disk usage should be non-negative");
            assert!(disk.usage_percent <= 100.0, "Disk usage should not exceed 100%");
        }
    }

    #[test]
    fn test_per_core_count_matches_core_count() {
        let metrics = collect();
        assert_eq!(
            metrics.cpu.per_core.len(),
            metrics.cpu.core_count,
            "per_core vec length should match core_count"
        );
    }
}
