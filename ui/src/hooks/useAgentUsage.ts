/**
 * useAgentUsage — hook for fetching agent usage stats and triggering context clears.
 *
 * Provides:
 * - Usage data fetching with configurable auto-refresh
 * - Real-time optimistic updates via agentEventBus (usage_update / context_cleared)
 * - Loading / error state
 * - clearContext action that triggers API call, then refreshes usage data
 * - Auto-pauses refresh when agent is stopped/failed
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import { agentEventBus } from '@/services/eventBus'
import type {
  AgentUsageStats,
  ClearContextResponse,
  UsageUpdateEvent,
  ContextClearedEvent,
} from '@/types/orchestrator'

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
// Helpers
// ---------------------------------------------------------------------------

/**
 * Optimistically apply a UsageSnapshot delta to existing AgentUsageStats.
 * The snapshot from the event represents a single turn's usage, so we add
 * it to both the current session and cumulative totals.
 */
function applyUsageUpdate(
  prev: AgentUsageStats,
  event: UsageUpdateEvent,
): AgentUsageStats {
  const snapshot = event.usage
  const currentSession = prev.current_session
    ? {
        ...prev.current_session,
        input_tokens: prev.current_session.input_tokens + snapshot.input_tokens,
        output_tokens: prev.current_session.output_tokens + snapshot.output_tokens,
        cache_read_input_tokens:
          prev.current_session.cache_read_input_tokens + snapshot.cache_read_input_tokens,
        cache_creation_input_tokens:
          prev.current_session.cache_creation_input_tokens +
          snapshot.cache_creation_input_tokens,
        total_cost_usd: prev.current_session.total_cost_usd + snapshot.total_cost_usd,
        num_turns: prev.current_session.num_turns + snapshot.num_turns,
        duration_ms: prev.current_session.duration_ms + snapshot.duration_ms,
        duration_api_ms: prev.current_session.duration_api_ms + snapshot.duration_api_ms,
        result_count: prev.current_session.result_count + 1,
      }
    : undefined

  return {
    ...prev,
    current_session: currentSession,
    cumulative: {
      ...prev.cumulative,
      input_tokens: prev.cumulative.input_tokens + snapshot.input_tokens,
      output_tokens: prev.cumulative.output_tokens + snapshot.output_tokens,
      cache_read_input_tokens:
        prev.cumulative.cache_read_input_tokens + snapshot.cache_read_input_tokens,
      cache_creation_input_tokens:
        prev.cumulative.cache_creation_input_tokens + snapshot.cache_creation_input_tokens,
      total_cost_usd: prev.cumulative.total_cost_usd + snapshot.total_cost_usd,
      num_turns: prev.cumulative.num_turns + snapshot.num_turns,
      duration_ms: prev.cumulative.duration_ms + snapshot.duration_ms,
      duration_api_ms: prev.cumulative.duration_api_ms + snapshot.duration_api_ms,
      result_count: prev.cumulative.result_count + 1,
    },
  }
}

/**
 * Handle a context_cleared event: reset the current session and bump count.
 */
function applyContextCleared(
  prev: AgentUsageStats,
  event: ContextClearedEvent,
): AgentUsageStats {
  return {
    ...prev,
    current_session: {
      input_tokens: 0,
      output_tokens: 0,
      cache_read_input_tokens: 0,
      cache_creation_input_tokens: 0,
      total_cost_usd: 0,
      num_turns: 0,
      duration_ms: 0,
      duration_api_ms: 0,
      result_count: 0,
      started_at: event.timestamp,
    },
    session_count: prev.session_count + 1,
  }
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
  // Real-time event bus subscriptions
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const unsubUsage = agentEventBus.on<UsageUpdateEvent>(
      'agent:usage_update',
      (event) => {
        if (event.agentId !== agentId || !mountedRef.current) return
        setUsage((prev) => (prev ? applyUsageUpdate(prev, event) : prev))
      },
    )

    const unsubCleared = agentEventBus.on<ContextClearedEvent>(
      'agent:context_cleared',
      (event) => {
        if (event.agentId !== agentId || !mountedRef.current) return
        setUsage((prev) => (prev ? applyContextCleared(prev, event) : prev))
      },
    )

    return () => {
      unsubUsage()
      unsubCleared()
    }
  }, [agentId])

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
