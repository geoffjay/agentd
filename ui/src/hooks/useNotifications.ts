/**
 * useNotifications — full notification list management hook.
 *
 * Provides:
 * - Paginated notification list with filtering and sorting
 * - Actions: markViewed, respond, dismiss, delete, bulk operations
 * - markAllViewed convenience action
 * - Auto-refresh every 30 seconds (configurable)
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { notifyClient } from '@/services/notify'
import type {
  Notification,
  NotificationPriority,
  NotificationSource,
  NotificationStatus,
} from '@/types/notify'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type NotificationTab = 'all' | 'actionable' | 'history'
export type SortOrder = 'newest' | 'oldest' | 'priority'

export interface NotificationFilters {
  status?: NotificationStatus | 'All'
  priority?: NotificationPriority | 'All'
  source?: NotificationSource | 'All'
}

export interface UseNotificationsOptions {
  tab?: NotificationTab
  filters?: NotificationFilters
  sort?: SortOrder
  limit?: number
  /** Auto-refresh interval in ms; 0 = disabled (default 30 000) */
  refreshInterval?: number
  paused?: boolean
}

export interface UseNotificationsResult {
  notifications: Notification[]
  total: number
  loading: boolean
  error?: string
  /** IDs currently being processed */
  busyIds: Set<string>
  refetch: () => void
  markViewed: (id: string) => Promise<void>
  respond: (id: string, response: string) => Promise<void>
  dismiss: (id: string) => Promise<void>
  remove: (id: string) => Promise<void>
  bulkDismiss: (ids: string[]) => Promise<void>
  bulkDelete: (ids: string[]) => Promise<void>
  markAllViewed: () => Promise<void>
}

// ---------------------------------------------------------------------------
// Priority ordering
// ---------------------------------------------------------------------------

const PRIORITY_ORDER: Record<NotificationPriority, number> = {
  urgent: 4,
  high: 3,
  normal: 2,
  low: 1,
}

function sortNotifications(items: Notification[], sort: SortOrder): Notification[] {
  const arr = [...items]
  switch (sort) {
    case 'oldest':
      return arr.sort(
        (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      )
    case 'priority':
      return arr.sort(
        (a, b) =>
          (PRIORITY_ORDER[b.priority] ?? 0) - (PRIORITY_ORDER[a.priority] ?? 0) ||
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
      )
    case 'newest':
    default:
      return arr.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
      )
  }
}

function applyFilters(items: Notification[], filters: NotificationFilters): Notification[] {
  let result = items
  if (filters.status && filters.status !== 'All') {
    result = result.filter((n) => n.status === filters.status)
  }
  if (filters.priority && filters.priority !== 'All') {
    result = result.filter((n) => n.priority === filters.priority)
  }
  if (filters.source && filters.source !== 'All') {
    result = result.filter((n) => n.source === filters.source)
  }
  return result
}

const DEFAULT_REFRESH = 30_000

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useNotifications({
  tab = 'all',
  filters = {},
  sort = 'newest',
  limit = 200,
  refreshInterval = DEFAULT_REFRESH,
  paused = false,
}: UseNotificationsOptions = {}): UseNotificationsResult {
  const [allNotifications, setAllNotifications] = useState<Notification[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set())
  const mountedRef = useRef(true)

  // ---------------------------------------------------------------------------
  // Fetch
  // ---------------------------------------------------------------------------

  const fetchData = useCallback(
    async (showLoading = true) => {
      if (!mountedRef.current) return
      if (showLoading) {
        setLoading(true)
        setError(undefined)
      }
      try {
        let result: Awaited<ReturnType<typeof notifyClient.listNotifications>>
        const params = { limit }
        switch (tab) {
          case 'actionable':
            result = await notifyClient.listActionable(params)
            break
          case 'history':
            result = await notifyClient.listHistory(params)
            break
          default:
            result = await notifyClient.listNotifications(params)
        }
        if (!mountedRef.current) return
        setAllNotifications(result.items)
        setTotal(result.total)
        setError(undefined)
      } catch (err) {
        if (!mountedRef.current) return
        setError(err instanceof Error ? err.message : 'Failed to load notifications')
      } finally {
        if (mountedRef.current) setLoading(false)
      }
    },
    [tab, limit],
  )

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

  // ---------------------------------------------------------------------------
  // Derived state
  // ---------------------------------------------------------------------------

  const filtered = applyFilters(allNotifications, filters)
  const notifications = sortNotifications(filtered, sort)

  // ---------------------------------------------------------------------------
  // Busy state helpers
  // ---------------------------------------------------------------------------

  const setBusy = (ids: string[], busy: boolean) => {
    setBusyIds((prev) => {
      const next = new Set(prev)
      for (const id of ids) {
        if (busy) next.add(id)
        else next.delete(id)
      }
      return next
    })
  }

  const updateLocal = (id: string, patch: Partial<Notification>) => {
    setAllNotifications((prev) =>
      prev.map((n) => (n.id === id ? { ...n, ...patch } : n)),
    )
  }

  const removeLocal = (ids: string[]) => {
    setAllNotifications((prev) => prev.filter((n) => !ids.includes(n.id)))
  }

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  const markViewed = useCallback(async (id: string) => {
    setBusy([id], true)
    try {
      const updated = await notifyClient.updateNotification(id, { status: 'viewed' })
      updateLocal(id, { status: updated.status })
    } finally {
      setBusy([id], false)
    }
  }, [])

  const respond = useCallback(async (id: string, response: string) => {
    setBusy([id], true)
    try {
      const updated = await notifyClient.updateNotification(id, {
        status: 'responded',
        response,
      })
      updateLocal(id, { status: updated.status, response: updated.response })
    } finally {
      setBusy([id], false)
    }
  }, [])

  const dismiss = useCallback(async (id: string) => {
    setBusy([id], true)
    try {
      const updated = await notifyClient.updateNotification(id, { status: 'dismissed' })
      updateLocal(id, { status: updated.status })
    } finally {
      setBusy([id], false)
    }
  }, [])

  const remove = useCallback(async (id: string) => {
    setBusy([id], true)
    try {
      await notifyClient.deleteNotification(id)
      removeLocal([id])
    } finally {
      setBusy([id], false)
    }
  }, [])

  const bulkDismiss = useCallback(async (ids: string[]) => {
    setBusy(ids, true)
    try {
      await Promise.allSettled(
        ids.map((id) => notifyClient.updateNotification(id, { status: 'dismissed' })),
      )
      for (const id of ids) {
        updateLocal(id, { status: 'dismissed' })
      }
    } finally {
      setBusy(ids, false)
    }
  }, [])

  const bulkDelete = useCallback(async (ids: string[]) => {
    setBusy(ids, true)
    try {
      await Promise.allSettled(ids.map((id) => notifyClient.deleteNotification(id)))
      removeLocal(ids)
    } finally {
      setBusy(ids, false)
    }
  }, [])

  const markAllViewed = useCallback(async () => {
    const pendingIds = allNotifications
      .filter((n) => n.status === 'pending')
      .map((n) => n.id)
    if (pendingIds.length === 0) return
    setBusy(pendingIds, true)
    try {
      await Promise.allSettled(
        pendingIds.map((id) => notifyClient.updateNotification(id, { status: 'viewed' })),
      )
      for (const id of pendingIds) {
        updateLocal(id, { status: 'viewed' })
      }
    } finally {
      setBusy(pendingIds, false)
    }
  }, [allNotifications])

  return {
    notifications,
    total,
    loading,
    error,
    busyIds,
    refetch: () => fetchData(false),
    markViewed,
    respond,
    dismiss,
    remove,
    bulkDismiss,
    bulkDelete,
    markAllViewed,
  }
}
