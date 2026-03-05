/**
 * useNotificationSummary — fetches notification count and priority breakdown.
 */

import { useCallback, useEffect, useState } from 'react'
import { notifyClient } from '@/services/notify'
import type { NotificationPriority } from '@/types/notify'

export interface NotificationPriorityCounts {
  Low: number
  Normal: number
  High: number
  Urgent: number
}

export interface UseNotificationSummaryResult {
  pending: number
  unread: number
  total: number
  priorityCounts: NotificationPriorityCounts
  loading: boolean
  error?: string
}

const EMPTY_PRIORITY: NotificationPriorityCounts = { Low: 0, Normal: 0, High: 0, Urgent: 0 }

export function useNotificationSummary(): UseNotificationSummaryResult {
  const [pending, setPending] = useState(0)
  const [unread, setUnread] = useState(0)
  const [total, setTotal] = useState(0)
  const [priorityCounts, setPriorityCounts] = useState<NotificationPriorityCounts>(EMPTY_PRIORITY)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const fetch = useCallback(async () => {
    setLoading(true)
    setError(undefined)
    try {
      const [actionable, countData] = await Promise.all([
        notifyClient.listActionable({ limit: 200 }),
        notifyClient.getCount(),
      ])

      // Pending = those still needing attention
      const pendingCount = actionable.items.filter((n) => n.status === 'Pending').length
      // Unread = Pending + Viewed (seen but not responded)
      const unreadCount = actionable.items.filter(
        (n) => n.status === 'Pending' || n.status === 'Viewed',
      ).length

      // Build priority breakdown from actionable set
      const pCounts: NotificationPriorityCounts = { Low: 0, Normal: 0, High: 0, Urgent: 0 }
      for (const n of actionable.items) {
        const p = n.priority as NotificationPriority
        if (p in pCounts) pCounts[p]++
      }

      setPending(pendingCount)
      setUnread(unreadCount)
      setTotal(countData.total)
      setPriorityCounts(pCounts)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load notifications')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void fetch()
  }, [fetch])

  return { pending, unread, total, priorityCounts, loading, error }
}
