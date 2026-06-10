/**
 * useSystemMetrics — polls the monitor service for the system metrics
 * history (time-series) and the current threshold-based health status.
 *
 * Resilient to the monitor service being down: `available` flips to false
 * and the dashboard renders a quiet empty state instead of crashing.
 */

import { useCallback, useRef, useState } from "react";
import { monitorClient } from "@/services/monitor";
import type {
	MonitorAlert,
	SystemHealthStatus,
	SystemMetrics,
} from "@/types/monitor";
import { usePolling } from "./usePolling";

export interface UseSystemMetricsResult {
	/** Retained metrics snapshots, oldest first */
	history: SystemMetrics[];
	/** Most recent snapshot (null when none collected yet) */
	latest: SystemMetrics | null;
	/** Active threshold alerts */
	alerts: MonitorAlert[];
	/** Overall health assessment (null when unavailable) */
	status: SystemHealthStatus | null;
	/** False when the monitor service is unreachable */
	available: boolean;
	/** True only during the very first load */
	loading: boolean;
	error?: string;
	refetch: () => void;
}

export function useSystemMetrics(): UseSystemMetricsResult {
	const [history, setHistory] = useState<SystemMetrics[]>([]);
	const [latest, setLatest] = useState<SystemMetrics | null>(null);
	const [alerts, setAlerts] = useState<MonitorAlert[]>([]);
	const [status, setStatus] = useState<SystemHealthStatus | null>(null);
	const [available, setAvailable] = useState(true);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | undefined>();
	const hasLoadedRef = useRef(false);

	const fetch = useCallback(async () => {
		if (!hasLoadedRef.current) setLoading(true);
		const [historyRes, statusRes] = await Promise.allSettled([
			monitorClient.getHistory(),
			monitorClient.getStatus(),
		]);

		if (historyRes.status === "rejected" && statusRes.status === "rejected") {
			setAvailable(false);
			setError("Monitor service unavailable");
			setHistory([]);
			setLatest(null);
			setAlerts([]);
			setStatus(null);
		} else {
			const newHistory =
				historyRes.status === "fulfilled" ? historyRes.value : [];
			const newStatus =
				statusRes.status === "fulfilled" ? statusRes.value : null;

			setAvailable(true);
			setError(undefined);
			setHistory(newHistory);
			setLatest(
				newStatus?.metrics ?? newHistory[newHistory.length - 1] ?? null,
			);
			setAlerts(newStatus?.alerts ?? []);
			setStatus(newStatus?.status ?? null);
		}

		hasLoadedRef.current = true;
		setLoading(false);
	}, []);

	usePolling(fetch);

	const refetch = useCallback(() => {
		void fetch();
	}, [fetch]);

	return {
		history,
		latest,
		alerts,
		status,
		available,
		loading,
		error,
		refetch,
	};
}
