/**
 * useUsageMetrics — fetches and aggregates token usage, cache efficiency,
 * and cost metrics across all running agents.
 *
 * Iterates `GET /agents/{id}/usage` for each agent discovered via
 * `GET /agents` and provides dashboard-level computed values:
 * - Per-agent usage breakdowns (for bar/pie charts)
 * - Aggregate totals: total cost, total tokens, cache hit ratio
 * - Auto-refresh on a configurable interval (same options as useMetrics)
 * - Pauses when the tab is hidden
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import type { Agent, AgentUsageStats } from '@/types/orchestrator'
import type { RefreshInterval } from './useMetrics'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AgentUsageEntry {
  /** Agent ID */
  agentId: string
  /** Agent name (for chart labels) */
  name: string
  /** Raw usage stats from the API */
  stats: AgentUsageStats
}

export interface AggregateUsage {
  totalInputTokens: number
  totalOutputTokens: number
  totalCacheReadTokens: number
  totalCacheCreationTokens: number
  totalCostUsd: number
  totalTokens: number
  /** Cache hit ratio as a fraction 0–1 */
  cacheHitRatio: number
}

export interface UseUsageMetricsResult {
  /** Per-agent usage entries */
  entries: AgentUsageEntry[]
  /** Aggregate computed values */
  aggregate: AggregateUsage
  /** Loading state */
  loading: boolean
  /** Error message, if any */
  error?: string
  /** Trigger a manual refresh */
  refetch: () => void
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const EMPTY_AGGREGATE: AggregateUsage = {
  totalInputTokens: 0,
  totalOutputTokens: 0,
  totalCacheReadTokens: 0,
  totalCacheCreationTokens: 0,
  totalCostUsd: 0,
  totalTokens: 0,
  cacheHitRatio: 0,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function computeAggregate(entries: AgentUsageEntry[]): AggregateUsage {
  let totalInputTokens = 0
  let totalOutputTokens = 0
  let totalCacheReadTokens = 0
  let totalCacheCreationTokens = 0
  let totalCostUsd = 0

  for (const entry of entries) {
    const c = entry.stats.cumulative
    totalInputTokens += c.input_tokens
    totalOutputTokens += c.output_tokens
    totalCacheReadTokens += c.cache_read_input_tokens
    totalCacheCreationTokens += c.cache_creation_input_tokens
    totalCostUsd += c.total_cost_usd
  }

  const totalTokens =
    totalInputTokens + totalOutputTokens + totalCacheReadTokens + totalCacheCreationTokens

  // Cache hit ratio: cache reads / (cache reads + cache creation + non-cached input)
  const cacheTotal = totalCacheReadTokens + totalCacheCreationTokens + totalInputTokens
  const cacheHitRatio = cacheTotal > 0 ? totalCacheReadTokens / cacheTotal : 0

  return {
    totalInputTokens,
    totalOutputTokens,
    totalCacheReadTokens,
    totalCacheCreationTokens,
    totalCostUsd,
    totalTokens,
    cacheHitRatio,
  }
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useUsageMetrics(refreshInterval: RefreshInterval = 30_000): UseUsageMetricsResult {
  const [entries, setEntries] = useState<AgentUsageEntry[]>([])
  const [aggregate, setAggregate] = useState<AggregateUsage>(EMPTY_AGGREGATE)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const mountedRef = useRef(true)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const fetchAll = useCallback(async () => {
    try {
      // 1. List all agents
      const agentsResp = await orchestratorClient.listAgents({ limit: 200 })
      const agents: Agent[] = agentsResp.items

      // 2. Fetch usage for each agent in parallel
      const usageResults = await Promise.allSettled(
        agents.map((agent) => orchestratorClient.getAgentUsage(agent.id)),
      )

      // 3. Build entries (skip agents where usage fetch failed)
      const newEntries: AgentUsageEntry[] = []
      for (let i = 0; i < agents.length; i++) {
        const result = usageResults[i]
        if (result.status === 'fulfilled') {
          newEntries.push({
            agentId: agents[i].id,
            name: agents[i].name,
            stats: result.value,
          })
        }
      }

      if (mountedRef.current) {
        setEntries(newEntries)
        setAggregate(computeAggregate(newEntries))
        setError(undefined)
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(err instanceof Error ? err.message : 'Failed to fetch usage metrics')
      }
    } finally {
      if (mountedRef.current) {
        setLoading(false)
      }
    }
  }, [])

  // Initial fetch
  useEffect(() => {
    void fetchAll()
  }, [fetchAll])

  // Auto-refresh
  useEffect(() => {
    if (timerRef.current) clearInterval(timerRef.current)
    timerRef.current = setInterval(() => void fetchAll(), refreshInterval)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [refreshInterval, fetchAll])

  // Pause on tab blur, resume on focus
  useEffect(() => {
    function handleVisibility() {
      if (document.hidden) {
        if (timerRef.current) clearInterval(timerRef.current)
      } else {
        void fetchAll()
        timerRef.current = setInterval(() => void fetchAll(), refreshInterval)
      }
    }
    document.addEventListener('visibilitychange', handleVisibility)
    return () => {
      document.removeEventListener('visibilitychange', handleVisibility)
    }
  }, [refreshInterval, fetchAll])

  return {
    entries,
    aggregate,
    loading,
    error,
    refetch: () => void fetchAll(),
  }
}
