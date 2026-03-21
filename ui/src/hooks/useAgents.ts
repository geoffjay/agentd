/**
 * useAgents — hook for the agents list page.
 *
 * Provides:
 * - Paginated agent fetching with status filter and client-side name search
 * - Column sorting (name, status, created_at)
 * - Auto-refresh every 10 seconds (paused when `paused` is true)
 * - createAgent / deleteAgent / bulkDelete actions
 * - URL-query-param sync helpers (returned separately)
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import type { Agent, AgentStatus, AgentUsageStats, CreateAgentRequest } from '@/types/orchestrator'
import { inferAgentRole } from '@/types/agent-roles'
import type { AgentRole } from '@/types/agent-roles'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type SortField = 'name' | 'status' | 'created_at' | 'cost' | 'tokens' | 'cache'
export type SortDir = 'asc' | 'desc'

export interface UseAgentsOptions {
  /** Status filter; undefined = all */
  status?: AgentStatus | ''
  /** Client-side role filter; empty string = all */
  role?: AgentRole | ''
  /** Client-side name search string */
  search?: string
  /** Page number (1-based) */
  page?: number
  /** Items per page */
  pageSize?: number
  /** Sort column */
  sortBy?: SortField
  /** Sort direction */
  sortDir?: SortDir
  /** Pause auto-refresh (e.g. when a dialog is open) */
  paused?: boolean
  /** Override auto-refresh interval (ms); default 10000 */
  refreshInterval?: number
}

export interface UseAgentsResult {
  /** Agents on the current page (after filter + sort) */
  agents: Agent[]
  /** Total matching agents (before pagination) */
  total: number
  /** All agents returned by the API (before client-side filtering) */
  allAgents: Agent[]
  /** Per-agent usage stats keyed by agent ID */
  usageMap: Map<string, AgentUsageStats>
  loading: boolean
  /** True while a background refresh is running (initial load uses loading) */
  refreshing: boolean
  error?: string
  /** Trigger a manual refresh */
  refetch: () => void
  /** Create a new agent; returns the created Agent */
  createAgent: (request: CreateAgentRequest) => Promise<Agent>
  /** Terminate a single agent by id */
  deleteAgent: (id: string) => Promise<void>
  /** Terminate multiple agents by id */
  bulkDelete: (ids: string[]) => Promise<void>
}

// ---------------------------------------------------------------------------
// Sorting helpers
// ---------------------------------------------------------------------------

function compareAgents(
  a: Agent,
  b: Agent,
  field: SortField,
  dir: SortDir,
  usageMap?: Map<string, AgentUsageStats>,
): number {
  let result = 0
  if (field === 'name') {
    result = a.name.localeCompare(b.name)
  } else if (field === 'status') {
    result = a.status.localeCompare(b.status)
  } else if (field === 'created_at') {
    result = new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
  } else if (field === 'cost' && usageMap) {
    const aCost = usageMap.get(a.id)?.cumulative.total_cost_usd ?? 0
    const bCost = usageMap.get(b.id)?.cumulative.total_cost_usd ?? 0
    result = aCost - bCost
  } else if (field === 'tokens' && usageMap) {
    const aTokens = usageMap.get(a.id)?.cumulative.input_tokens ?? 0
    const bTokens = usageMap.get(b.id)?.cumulative.input_tokens ?? 0
    result = aTokens - bTokens
  } else if (field === 'cache' && usageMap) {
    const aStats = usageMap.get(a.id)?.cumulative
    const bStats = usageMap.get(b.id)?.cumulative
    const aRatio = aStats
      ? aStats.cache_read_input_tokens /
        (aStats.cache_read_input_tokens + aStats.cache_creation_input_tokens + aStats.input_tokens || 1)
      : 0
    const bRatio = bStats
      ? bStats.cache_read_input_tokens /
        (bStats.cache_read_input_tokens + bStats.cache_creation_input_tokens + bStats.input_tokens || 1)
      : 0
    result = aRatio - bRatio
  }
  return dir === 'asc' ? result : -result
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

const DEFAULT_PAGE_SIZE = 20
const DEFAULT_REFRESH_INTERVAL = 10_000

export function useAgents({
  status,
  role = '',
  search = '',
  page = 1,
  pageSize = DEFAULT_PAGE_SIZE,
  sortBy = 'created_at',
  sortDir = 'desc',
  paused = false,
  refreshInterval = DEFAULT_REFRESH_INTERVAL,
}: UseAgentsOptions = {}): UseAgentsResult {
  const [allAgents, setAllAgents] = useState<Agent[]>([])
  const [usageMap, setUsageMap] = useState<Map<string, AgentUsageStats>>(new Map())
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | undefined>()

  const isInitial = useRef(true)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // ---------------------------------------------------------------------------
  // Fetch
  // ---------------------------------------------------------------------------

  const fetchAgents = useCallback(
    async (isBackground = false) => {
      if (isBackground) {
        setRefreshing(true)
      } else {
        setLoading(true)
        setError(undefined)
      }

      try {
        const params = {
          limit: 200, // fetch a large batch; filter/paginate client-side
          ...(status ? { status } : {}),
        }
        const result = await orchestratorClient.listAgents(params)
        setAllAgents(result.items)

        // Fetch usage for each agent in parallel (best-effort)
        const usageResults = await Promise.allSettled(
          result.items.map((agent) => orchestratorClient.getAgentUsage(agent.id)),
        )
        const newUsageMap = new Map<string, AgentUsageStats>()
        for (let i = 0; i < result.items.length; i++) {
          const r = usageResults[i]
          if (r.status === 'fulfilled') {
            newUsageMap.set(result.items[i].id, r.value)
          }
        }
        setUsageMap(newUsageMap)

        setError(undefined)
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to load agents'
        setError(msg)
      } finally {
        setLoading(false)
        setRefreshing(false)
      }
    },
    [status],
  )

  // ---------------------------------------------------------------------------
  // Initial fetch + auto-refresh
  // ---------------------------------------------------------------------------

  useEffect(() => {
    isInitial.current = true
    fetchAgents(false)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status]) // re-fetch when status filter changes

  useEffect(() => {
    if (paused) {
      if (timerRef.current) clearInterval(timerRef.current)
      return
    }

    timerRef.current = setInterval(() => {
      fetchAgents(true)
    }, refreshInterval)

    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [paused, refreshInterval, fetchAgents])

  // ---------------------------------------------------------------------------
  // Client-side filter, sort, paginate
  // ---------------------------------------------------------------------------

  const filtered = allAgents.filter((agent) => {
    if (search && !agent.name.toLowerCase().includes(search.toLowerCase())) {
      return false
    }
    if (role && inferAgentRole(agent.name) !== role) {
      return false
    }
    return true
  })

  const sorted = [...filtered].sort((a, b) => compareAgents(a, b, sortBy, sortDir, usageMap))

  const total = sorted.length
  const start = (page - 1) * pageSize
  const agents = sorted.slice(start, start + pageSize)

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  const refetch = useCallback(() => fetchAgents(false), [fetchAgents])

  const createAgent = useCallback(
    async (request: CreateAgentRequest): Promise<Agent> => {
      const created = await orchestratorClient.createAgent(request)
      await fetchAgents(true) // refresh list after creation
      return created
    },
    [fetchAgents],
  )

  const deleteAgent = useCallback(async (id: string): Promise<void> => {
    await orchestratorClient.deleteAgent(id)
    setAllAgents((prev) => prev.filter((a) => a.id !== id))
  }, [])

  const bulkDelete = useCallback(async (ids: string[]): Promise<void> => {
    await Promise.all(ids.map((id) => orchestratorClient.deleteAgent(id)))
    setAllAgents((prev) => prev.filter((a) => !ids.includes(a.id)))
  }, [])

  return {
    agents,
    total,
    allAgents,
    usageMap,
    loading,
    refreshing,
    error,
    refetch,
    createAgent,
    deleteAgent,
    bulkDelete,
  }
}
