/**
 * useAgentSummary — fetches agent counts by status and the 5 most recently
 * updated agents.
 */

import { useCallback, useEffect, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import type { Agent, AgentStatus, AgentUsageStats } from '@/types/orchestrator'

export interface AgentStatusCounts {
  running: number
  pending: number
  stopped: number
  failed: number
}

/** Aggregate usage stats across all agents */
export interface AggregateUsageSummary {
  totalCostUsd: number
  totalTokens: number
  cacheHitPercent: number
}

export interface UseAgentSummaryResult {
  counts: AgentStatusCounts
  recentAgents: Agent[]
  total: number
  /** Aggregate usage stats (null while loading or on error) */
  aggregateUsage: AggregateUsageSummary | null
  loading: boolean
  error?: string
}

const EMPTY_COUNTS: AgentStatusCounts = { running: 0, pending: 0, stopped: 0, failed: 0 }

function computeAggregateUsage(usageList: AgentUsageStats[]): AggregateUsageSummary {
  let totalCostUsd = 0
  let totalTokens = 0
  let totalCacheRead = 0
  let totalCacheCreation = 0
  let totalInput = 0

  for (const stats of usageList) {
    const c = stats.cumulative
    totalCostUsd += c.total_cost_usd
    totalTokens += c.input_tokens + c.output_tokens
    totalCacheRead += c.cache_read_input_tokens
    totalCacheCreation += c.cache_creation_input_tokens
    totalInput += c.input_tokens
  }

  const cacheTotal = totalCacheRead + totalCacheCreation + totalInput
  const cacheHitPercent = cacheTotal > 0 ? (totalCacheRead / cacheTotal) * 100 : 0

  return { totalCostUsd, totalTokens, cacheHitPercent }
}

export function useAgentSummary(): UseAgentSummaryResult {
  const [counts, setCounts] = useState<AgentStatusCounts>(EMPTY_COUNTS)
  const [recentAgents, setRecentAgents] = useState<Agent[]>([])
  const [total, setTotal] = useState(0)
  const [aggregateUsage, setAggregateUsage] = useState<AggregateUsageSummary | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const fetch = useCallback(async () => {
    setLoading(true)
    setError(undefined)
    try {
      // Fetch all agents (up to 200 for summary purposes)
      const result = await orchestratorClient.listAgents({ limit: 200 })
      const agents = result.items

      const newCounts: AgentStatusCounts = { running: 0, pending: 0, stopped: 0, failed: 0 }
      for (const agent of agents) {
        const s = agent.status as AgentStatus
        if (s in newCounts) newCounts[s]++
      }

      // Sort by updated_at descending, take top 5
      const sorted = [...agents].sort(
        (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
      )

      // Fetch usage for each agent in parallel (best-effort)
      const usageResults = await Promise.allSettled(
        agents.map((agent) => orchestratorClient.getAgentUsage(agent.id)),
      )
      const usageList: AgentUsageStats[] = []
      for (const r of usageResults) {
        if (r.status === 'fulfilled') usageList.push(r.value)
      }

      setCounts(newCounts)
      setRecentAgents(sorted.slice(0, 5))
      setTotal(result.total)
      setAggregateUsage(usageList.length > 0 ? computeAggregateUsage(usageList) : null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load agents')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void fetch()
  }, [fetch])

  return { counts, recentAgents, total, aggregateUsage, loading, error }
}
