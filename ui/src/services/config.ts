/**
 * Service configuration — all services are accessed through the UI proxy server.
 *
 * The UI server at `/api/<service>/**` proxies requests to the appropriate
 * backend service, eliminating port mismatch issues when running in production.
 */
export const serviceConfig = {
	askServiceUrl: "/api/ask",
	notifyServiceUrl: "/api/notify",
	orchestratorServiceUrl: "/api/orchestrator",
	memoryServiceUrl: "/api/memory",
	communicateServiceUrl: "/api/communicate",
} as const;

export type ServiceConfig = typeof serviceConfig;
