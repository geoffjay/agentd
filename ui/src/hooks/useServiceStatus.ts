/**
 * useServiceStatus — wraps useServiceHealth and derives a single global
 * status signal for use in the header and banners.
 *
 * Returns:
 * - services: individual service health entries
 * - globalStatus: 'healthy' | 'degraded' | 'down'
 * - isServiceDown(key): whether a specific service is unreachable
 * - refresh: trigger an immediate refresh
 */

import { useServiceHealth } from './useServiceHealth'
import type { ServiceHealth } from './useServiceHealth'
import type { ServiceStatus } from '@/components/common/StatusBadge'

export type GlobalStatus = 'healthy' | 'degraded' | 'down'

export interface UseServiceStatusResult {
  services: ServiceHealth[]
  globalStatus: GlobalStatus
  loading: boolean
  initializing: boolean
  isServiceDown: (key: ServiceHealth['key']) => boolean
  isServiceDegraded: (key: ServiceHealth['key']) => boolean
  refresh: () => void
}

function computeGlobalStatus(services: ServiceHealth[]): GlobalStatus {
  if (services.length === 0) return 'healthy'
  const statuses = services.map((s) => s.status)
  if (statuses.every((s) => s === 'healthy')) return 'healthy'
  if (statuses.some((s) => s === 'down')) return 'down'
  return 'degraded'
}

export function useServiceStatus(): UseServiceStatusResult {
  const { services, loading, initializing, refresh } = useServiceHealth()

  const globalStatus = computeGlobalStatus(services)

  const getStatus = (key: ServiceHealth['key']): ServiceStatus =>
    services.find((s) => s.key === key)?.status ?? 'unknown'

  return {
    services,
    globalStatus,
    loading,
    initializing,
    isServiceDown: (key) => getStatus(key) === 'down',
    isServiceDegraded: (key) => {
      const s = getStatus(key)
      return s === 'down' || s === 'degraded'
    },
    refresh,
  }
}
