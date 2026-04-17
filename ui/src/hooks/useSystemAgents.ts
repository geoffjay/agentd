/**
 * useSystemAgents — hook for fetching and displaying built-in system agents.
 *
 * Fetches from `GET /system-agents`, auto-refreshes on the same interval as
 * the main agent list (default 10 seconds), and exposes a manual `refetch`.
 *
 * System agents are always present while the orchestrator is running.
 * This hook never exposes create/delete actions — those are not available
 * for built-in agents.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { orchestratorClient } from "@/services/orchestrator";
import type { Agent } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseSystemAgentsOptions {
	/** Pause auto-refresh (e.g. when a dialog is open). Default: false */
	paused?: boolean;
	/** Auto-refresh interval in milliseconds. Default: 10 000 */
	refreshInterval?: number;
}

export interface UseSystemAgentsResult {
	/** Built-in system agents returned by the API */
	agents: Agent[];
	/** True on initial load before any data arrives */
	loading: boolean;
	/** True during background refresh (data already loaded) */
	refreshing: boolean;
	/** Error message if the last fetch failed */
	error?: string;
	/** Trigger an immediate refresh */
	refetch: () => void;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useSystemAgents(
	options: UseSystemAgentsOptions = {},
): UseSystemAgentsResult {
	const { paused = false, refreshInterval = 10_000 } = options;

	const [agents, setAgents] = useState<Agent[]>([]);
	const [loading, setLoading] = useState(true);
	const [refreshing, setRefreshing] = useState(false);
	const [error, setError] = useState<string | undefined>();

	const firstLoad = useRef(true);
	const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

	const fetchAgents = useCallback(async () => {
		if (firstLoad.current) {
			setLoading(true);
		} else {
			setRefreshing(true);
		}

		try {
			const data = await orchestratorClient.listSystemAgents();
			setAgents(data);
			setError(undefined);
		} catch (err) {
			const msg = err instanceof Error ? err.message : "Unknown error";
			setError(`Failed to fetch system agents: ${msg}`);
		} finally {
			setLoading(false);
			setRefreshing(false);
			firstLoad.current = false;
		}
	}, []);

	// Schedule next refresh after current fetch settles.
	const scheduleNext = useCallback(() => {
		if (timerRef.current) clearTimeout(timerRef.current);
		timerRef.current = setTimeout(() => {
			if (!paused) fetchAgents().then(scheduleNext);
		}, refreshInterval);
	}, [paused, refreshInterval, fetchAgents]);

	// Initial fetch + start refresh loop.
	useEffect(() => {
		firstLoad.current = true;
		fetchAgents().then(scheduleNext);
		return () => {
			if (timerRef.current) clearTimeout(timerRef.current);
		};
	}, [fetchAgents, scheduleNext]);

	const refetch = useCallback(() => {
		if (timerRef.current) clearTimeout(timerRef.current);
		fetchAgents().then(scheduleNext);
	}, [fetchAgents, scheduleNext]);

	return { agents, loading, refreshing, error, refetch };
}
