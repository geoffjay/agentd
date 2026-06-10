/**
 * TypeScript types for the Monitor service.
 * Mirrors the Rust types in crates/monitor/src/types.rs.
 */

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

/** CPU utilisation metrics */
export interface CpuMetrics {
	/** Global CPU usage as a percentage (0.0-100.0) */
	usage_percent: number;
	/** Number of logical CPU cores */
	core_count: number;
	/** Per-core usage percentages */
	per_core: number[];
}

/** Memory usage metrics */
export interface MemoryMetrics {
	total_bytes: number;
	used_bytes: number;
	available_bytes: number;
	/** Memory usage as a percentage (0.0-100.0) */
	usage_percent: number;
}

/** Disk usage metrics for a single mount point */
export interface DiskMetrics {
	name: string;
	mount_point: string;
	total_bytes: number;
	available_bytes: number;
	used_bytes: number;
	/** Usage as a percentage (0.0-100.0) */
	usage_percent: number;
}

/** System load averages over 1, 5, and 15 minute windows */
export interface LoadAverage {
	one: number;
	five: number;
	fifteen: number;
}

/** A complete snapshot of system metrics at a point in time */
export interface SystemMetrics {
	/** When these metrics were collected (ISO 8601) */
	collected_at: string;
	cpu: CpuMetrics;
	memory: MemoryMetrics;
	disks: DiskMetrics[];
	load_average: LoadAverage;
}

// ---------------------------------------------------------------------------
// Status / alerts
// ---------------------------------------------------------------------------

/** Overall system health status derived from threshold checks */
export type SystemHealthStatus = "healthy" | "degraded" | "critical";

/** An alert triggered by a threshold breach */
export interface MonitorAlert {
	/** Metric that triggered the alert (e.g. "cpu", "memory", "disk:/") */
	metric: string;
	current_value: number;
	threshold: number;
	message: string;
	/** When the alert was raised (ISO 8601) */
	raised_at: string;
}

/** Health assessment response from GET /status */
export interface SystemStatus {
	status: SystemHealthStatus;
	/** Latest metrics snapshot (null if no collection has occurred) */
	metrics: SystemMetrics | null;
	alerts: MonitorAlert[];
	/** Timestamp of the last successful collection (ISO 8601) */
	last_collected_at: string | null;
}

/** Response from POST /collect */
export interface CollectResponse {
	metrics: SystemMetrics;
	alerts: MonitorAlert[];
}
