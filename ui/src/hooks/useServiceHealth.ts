/**
 * useServiceHealth — polls all service health endpoints in parallel.
 *
 * Uses a stale-while-revalidate pattern: returns the last known data
 * immediately while refreshing in the background every 30 seconds
 * (paused while the tab is hidden, via the shared usePolling helper).
 */

import { useCallback, useState } from "react";
import type { ServiceStatus } from "@/components/common/StatusBadge";
import { askClient } from "@/services/ask";
import { communicateClient } from "@/services/communicate";
import { memoryClient } from "@/services/memory";
import { monitorClient } from "@/services/monitor";
import { notifyClient } from "@/services/notify";
import { orchestratorClient } from "@/services/orchestrator";
import type { HealthResponse } from "@/types/common";
import { usePolling } from "./usePolling";

export interface ServiceHealth {
	name: string;
	key: "orchestrator" | "notify" | "ask" | "memory" | "communicate" | "monitor";
	port: number;
	status: ServiceStatus;
	version?: string;
	lastChecked?: Date;
	error?: string;
}

export interface UseServiceHealthResult {
	services: ServiceHealth[];
	loading: boolean;
	/** True only on the very first load (no cached data yet) */
	initializing: boolean;
	refresh: () => void;
}

async function fetchHealth(
	key: ServiceHealth["key"],
	fetcher: () => Promise<HealthResponse>,
	port: number,
): Promise<ServiceHealth> {
	const base: Pick<ServiceHealth, "name" | "key" | "port"> = {
		name: key.charAt(0).toUpperCase() + key.slice(1),
		key,
		port,
	};
	try {
		const data = await fetcher();
		return {
			...base,
			status:
				data.status === "ok" || data.status === "healthy"
					? "healthy"
					: "degraded",
			version: data.version,
			lastChecked: new Date(),
		};
	} catch {
		return {
			...base,
			status: "down",
			lastChecked: new Date(),
			error: "Service unreachable",
		};
	}
}

export function useServiceHealth(): UseServiceHealthResult {
	const [services, setServices] = useState<ServiceHealth[]>([]);
	const [loading, setLoading] = useState(false);
	const [initializing, setInitializing] = useState(true);

	const fetch = useCallback(async () => {
		setLoading(true);
		const results = await Promise.all([
			fetchHealth("orchestrator", () => orchestratorClient.getHealth(), 17006),
			fetchHealth("notify", () => notifyClient.getHealth(), 17004),
			fetchHealth("ask", () => askClient.getHealth(), 17001),
			fetchHealth("memory", () => memoryClient.getHealth(), 17008),
			fetchHealth("communicate", () => communicateClient.getHealth(), 17010),
			fetchHealth("monitor", () => monitorClient.getHealth(), 17003),
		]);
		setServices(results);
		setLoading(false);
		setInitializing(false);
	}, []);

	usePolling(fetch);

	const refresh = useCallback(() => {
		void fetch();
	}, [fetch]);

	return { services, loading, initializing, refresh };
}
