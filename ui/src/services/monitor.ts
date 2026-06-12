/**
 * Client for the Monitor service (default port 17003).
 *
 * Exposes system metrics (CPU, memory, disk, load average), the retained
 * metrics history ring buffer, and threshold-based health assessment.
 */

import type { HealthResponse } from "@/types/common";
import type {
	CollectResponse,
	NamedQuery,
	QueryResult,
	RunQueryOptions,
	SystemMetrics,
	SystemStatus,
} from "@/types/monitor";
import { ApiClient } from "./base";
import { serviceConfig } from "./config";

export class MonitorClient extends ApiClient {
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	getHealth(): Promise<HealthResponse> {
		return this.get<HealthResponse>("/health");
	}

	// -------------------------------------------------------------------------
	// Metrics
	// -------------------------------------------------------------------------

	/** `GET /metrics` — latest system metrics snapshot (503 if none yet). */
	getMetrics(): Promise<SystemMetrics> {
		return this.get<SystemMetrics>("/metrics");
	}

	/** `GET /history` — all retained metrics snapshots (ring buffer). */
	getHistory(): Promise<SystemMetrics[]> {
		return this.get<SystemMetrics[]>("/history");
	}

	/** `POST /collect` — trigger an immediate metrics collection. */
	collect(): Promise<CollectResponse> {
		return this.post<CollectResponse>("/collect");
	}

	// -------------------------------------------------------------------------
	// Status
	// -------------------------------------------------------------------------

	/** `GET /status` — current health assessment against thresholds. */
	getStatus(): Promise<SystemStatus> {
		return this.get<SystemStatus>("/status");
	}

	// -------------------------------------------------------------------------
	// Named Prometheus queries
	// -------------------------------------------------------------------------

	/**
	 * `GET /queries` — the curated PromQL catalog.
	 *
	 * Static on the server side: succeeds even when Prometheus is down.
	 */
	listQueries(): Promise<NamedQuery[]> {
		return this.get<NamedQuery[]>("/queries");
	}

	/**
	 * `GET /queries/{name}` — execute a named query against Prometheus.
	 *
	 * Fails with `ApiError(502)` when Prometheus itself is unreachable —
	 * distinct from the monitor being down (network error / `ApiError(0)`),
	 * so callers can render the right degraded state.
	 */
	runQuery(name: string, options?: RunQueryOptions): Promise<QueryResult> {
		return this.get<QueryResult>(`/queries/${encodeURIComponent(name)}`, {
			window: options?.window,
			mode: options?.mode,
			range_minutes: options?.rangeMinutes,
			step_secs: options?.stepSecs,
		});
	}
}

/** Singleton client instance using the configured service URL */
export const monitorClient = new MonitorClient({
	baseUrl: serviceConfig.monitorServiceUrl,
});
