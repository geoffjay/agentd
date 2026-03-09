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
import type { Agent, AgentStatus, CreateAgentRequest } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type SortField = 'name' | 'status' | 'created_at'
export type SortDir = 'asc' | 'desc'

export interface UseAgentsOptions {
  /** Status filter; undefined = all */
  status?: AgentStatus | ''
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

function compareAgents(a: Agent, b: Agent, field: SortField, dir: SortDir): number {
  let result = 0
  if (field === 'name') {
    result = a.name.localeCompare(b.name)
  } else if (field === 'status') {
    result = a.status.localeCompare(b.status)
  } else if (field === 'created_at') {
    result = new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
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
  search = '',
  page = 1,
  pageSize = DEFAULT_PAGE_SIZE,
  sortBy = 'created_at',
  sortDir = 'desc',
  paused = false,
  refreshInterval = DEFAULT_REFRESH_INTERVAL,
}: UseAgentsOptions = {}): UseAgentsResult {
  const [allAgents, setAllAgents] = useState<Agent[]>([])
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
    if (search) {
      return agent.name.toLowerCase().includes(search.toLowerCase())
    }
    return true
  })

  const sorted = [...filtered].sort((a, b) => compareAgents(a, b, sortBy, sortDir))

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
    loading,
    refreshing,
    error,
    refetch,
    createAgent,
    deleteAgent,
    bulkDelete,
  }
}
