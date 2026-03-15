/**
 * useServiceHealth — polls all three service health endpoints in parallel.
 *
 * Uses a stale-while-revalidate pattern: returns the last known data
 * immediately while refreshing in the background every 30 seconds.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import { notifyClient } from '@/services/notify'
import { askClient } from '@/services/ask'
import { memoryClient } from '@/services/memory'
import type { HealthResponse } from '@/types/common'
import type { ServiceStatus } from '@/components/common/StatusBadge'

export interface ServiceHealth {
  name: string
  key: 'orchestrator' | 'notify' | 'ask' | 'memory'
  port: number
  status: ServiceStatus
  version?: string
  lastChecked?: Date
  error?: string
}

export interface UseServiceHealthResult {
  services: ServiceHealth[]
  loading: boolean
  /** True only on the very first load (no cached data yet) */
  initializing: boolean
  refresh: () => void
}

const REFRESH_INTERVAL_MS = 30_000

async function fetchHealth(
  key: ServiceHealth['key'],
  fetcher: () => Promise<HealthResponse>,
  port: number,
): Promise<ServiceHealth> {
  const base: Pick<ServiceHealth, 'name' | 'key' | 'port'> = {
    name: key.charAt(0).toUpperCase() + key.slice(1),
    key,
    port,
  }
  try {
    const data = await fetcher()
    return {
      ...base,
      status: data.status === 'ok' || data.status === 'healthy' ? 'healthy' : 'degraded',
      version: data.version,
      lastChecked: new Date(),
    }
  } catch {
    return {
      ...base,
      status: 'down',
      lastChecked: new Date(),
      error: 'Service unreachable',
    }
  }
}

export function useServiceHealth(): UseServiceHealthResult {
  const [services, setServices] = useState<ServiceHealth[]>([])
  const [loading, setLoading] = useState(false)
  const [initializing, setInitializing] = useState(true)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetch = useCallback(async () => {
    setLoading(true)
    const results = await Promise.all([
      fetchHealth('orchestrator', () => orchestratorClient.getHealth(), 17006),
      fetchHealth('notify', () => notifyClient.getHealth(), 17004),
      fetchHealth('ask', () => askClient.getHealth(), 17001),
      fetchHealth('memory', () => memoryClient.getHealth(), 17008),
    ])
    setServices(results)
    setLoading(false)
    setInitializing(false)
  }, [])

  const refresh = useCallback(() => {
    void fetch()
  }, [fetch])

  useEffect(() => {
    void fetch()
    intervalRef.current = setInterval(() => {
      void fetch()
    }, REFRESH_INTERVAL_MS)
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current)
    }
  }, [fetch])

  return { services, loading, initializing, refresh }
}
