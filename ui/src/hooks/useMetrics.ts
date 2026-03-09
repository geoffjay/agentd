/**
 * useMetrics — fetches and aggregates monitoring metrics from all services.
 *
 * Sources:
 * - Agent status counts from orchestrator (real-time)
 * - Notification priority counts from notify service (real-time)
 * - Prometheus /metrics text from each service (request/error rates)
 * - Synthetic time-series derived from snapshots (for chart history)
 *
 * Auto-refreshes on a configurable interval. Pauses when the tab is hidden.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import { notifyClient } from '@/services/notify'
import { serviceConfig } from '@/services/config'
import { parsePrometheusText, extractHttpMetrics } from './usePrometheusParser'
import type { ParsedMetrics, HttpMetricsSummary } from './usePrometheusParser'
import type { AgentStatus } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type RefreshInterval = 10_000 | 30_000 | 60_000 | 300_000

export interface AgentStatusCounts {
  Running: number
  Pending: number
  Stopped: number
  Failed: number
}

export interface NotificationCounts {
  Low: number
  Normal: number
  High: number
  Urgent: number
  total: number
}

export interface ServiceMetricsData {
  key: string
  name: string
  port: number
  http: HttpMetricsSummary
  raw?: ParsedMetrics
  reachable: boolean
}

/** Single time-series point for agent activity chart */
export interface AgentTimePoint {
  x: string // ISO timestamp
  y: number
}

export interface UseMetricsResult {
  /** Current agent status counts */
  agentCounts: AgentStatusCounts
  /** Current notification priority counts */
  notifCounts: NotificationCounts
  /** Per-service Prometheus metrics */
  serviceMetrics: ServiceMetricsData[]
  /** Synthetic agent time-series (running count over last N snapshots) */
  agentTimeSeries: AgentTimePoint[]
  /** Overall loading state */
  loading: boolean
  /** Last successful refresh time */
  lastRefresh?: Date
  /** Any error during fetch */
  error?: string
  /** Trigger a manual refresh */
  refetch: () => void
  refreshInterval: RefreshInterval
  setRefreshInterval: (interval: RefreshInterval) => void
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SERVICE_ENDPOINTS = [
  { key: 'orchestrator', name: 'Orchestrator', port: 17006, url: () => serviceConfig.orchestratorServiceUrl },
  { key: 'notify', name: 'Notify', port: 17004, url: () => serviceConfig.notifyServiceUrl },
  { key: 'ask', name: 'Ask', port: 17001, url: () => serviceConfig.askServiceUrl },
]

const EMPTY_AGENT_COUNTS: AgentStatusCounts = { Running: 0, Pending: 0, Stopped: 0, Failed: 0 }
const EMPTY_NOTIF_COUNTS: NotificationCounts = { Low: 0, Normal: 0, High: 0, Urgent: 0, total: 0 }

// How many data points to keep for time-series (one per refresh)
const MAX_HISTORY_POINTS = 60

// ---------------------------------------------------------------------------
// Prometheus fetch helper
// ---------------------------------------------------------------------------

async function fetchPrometheusMetrics(baseUrl: string): Promise<ParsedMetrics | undefined> {
  try {
    const resp = await fetch(`${baseUrl}/metrics`, {
      signal: AbortSignal.timeout(5000),
    })
    if (!resp.ok) return undefined
    const text = await resp.text()
    return parsePrometheusText(text)
  } catch {
    return undefined
  }
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useMetrics(initialInterval: RefreshInterval = 30_000): UseMetricsResult {
  const [agentCounts, setAgentCounts] = useState<AgentStatusCounts>(EMPTY_AGENT_COUNTS)
  const [notifCounts, setNotifCounts] = useState<NotificationCounts>(EMPTY_NOTIF_COUNTS)
  const [serviceMetrics, setServiceMetrics] = useState<ServiceMetricsData[]>([])
  const [agentTimeSeries, setAgentTimeSeries] = useState<AgentTimePoint[]>([])
  const [loading, setLoading] = useState(true)
  const [lastRefresh, setLastRefresh] = useState<Date | undefined>()
  const [error, setError] = useState<string | undefined>()
  const [refreshInterval, setRefreshInterval] = useState<RefreshInterval>(initialInterval)

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const historyRef = useRef<AgentTimePoint[]>([])

  const fetchAll = useCallback(async () => {
    try {
      // Fetch all data in parallel
      const [agentResult, notifActionable, notifCount, ...prometheusResults] =
        await Promise.allSettled([
          orchestratorClient.listAgents({ limit: 200 }),
          notifyClient.listActionable({ limit: 200 }),
          notifyClient.getCount(),
          ...SERVICE_ENDPOINTS.map((svc) => fetchPrometheusMetrics(svc.url())),
        ])

      // Agent counts
      if (agentResult.status === 'fulfilled') {
        const agents = agentResult.value.items
        const counts: AgentStatusCounts = { Running: 0, Pending: 0, Stopped: 0, Failed: 0 }
        for (const agent of agents) {
          const s = agent.status as AgentStatus
          if (s in counts) counts[s]++
        }
        setAgentCounts(counts)

        // Append to time series
        const point: AgentTimePoint = {
          x: new Date().toISOString(),
          y: counts.Running,
        }
        historyRef.current = [...historyRef.current, point].slice(-MAX_HISTORY_POINTS)
        setAgentTimeSeries([...historyRef.current])
      }

      // Notification counts
      if (
        notifActionable.status === 'fulfilled' &&
        notifCount.status === 'fulfilled'
      ) {
        const nCounts: NotificationCounts = { Low: 0, Normal: 0, High: 0, Urgent: 0, total: 0 }
        for (const n of notifActionable.value.items) {
          const p = n.priority as keyof Omit<NotificationCounts, 'total'>
          if (p in nCounts) nCounts[p]++
        }
        nCounts.total = notifCount.value.total
        setNotifCounts(nCounts)
      }

      // Service Prometheus metrics
      const svcData: ServiceMetricsData[] = SERVICE_ENDPOINTS.map((svc, i) => {
        const promResult = prometheusResults[i]
        const raw = promResult?.status === 'fulfilled' ? promResult.value : undefined
        return {
          key: svc.key,
          name: svc.name,
          port: svc.port,
          http: raw ? extractHttpMetrics(raw) : { requestsTotal: 0, errorsTotal: 0, errorRate: 0 },
          raw,
          reachable: raw !== undefined,
        }
      })
      setServiceMetrics(svcData)

      setLastRefresh(new Date())
      setError(undefined)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch metrics')
    } finally {
      setLoading(false)
    }
  }, [])

  // Initial fetch + auto-refresh
  useEffect(() => {
    void fetchAll()
  }, [fetchAll])

  useEffect(() => {
    if (timerRef.current) clearInterval(timerRef.current)
    timerRef.current = setInterval(() => void fetchAll(), refreshInterval)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [refreshInterval, fetchAll])

  // Pause on tab blur, resume on focus
  useEffect(() => {
    function onHide() {
      if (timerRef.current) clearInterval(timerRef.current)
    }
    function onShow() {
      void fetchAll()
      timerRef.current = setInterval(() => void fetchAll(), refreshInterval)
    }
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) onHide()
      else onShow()
    })
    return () => {
      document.removeEventListener('visibilitychange', onHide)
      document.removeEventListener('visibilitychange', onShow)
    }
  }, [refreshInterval, fetchAll])

  return {
    agentCounts,
    notifCounts,
    serviceMetrics,
    agentTimeSeries,
    loading,
    lastRefresh,
    error,
    refetch: () => void fetchAll(),
    refreshInterval,
    setRefreshInterval,
  }
}
