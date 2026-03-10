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
 * - Real-time updates from WebSocket events (debounced to avoid excessive re-renders)
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import { agentEventBus } from '@/services/eventBus'
import type {
  Agent,
  AgentUsageStats,
  UsageUpdateEvent,
  ContextClearedEvent,
} from '@/types/orchestrator'
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

/** Debounce window for real-time event updates (ms) */
const REALTIME_DEBOUNCE_MS = 2_000

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export function computeAggregate(entries: AgentUsageEntry[]): AggregateUsage {
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

/**
 * Apply a usage update event to an entries array, returning new entries + aggregate.
 */
function applyUsageEventToEntries(
  entries: AgentUsageEntry[],
  event: UsageUpdateEvent,
): AgentUsageEntry[] {
  return entries.map((entry) => {
    if (entry.agentId !== event.agentId) return entry
    const snapshot = event.usage
    return {
      ...entry,
      stats: {
        ...entry.stats,
        cumulative: {
          ...entry.stats.cumulative,
          input_tokens: entry.stats.cumulative.input_tokens + snapshot.input_tokens,
          output_tokens: entry.stats.cumulative.output_tokens + snapshot.output_tokens,
          cache_read_input_tokens:
            entry.stats.cumulative.cache_read_input_tokens + snapshot.cache_read_input_tokens,
          cache_creation_input_tokens:
            entry.stats.cumulative.cache_creation_input_tokens +
            snapshot.cache_creation_input_tokens,
          total_cost_usd: entry.stats.cumulative.total_cost_usd + snapshot.total_cost_usd,
          num_turns: entry.stats.cumulative.num_turns + snapshot.num_turns,
          duration_ms: entry.stats.cumulative.duration_ms + snapshot.duration_ms,
          duration_api_ms: entry.stats.cumulative.duration_api_ms + snapshot.duration_api_ms,
          result_count: entry.stats.cumulative.result_count + 1,
        },
      },
    }
  })
}

/**
 * Apply a context cleared event: reset current session, bump session count.
 */
function applyContextClearToEntries(
  entries: AgentUsageEntry[],
  event: ContextClearedEvent,
): AgentUsageEntry[] {
  return entries.map((entry) => {
    if (entry.agentId !== event.agentId) return entry
    return {
      ...entry,
      stats: {
        ...entry.stats,
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
        session_count: entry.stats.session_count + 1,
      },
    }
  })
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
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Keep a mutable ref to entries so event handlers can read without stale closures
  const entriesRef = useRef(entries)
  entriesRef.current = entries

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  // Debounced commit: batches rapid event updates into a single state change
  const scheduleCommit = useCallback((updatedEntries: AgentUsageEntry[]) => {
    entriesRef.current = updatedEntries
    if (debounceRef.current) clearTimeout(debounceRef.current)
    debounceRef.current = setTimeout(() => {
      if (!mountedRef.current) return
      setEntries(entriesRef.current)
      setAggregate(computeAggregate(entriesRef.current))
      debounceRef.current = null
    }, REALTIME_DEBOUNCE_MS)
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
        entriesRef.current = newEntries
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

  // ---------------------------------------------------------------------------
  // Real-time event bus subscriptions (debounced)
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const unsubUsage = agentEventBus.on<UsageUpdateEvent>(
      'agent:usage_update',
      (event) => {
        if (!mountedRef.current) return
        const updated = applyUsageEventToEntries(entriesRef.current, event)
        scheduleCommit(updated)
      },
    )

    const unsubCleared = agentEventBus.on<ContextClearedEvent>(
      'agent:context_cleared',
      (event) => {
        if (!mountedRef.current) return
        const updated = applyContextClearToEntries(entriesRef.current, event)
        scheduleCommit(updated)
      },
    )

    return () => {
      unsubUsage()
      unsubCleared()
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [scheduleCommit])

  return {
    entries,
    aggregate,
    loading,
    error,
    refetch: () => void fetchAll(),
  }
}
