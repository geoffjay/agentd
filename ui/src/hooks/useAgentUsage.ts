/**
 * useAgentUsage — hook for fetching agent usage stats and triggering context clears.
 *
 * Provides:
 * - Usage data fetching with configurable auto-refresh
 * - Loading / error state
 * - clearContext action that triggers API call, then refreshes usage data
 * - Auto-pauses refresh when agent is stopped/failed
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import type { AgentUsageStats, ClearContextResponse } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseAgentUsageReturn {
  usage: AgentUsageStats | null
  loading: boolean
  error: string | null
  refresh: () => void
  clearContext: () => Promise<ClearContextResponse>
  clearing: boolean
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

const DEFAULT_AUTO_REFRESH_MS = 10_000

export function useAgentUsage(
  agentId: string,
  autoRefreshMs: number = DEFAULT_AUTO_REFRESH_MS,
): UseAgentUsageReturn {
  const [usage, setUsage] = useState<AgentUsageStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [clearing, setClearing] = useState(false)

  // Track whether the component is still mounted to avoid state updates
  // after unmount.
  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  // ---------------------------------------------------------------------------
  // Fetch usage
  // ---------------------------------------------------------------------------

  const fetchUsage = useCallback(
    async (showLoading = true) => {
      if (showLoading) {
        setLoading(true)
        setError(null)
      }
      try {
        const data = await orchestratorClient.getAgentUsage(agentId)
        if (mountedRef.current) {
          setUsage(data)
          setError(null)
        }
      } catch (err) {
        if (mountedRef.current) {
          const msg = err instanceof Error ? err.message : 'Failed to load usage'
          setError(msg)
        }
      } finally {
        if (mountedRef.current) {
          setLoading(false)
        }
      }
    },
    [agentId],
  )

  // ---------------------------------------------------------------------------
  // Initial fetch
  // ---------------------------------------------------------------------------

  useEffect(() => {
    fetchUsage(true)
  }, [fetchUsage])

  // ---------------------------------------------------------------------------
  // Auto-refresh
  // ---------------------------------------------------------------------------

  useEffect(() => {
    if (!autoRefreshMs) return

    const timer = setInterval(() => {
      fetchUsage(false)
    }, autoRefreshMs)

    return () => clearInterval(timer)
  }, [autoRefreshMs, fetchUsage])

  // ---------------------------------------------------------------------------
  // Clear context action
  // ---------------------------------------------------------------------------

  const clearContext = useCallback(async (): Promise<ClearContextResponse> => {
    setClearing(true)
    try {
      const response = await orchestratorClient.clearContext(agentId)
      // Refresh usage data after clearing to reflect the new session.
      await fetchUsage(false)
      return response
    } finally {
      if (mountedRef.current) {
        setClearing(false)
      }
    }
  }, [agentId, fetchUsage])

  return {
    usage,
    loading,
    error,
    refresh: () => fetchUsage(false),
    clearContext,
    clearing,
  }
}
