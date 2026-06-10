/**
 * Test data factory for Monitor service types.
 *
 * Usage:
 *   const metrics = makeSystemMetrics()
 *   const history = makeSystemMetricsHistory(12)
 *   const status = makeSystemStatus({ status: 'degraded' })
 */

import type {
	MonitorAlert,
	SystemMetrics,
	SystemStatus,
} from "@/types/monitor";

/** Base timestamp used for deterministic snapshot times */
const BASE_TIME_MS = Date.parse("2024-01-01T00:00:00Z");

// ---------------------------------------------------------------------------
// SystemMetrics factory
// ---------------------------------------------------------------------------

export function makeSystemMetrics(
	overrides?: Partial<SystemMetrics>,
): SystemMetrics {
	return {
		collected_at: "2024-01-01T00:00:00Z",
		cpu: {
			usage_percent: 42.5,
			core_count: 8,
			per_core: [40.0, 45.0, 41.0, 44.0, 42.0, 43.0, 40.5, 44.5],
		},
		memory: {
			total_bytes: 16_000_000_000,
			used_bytes: 8_000_000_000,
			available_bytes: 8_000_000_000,
			usage_percent: 50.0,
		},
		disks: [
			{
				name: "disk0",
				mount_point: "/",
				total_bytes: 500_000_000_000,
				available_bytes: 200_000_000_000,
				used_bytes: 300_000_000_000,
				usage_percent: 60.0,
			},
		],
		load_average: { one: 1.5, five: 1.2, fifteen: 0.9 },
		...overrides,
	};
}

/** Create a history of N snapshots spaced 30 seconds apart, oldest first */
export function makeSystemMetricsHistory(
	count: number,
	overrides?: Partial<SystemMetrics>,
): SystemMetrics[] {
	return Array.from({ length: count }, (_, i) =>
		makeSystemMetrics({
			collected_at: new Date(BASE_TIME_MS + i * 30_000).toISOString(),
			...overrides,
		}),
	);
}

// ---------------------------------------------------------------------------
// Alert / status factories
// ---------------------------------------------------------------------------

export function makeMonitorAlert(
	overrides?: Partial<MonitorAlert>,
): MonitorAlert {
	return {
		metric: "cpu",
		current_value: 95.0,
		threshold: 90.0,
		message: "CPU usage is critical: 95.0% (threshold: 90.0%)",
		raised_at: "2024-01-01T00:00:00Z",
		...overrides,
	};
}

export function makeSystemStatus(
	overrides?: Partial<SystemStatus>,
): SystemStatus {
	return {
		status: "healthy",
		metrics: makeSystemMetrics(),
		alerts: [],
		last_collected_at: "2024-01-01T00:00:00Z",
		...overrides,
	};
}
