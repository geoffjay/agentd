/**
 * SystemHealthPanel — grid of service health indicators with response times.
 *
 * Shows per-service:
 * - Status dot + name
 * - Port
 * - Response time (measured client-side from /health check)
 * - Uptime calculation from repeated successful checks
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { Activity, CheckCircle2, Clock, XCircle } from 'lucide-react'
import { orchestratorClient } from '@/services/orchestrator'
import { notifyClient } from '@/services/notify'
import { askClient } from '@/services/ask'
import type { ServiceMetricsData } from '@/hooks/useMetrics'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SystemHealthPanelProps {
  serviceMetrics: ServiceMetricsData[]
  loading?: boolean
}

interface ResponseTimeStat {
  key: string
  latestMs?: number
  successCount: number
  totalChecks: number
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const HEALTH_FETCHERS = {
  orchestrator: () => orchestratorClient.getHealth(),
  notify: () => notifyClient.getHealth(),
  ask: () => askClient.getHealth(),
}

function uptimePct(stat: ResponseTimeStat): number {
  if (stat.totalChecks === 0) return 0
  return Math.round((stat.successCount / stat.totalChecks) * 100)
}

function responseClass(ms?: number): string {
  if (ms === undefined) return 'text-gray-400 dark:text-gray-500'
  if (ms < 100) return 'text-green-500 dark:text-green-400'
  if (ms < 500) return 'text-yellow-500 dark:text-yellow-400'
  return 'text-red-500 dark:text-red-400'
}

// ---------------------------------------------------------------------------
// SystemHealthPanel
// ---------------------------------------------------------------------------

export function SystemHealthPanel({ serviceMetrics, loading }: SystemHealthPanelProps) {
  const [responseStats, setResponseStats] = useState<Record<string, ResponseTimeStat>>({})
  const statsRef = useRef<Record<string, ResponseTimeStat>>({})

  const measureResponseTimes = useCallback(async () => {
    const entries = Object.entries(HEALTH_FETCHERS)
    await Promise.allSettled(
      entries.map(async ([key, fetcher]) => {
        const start = performance.now()
        let success = false
        try {
          await fetcher()
          success = true
        } catch {
          // reachability failure
        }
        const ms = success ? Math.round(performance.now() - start) : undefined

        const prev = statsRef.current[key] ?? { key, successCount: 0, totalChecks: 0 }
        const next: ResponseTimeStat = {
          key,
          latestMs: success ? ms : undefined,
          successCount: prev.successCount + (success ? 1 : 0),
          totalChecks: prev.totalChecks + 1,
        }
        statsRef.current = { ...statsRef.current, [key]: next }
      }),
    )
    setResponseStats({ ...statsRef.current })
  }, [])

  useEffect(() => {
    void measureResponseTimes()
    const timer = setInterval(() => void measureResponseTimes(), 30_000)
    return () => clearInterval(timer)
  }, [measureResponseTimes])

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5">
      <div className="flex items-center gap-2 mb-4">
        <Activity size={16} className="text-primary-500" />
        <h3 className="text-sm font-semibold text-gray-900 dark:text-white">System Health</h3>
      </div>

      {loading ? (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          {[1, 2, 3].map((i) => (
            <div key={i} className="h-20 rounded-lg bg-gray-100 dark:bg-gray-700 animate-pulse" />
          ))}
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          {serviceMetrics.map((svc) => {
            const stat = responseStats[svc.key]
            const uptime = stat ? uptimePct(stat) : null

            return (
              <div
                key={svc.key}
                className={[
                  'rounded-lg border p-3 space-y-2',
                  svc.reachable
                    ? 'border-gray-100 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50'
                    : 'border-red-100 dark:border-red-900/30 bg-red-50/30 dark:bg-red-900/5',
                ].join(' ')}
              >
                {/* Service name + status */}
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-gray-900 dark:text-white">
                    {svc.name}
                  </span>
                  {svc.reachable ? (
                    <CheckCircle2 size={15} className="text-green-500 dark:text-green-400" />
                  ) : (
                    <XCircle size={15} className="text-red-500 dark:text-red-400" />
                  )}
                </div>

                {/* Port */}
                <p className="text-xs text-gray-400 dark:text-gray-500">Port {svc.port}</p>

                {/* Response time + uptime */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-1">
                    <Clock size={12} className="text-gray-400 dark:text-gray-500" />
                    <span className={`text-xs tabular-nums font-medium ${responseClass(stat?.latestMs)}`}>
                      {stat?.latestMs !== undefined ? `${stat.latestMs}ms` : '—'}
                    </span>
                  </div>
                  {uptime !== null && (
                    <span className={`text-xs tabular-nums ${uptime === 100 ? 'text-green-500 dark:text-green-400' : uptime > 90 ? 'text-yellow-500' : 'text-red-500'}`}>
                      {uptime}% up
                    </span>
                  )}
                </div>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}

export default SystemHealthPanel
