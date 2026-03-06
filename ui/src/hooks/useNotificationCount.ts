/**
 * useNotificationCount — fetches the pending notification count.
 *
 * Auto-refreshes every 15 seconds (configurable). Provides:
 * - pending: number of Pending notifications
 * - unread: Pending + Viewed
 * - total: all notifications
 * - count: raw CountResponse for detailed breakdown
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { notifyClient } from '@/services/notify'
import type { CountResponse } from '@/types/notify'

export interface UseNotificationCountOptions {
  /** Auto-refresh interval in ms; 0 = disabled (default 15 000) */
  refreshInterval?: number
  /** Pause auto-refresh */
  paused?: boolean
}

export interface UseNotificationCountResult {
  /** Notifications with status Pending */
  pending: number
  /** Pending + Viewed */
  unread: number
  /** Total notifications of all statuses */
  total: number
  /** Raw count response */
  count?: CountResponse
  loading: boolean
  error?: string
  refetch: () => void
}

const DEFAULT_REFRESH = 15_000

export function useNotificationCount({
  refreshInterval = DEFAULT_REFRESH,
  paused = false,
}: UseNotificationCountOptions = {}): UseNotificationCountResult {
  const [count, setCount] = useState<CountResponse | undefined>()
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()
  const mountedRef = useRef(true)

  const fetchData = useCallback(async (showLoading = true) => {
    if (!mountedRef.current) return
    if (showLoading) {
      setLoading(true)
      setError(undefined)
    }
    try {
      const data = await notifyClient.getCount()
      if (!mountedRef.current) return
      setCount(data)
      setError(undefined)
    } catch (err) {
      if (!mountedRef.current) return
      setError(err instanceof Error ? err.message : 'Failed to load notification count')
    } finally {
      if (mountedRef.current) setLoading(false)
    }
  }, [])

  useEffect(() => {
    mountedRef.current = true
    void fetchData(true)
    return () => {
      mountedRef.current = false
    }
  }, [fetchData])

  useEffect(() => {
    if (!refreshInterval || paused) return
    const timer = setInterval(() => fetchData(false), refreshInterval)
    return () => clearInterval(timer)
  }, [refreshInterval, paused, fetchData])

  // Derive counts from by_status breakdown
  const pending =
    count?.by_status.find((s) => s.status === 'Pending')?.count ?? 0
  const viewed =
    count?.by_status.find((s) => s.status === 'Viewed')?.count ?? 0
  const unread = pending + viewed

  return {
    pending,
    unread,
    total: count?.total ?? 0,
    count,
    loading,
    error,
    refetch: () => fetchData(false),
  }
}
