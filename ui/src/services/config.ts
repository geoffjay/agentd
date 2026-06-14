/**
 * Service configuration.
 *
 * Resolution order per service:
 * 1. Runtime config fetched from the hosting agentd-ui service
 *    (`/config.json`, see `../runtime-config.ts`) — set before this module
 *    evaluates via the `main.tsx` bootstrap.
 * 2. Build-time `VITE_*` env vars (local development).
 * 3. Compiled dev-port defaults.
 */
import { runtimeServiceUrl } from "../runtime-config";

export const serviceConfig = {
	coreServiceUrl:
		runtimeServiceUrl("core") ??
		import.meta.env.VITE_AGENTD_CORE_SERVICE_URL ??
		"http://localhost:17000",
	askServiceUrl:
		runtimeServiceUrl("ask") ??
		import.meta.env.VITE_AGENTD_ASK_SERVICE_URL ??
		"http://localhost:17001",
	notifyServiceUrl:
		runtimeServiceUrl("notify") ??
		import.meta.env.VITE_AGENTD_NOTIFY_SERVICE_URL ??
		"http://localhost:17004",
	orchestratorServiceUrl:
		runtimeServiceUrl("orchestrator") ??
		import.meta.env.VITE_AGENTD_ORCHESTRATOR_SERVICE_URL ??
		"http://localhost:17006",
	memoryServiceUrl:
		runtimeServiceUrl("memory") ??
		import.meta.env.VITE_AGENTD_MEMORY_SERVICE_URL ??
		"http://localhost:17008",
	monitorServiceUrl:
		runtimeServiceUrl("monitor") ??
		import.meta.env.VITE_AGENTD_MONITOR_SERVICE_URL ??
		"http://localhost:17003",
	communicateServiceUrl:
		runtimeServiceUrl("communicate") ??
		import.meta.env.VITE_AGENTD_COMMUNICATE_SERVICE_URL ??
		"http://localhost:17010",
} as const;

export type ServiceConfig = typeof serviceConfig;
