/**
 * Service configuration with environment variable defaults
 */
export const serviceConfig = {
  askServiceUrl: import.meta.env.VITE_AGENTD_ASK_SERVICE_URL ?? 'http://localhost:17001',
  notifyServiceUrl: import.meta.env.VITE_AGENTD_NOTIFY_SERVICE_URL ?? 'http://localhost:17004',
  orchestratorServiceUrl: import.meta.env.VITE_AGENTD_ORCHESTRATOR_SERVICE_URL ?? 'http://localhost:17006',
} as const

export type ServiceConfig = typeof serviceConfig
