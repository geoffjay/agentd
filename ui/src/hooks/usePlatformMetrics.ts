/**
 * usePlatformMetrics — polls the monitor service's curated named-query API
 * (Prometheus-backed) for platform-level metrics: dispatch health, HTTP
 * error rate and latency per service, approvals backlog, session cost,
 * and agent/connection counts.
 *
 * Degrades in two distinct ways so the page can say what actually broke:
 * - `monitorDown`    — the monitor service itself is unreachable.
 * - `prometheusDown` — the monitor answered but Prometheus is down (502).
 */

import { useCallback, useRef, useState } from "react";
import { monitorClient } from "@/services/monitor";
import { ApiError } from "@/types/common";
import type { QueryResult, VectorSample } from "@/types/monitor";
import { usePolling } from "./usePolling";

// ---------------------------------------------------------------------------
// Queried catalog entries
// ---------------------------------------------------------------------------

/** Named queries this hook polls each cycle. */
export const PLATFORM_QUERIES = [
	"dispatch-success-rate",
	"dispatch-throughput",
	"agent-restart-rate",
	"approvals-backlog",
	"http-error-rate",
	"http-p95-latency",
	"session-cost",
	"agents-active",
	"websocket-connections",
] as const;

export type PlatformQueryName = (typeof PLATFORM_QUERIES)[number];

export type PlatformQueryResults = Partial<
	Record<PlatformQueryName, QueryResult>
>;

export interface UsePlatformMetricsResult {
	/** Latest result per query; absent while loading or on per-query failure */
	results: PlatformQueryResults;
	/** True when the monitor service itself is unreachable */
	monitorDown: boolean;
	/** True when the monitor answered but Prometheus is down (HTTP 502) */
	prometheusDown: boolean;
	/** True only during the very first load */
	loading: boolean;
	refetch: () => void;
}

// ---------------------------------------------------------------------------
// Vector helpers (exported for the chart components)
// ---------------------------------------------------------------------------

/** All samples of an instant-vector result as { labels, value } entries. */
export function vectorEntries(
	result: QueryResult | undefined,
): Array<{ labels: Record<string, string>; value: number }> {
	if (!result || result.data.resultType !== "vector") return [];
	return result.data.result.map((sample: VectorSample) => ({
		labels: sample.metric,
		value: Number.parseFloat(sample.value[1]),
	}));
}

/**
 * The single scalar value of an instant-vector result (sum across samples),
 * or null when there is no data.
 */
export function vectorValue(result: QueryResult | undefined): number | null {
	const entries = vectorEntries(result);
	if (entries.length === 0) return null;
	return entries.reduce((acc, e) => acc + e.value, 0);
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function usePlatformMetrics(
	intervalMs?: number,
): UsePlatformMetricsResult {
	const [results, setResults] = useState<PlatformQueryResults>({});
	const [monitorDown, setMonitorDown] = useState(false);
	const [prometheusDown, setPrometheusDown] = useState(false);
	const [loading, setLoading] = useState(true);
	const hasLoadedRef = useRef(false);

	const fetch = useCallback(async () => {
		if (!hasLoadedRef.current) setLoading(true);

		const settled = await Promise.allSettled(
			PLATFORM_QUERIES.map((name) => monitorClient.runQuery(name)),
		);

		const next: PlatformQueryResults = {};
		let sawBadGateway = false;
		let sawSuccess = false;

		settled.forEach((outcome, i) => {
			const name = PLATFORM_QUERIES[i];
			if (outcome.status === "fulfilled") {
				next[name] = outcome.value;
				sawSuccess = true;
			} else if (
				outcome.reason instanceof ApiError &&
				outcome.reason.status === 502
			) {
				sawBadGateway = true;
			}
		});

		setResults(next);
		setPrometheusDown(sawBadGateway);
		// Only "nothing answered and nothing was a 502" means the monitor
		// itself is gone — a 502 proves the monitor responded.
		setMonitorDown(!sawSuccess && !sawBadGateway);

		hasLoadedRef.current = true;
		setLoading(false);
	}, []);

	usePolling(fetch, intervalMs);

	const refetch = useCallback(() => {
		void fetch();
	}, [fetch]);

	return { results, monitorDown, prometheusDown, loading, refetch };
}
