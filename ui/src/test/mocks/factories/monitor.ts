/**
 * Test data factory for Monitor service types.
 *
 * Usage:
 *   const metrics = makeSystemMetrics()
 *   const history = makeSystemMetricsHistory(12)
 *   const status = makeSystemStatus({ status: 'degraded' })
 */

import type {
	MatrixSeries,
	MonitorAlert,
	NamedQuery,
	QueryResult,
	SystemMetrics,
	SystemStatus,
	VectorSample,
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

// ---------------------------------------------------------------------------
// Named-query factories (GET /queries, GET /queries/{name})
// ---------------------------------------------------------------------------

export function makeNamedQuery(overrides?: Partial<NamedQuery>): NamedQuery {
	return {
		name: "dispatch-success-rate",
		description:
			"Fraction of finished workflow dispatches that completed successfully",
		promql:
			'sum(increase(workflow_dispatches_total{status="completed"}[$__window]))',
		unit: "ratio",
		default_window: "1h",
		...overrides,
	};
}

/** A small representative catalog (subset of the server's 14 entries). */
export function makeQueryCatalog(): NamedQuery[] {
	return [
		makeNamedQuery(),
		makeNamedQuery({
			name: "agents-active",
			description: "Currently active agents",
			promql: "agents_active",
			unit: "count",
			default_window: "",
		}),
		makeNamedQuery({
			name: "session-cost",
			description: "Claude session spend over the window (USD, gauge delta)",
			promql: "delta(usage_session_cost_usd_total[$__window])",
			unit: "usd",
			default_window: "24h",
		}),
		makeNamedQuery({
			name: "http-p95-latency",
			description: "p95 HTTP request latency per service (seconds)",
			promql: "histogram_quantile(0.95, ...)",
			unit: "seconds",
			default_window: "15m",
		}),
	];
}

export function makeVectorSample(
	overrides?: Partial<VectorSample>,
): VectorSample {
	return {
		metric: { service: "orchestrator" },
		value: [1_704_067_200, "3"],
		...overrides,
	};
}

export function makeMatrixSeries(
	overrides?: Partial<MatrixSeries>,
): MatrixSeries {
	return {
		metric: { service: "orchestrator" },
		values: [
			[1_704_067_200, "1"],
			[1_704_067_260, "2"],
			[1_704_067_320, "3"],
		],
		...overrides,
	};
}

/** Instant-vector query result. */
export function makeVectorQueryResult(
	name = "agents-active",
	samples: VectorSample[] = [makeVectorSample()],
): QueryResult {
	return {
		name,
		promql: "agents_active",
		mode: "instant",
		executed_at: "2024-01-01T00:00:00Z",
		data: { resultType: "vector", result: samples },
	};
}

/** Range (matrix) query result. */
export function makeMatrixQueryResult(
	name = "dispatch-throughput",
	series: MatrixSeries[] = [makeMatrixSeries()],
): QueryResult {
	return {
		name,
		promql: "sum by (status) (increase(workflow_dispatches_total[1h]))",
		mode: "range",
		executed_at: "2024-01-01T00:00:00Z",
		data: { resultType: "matrix", result: series },
	};
}
