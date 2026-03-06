/**
 * useApprovals — global pending approval queue hook.
 *
 * Provides:
 * - All pending approvals fetched on mount with auto-refresh (default 10 s)
 * - approve / deny individual approvals (removes from local state immediately)
 * - bulkApprove / bulkDeny for multiple approvals at once
 * - Per-approval loading state to prevent double-submission
 * - Agent map (id → name) fetched alongside approvals for display
 * - Browser Notification API integration for new arrivals
 * - Optional filter by agentId
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import type { Agent, PendingApproval } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseApprovalsOptions {
  /** Auto-refresh interval in ms; 0 = disabled (default 10 000) */
  refreshInterval?: number
  /** Pause auto-refresh */
  paused?: boolean
  /** Filter displayed approvals to a specific agent */
  agentId?: string
  /** Whether to request browser notification permission and fire alerts */
  browserNotifications?: boolean
}

export interface UseApprovalsResult {
  /** All pending approvals (filtered by agentId if provided) */
  approvals: PendingApproval[]
  /** Total pending count across ALL agents (for badge display) */
  totalPendingCount: number
  loading: boolean
  error?: string
  /** Map of agentId → Agent for display purposes */
  agentMap: Map<string, Agent>
  /** IDs of approvals currently being actioned (loading state) */
  busyIds: Set<string>
  refetch: () => void
  approve: (id: string) => Promise<void>
  deny: (id: string) => Promise<void>
  bulkApprove: (ids: string[]) => Promise<void>
  bulkDeny: (ids: string[]) => Promise<void>
}

// ---------------------------------------------------------------------------
// Browser Notification helper
// ---------------------------------------------------------------------------

function requestNotificationPermission(): void {
  if (typeof Notification === 'undefined') return
  if (Notification.permission === 'default') {
    Notification.requestPermission().catch(() => {})
  }
}

function fireNotification(title: string, body: string): void {
  if (typeof Notification === 'undefined') return
  if (Notification.permission !== 'granted') return
  try {
    new Notification(title, { body, icon: '/favicon.ico' })
  } catch {
    // Some environments block Notification constructor
  }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_REFRESH = 10_000

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useApprovals({
  refreshInterval = DEFAULT_REFRESH,
  paused = false,
  agentId,
  browserNotifications = false,
}: UseApprovalsOptions = {}): UseApprovalsResult {
  const [allApprovals, setAllApprovals] = useState<PendingApproval[]>([])
  const [agentMap, setAgentMap] = useState<Map<string, Agent>>(new Map())
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set())

  // Track previously-seen IDs to detect new arrivals for notifications
  const seenIdsRef = useRef<Set<string>>(new Set())
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
        const [approvalsResult, agentsResult] = await Promise.all([
          orchestratorClient.listApprovals({ status: 'Pending', limit: 200 }),
          orchestratorClient.listAgents({ limit: 200 }),
        ])

        if (!mountedRef.current) return

        const incoming = approvalsResult.items

        // Detect new approvals for browser notifications
        if (browserNotifications && seenIdsRef.current.size > 0) {
          for (const a of incoming) {
            if (!seenIdsRef.current.has(a.id)) {
              fireNotification('Approval Required', `Tool "${a.tool_name}" is waiting for approval`)
            }
          }
        }
        seenIdsRef.current = new Set(incoming.map((a) => a.id))

        setAllApprovals(incoming)
        setAgentMap(new Map(agentsResult.items.map((ag) => [ag.id, ag])))
        setError(undefined)
      } catch (err) {
        if (!mountedRef.current) return
        setError(err instanceof Error ? err.message : 'Failed to load approvals')
      } finally {
        if (mountedRef.current) setLoading(false)
      }
    },
    [browserNotifications],
  )

  // ---------------------------------------------------------------------------
  // Mount / unmount
  // ---------------------------------------------------------------------------

  useEffect(() => {
    mountedRef.current = true
    fetchData(true)
    if (browserNotifications) requestNotificationPermission()
    return () => {
      mountedRef.current = false
    }
  }, [fetchData, browserNotifications])

  // ---------------------------------------------------------------------------
  // Auto-refresh
  // ---------------------------------------------------------------------------

  useEffect(() => {
    if (!refreshInterval || paused) return
    const timer = setInterval(() => fetchData(false), refreshInterval)
    return () => clearInterval(timer)
  }, [refreshInterval, paused, fetchData])

  // ---------------------------------------------------------------------------
  // Actions
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

  const removeApprovals = (ids: string[]) => {
    setAllApprovals((prev) => prev.filter((a) => !ids.includes(a.id)))
  }

  const approve = useCallback(async (id: string) => {
    setBusy([id], true)
    try {
      await orchestratorClient.approveRequest(id)
      removeApprovals([id])
    } finally {
      setBusy([id], false)
    }
  }, [])

  const deny = useCallback(async (id: string) => {
    setBusy([id], true)
    try {
      await orchestratorClient.denyRequest(id)
      removeApprovals([id])
    } finally {
      setBusy([id], false)
    }
  }, [])

  const bulkApprove = useCallback(async (ids: string[]) => {
    setBusy(ids, true)
    try {
      await Promise.allSettled(ids.map((id) => orchestratorClient.approveRequest(id)))
      removeApprovals(ids)
    } finally {
      setBusy(ids, false)
    }
  }, [])

  const bulkDeny = useCallback(async (ids: string[]) => {
    setBusy(ids, true)
    try {
      await Promise.allSettled(ids.map((id) => orchestratorClient.denyRequest(id)))
      removeApprovals(ids)
    } finally {
      setBusy(ids, false)
    }
  }, [])

  // ---------------------------------------------------------------------------
  // Derived state
  // ---------------------------------------------------------------------------

  const approvals = agentId ? allApprovals.filter((a) => a.agent_id === agentId) : allApprovals

  return {
    approvals,
    totalPendingCount: allApprovals.length,
    loading,
    error,
    agentMap,
    busyIds,
    refetch: () => fetchData(false),
    approve,
    deny,
    bulkApprove,
    bulkDeny,
  }
}
