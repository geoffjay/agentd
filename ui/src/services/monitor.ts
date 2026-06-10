/**
 * Client for the Monitor service (default port 17003).
 *
 * Exposes system metrics (CPU, memory, disk, load average), the retained
 * metrics history ring buffer, and threshold-based health assessment.
 */

import type { HealthResponse } from "@/types/common";
import type {
	CollectResponse,
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
}

/** Singleton client instance using the configured service URL */
export const monitorClient = new MonitorClient({
	baseUrl: serviceConfig.monitorServiceUrl,
});
