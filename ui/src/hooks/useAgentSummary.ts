/**
 * useAgentSummary — fetches agent counts by status and the 5 most recently
 * updated agents.
 */

import { useCallback, useEffect, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import type { Agent, AgentStatus } from '@/types/orchestrator'

export interface AgentStatusCounts {
  Running: number
  Pending: number
  Stopped: number
  Failed: number
}

export interface UseAgentSummaryResult {
  counts: AgentStatusCounts
  recentAgents: Agent[]
  total: number
  loading: boolean
  error?: string
}

const EMPTY_COUNTS: AgentStatusCounts = { Running: 0, Pending: 0, Stopped: 0, Failed: 0 }

export function useAgentSummary(): UseAgentSummaryResult {
  const [counts, setCounts] = useState<AgentStatusCounts>(EMPTY_COUNTS)
  const [recentAgents, setRecentAgents] = useState<Agent[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const fetch = useCallback(async () => {
    setLoading(true)
    setError(undefined)
    try {
      // Fetch all agents (up to 200 for summary purposes)
      const result = await orchestratorClient.listAgents({ limit: 200 })
      const agents = result.items

      const newCounts: AgentStatusCounts = { Running: 0, Pending: 0, Stopped: 0, Failed: 0 }
      for (const agent of agents) {
        const s = agent.status as AgentStatus
        if (s in newCounts) newCounts[s]++
      }

      // Sort by updated_at descending, take top 5
      const sorted = [...agents].sort(
        (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
      )

      setCounts(newCounts)
      setRecentAgents(sorted.slice(0, 5))
      setTotal(result.total)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load agents')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void fetch()
  }, [fetch])

  return { counts, recentAgents, total, loading, error }
}
