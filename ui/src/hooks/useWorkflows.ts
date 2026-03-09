/**
 * useWorkflows — hook for the workflow management page.
 *
 * Provides:
 * - Paginated workflow fetching with auto-refresh
 * - create, update, delete, toggle-enabled actions
 * - Per-workflow dispatch history
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import type {
  CreateWorkflowRequest,
  DispatchRecord,
  UpdateWorkflowRequest,
  Workflow,
} from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type WorkflowSortField = 'name' | 'created_at' | 'poll_interval_secs'
export type WorkflowSortDir = 'asc' | 'desc'

export interface UseWorkflowsOptions {
  /** Only include enabled/disabled workflows; undefined = all */
  enabled?: boolean
  /** Client-side name search */
  search?: string
  page?: number
  pageSize?: number
  sortBy?: WorkflowSortField
  sortDir?: WorkflowSortDir
  /** Pause auto-refresh */
  paused?: boolean
  refreshInterval?: number
}

export interface UseWorkflowsResult {
  workflows: Workflow[]
  total: number
  allWorkflows: Workflow[]
  loading: boolean
  refreshing: boolean
  error?: string
  refetch: () => void
  createWorkflow: (request: CreateWorkflowRequest) => Promise<Workflow>
  updateWorkflow: (id: string, request: UpdateWorkflowRequest) => Promise<Workflow>
  deleteWorkflow: (id: string) => Promise<void>
  toggleEnabled: (id: string, enabled: boolean) => Promise<Workflow>
}

export interface UseWorkflowDetailResult {
  workflow?: Workflow
  loading: boolean
  error?: string
  refetch: () => void
  updateWorkflow: (request: UpdateWorkflowRequest) => Promise<Workflow>
  deleteWorkflow: () => Promise<void>
}

export interface UseDispatchHistoryOptions {
  workflowId: string
  page?: number
  pageSize?: number
  status?: DispatchRecord['status']
}

export interface UseDispatchHistoryResult {
  dispatches: DispatchRecord[]
  total: number
  loading: boolean
  error?: string
  refetch: () => void
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DEFAULT_PAGE_SIZE = 20
const DEFAULT_REFRESH_INTERVAL = 30_000

function compareWorkflows(
  a: Workflow,
  b: Workflow,
  field: WorkflowSortField,
  dir: WorkflowSortDir,
): number {
  let result = 0
  if (field === 'name') {
    result = a.name.localeCompare(b.name)
  } else if (field === 'created_at') {
    result = new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
  } else if (field === 'poll_interval_secs') {
    result = a.poll_interval_secs - b.poll_interval_secs
  }
  return dir === 'asc' ? result : -result
}

// ---------------------------------------------------------------------------
// useWorkflows — list hook
// ---------------------------------------------------------------------------

export function useWorkflows({
  enabled,
  search = '',
  page = 1,
  pageSize = DEFAULT_PAGE_SIZE,
  sortBy = 'created_at',
  sortDir = 'desc',
  paused = false,
  refreshInterval = DEFAULT_REFRESH_INTERVAL,
}: UseWorkflowsOptions = {}): UseWorkflowsResult {
  const [allWorkflows, setAllWorkflows] = useState<Workflow[]>([])
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | undefined>()

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetchWorkflows = useCallback(async (isBackground = false) => {
    if (isBackground) {
      setRefreshing(true)
    } else {
      setLoading(true)
      setError(undefined)
    }

    try {
      const result = await orchestratorClient.listWorkflows({ limit: 200 })
      setAllWorkflows(result.items)
      setError(undefined)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load workflows')
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }, [])

  useEffect(() => {
    fetchWorkflows(false)
  }, [fetchWorkflows])

  useEffect(() => {
    if (paused) {
      if (timerRef.current) clearInterval(timerRef.current)
      return
    }
    timerRef.current = setInterval(() => fetchWorkflows(true), refreshInterval)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [paused, refreshInterval, fetchWorkflows])

  // Client-side filter, sort, paginate
  const filtered = allWorkflows.filter((w) => {
    if (enabled !== undefined && w.enabled !== enabled) return false
    if (search) return w.name.toLowerCase().includes(search.toLowerCase())
    return true
  })

  const sorted = [...filtered].sort((a, b) => compareWorkflows(a, b, sortBy, sortDir))
  const total = sorted.length
  const start = (page - 1) * pageSize
  const workflows = sorted.slice(start, start + pageSize)

  // Actions
  const refetch = useCallback(() => fetchWorkflows(false), [fetchWorkflows])

  const createWorkflow = useCallback(
    async (request: CreateWorkflowRequest): Promise<Workflow> => {
      const created = await orchestratorClient.createWorkflow(request)
      await fetchWorkflows(true)
      return created
    },
    [fetchWorkflows],
  )

  const updateWorkflow = useCallback(
    async (id: string, request: UpdateWorkflowRequest): Promise<Workflow> => {
      const updated = await orchestratorClient.updateWorkflow(id, request)
      setAllWorkflows((prev) => prev.map((w) => (w.id === id ? updated : w)))
      return updated
    },
    [],
  )

  const deleteWorkflow = useCallback(async (id: string): Promise<void> => {
    await orchestratorClient.deleteWorkflow(id)
    setAllWorkflows((prev) => prev.filter((w) => w.id !== id))
  }, [])

  const toggleEnabled = useCallback(
    async (id: string, newEnabled: boolean): Promise<Workflow> => {
      return updateWorkflow(id, { enabled: newEnabled })
    },
    [updateWorkflow],
  )

  return {
    workflows,
    total,
    allWorkflows,
    loading,
    refreshing,
    error,
    refetch,
    createWorkflow,
    updateWorkflow,
    deleteWorkflow,
    toggleEnabled,
  }
}

// ---------------------------------------------------------------------------
// useWorkflowDetail — single workflow
// ---------------------------------------------------------------------------

export function useWorkflowDetail(id: string): UseWorkflowDetailResult {
  const [workflow, setWorkflow] = useState<Workflow | undefined>()
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const fetchWorkflow = useCallback(async () => {
    setLoading(true)
    setError(undefined)
    try {
      const result = await orchestratorClient.getWorkflow(id)
      setWorkflow(result)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load workflow')
    } finally {
      setLoading(false)
    }
  }, [id])

  useEffect(() => {
    fetchWorkflow()
  }, [fetchWorkflow])

  const updateWorkflow = useCallback(
    async (request: UpdateWorkflowRequest): Promise<Workflow> => {
      const updated = await orchestratorClient.updateWorkflow(id, request)
      setWorkflow(updated)
      return updated
    },
    [id],
  )

  const deleteWorkflow = useCallback(async (): Promise<void> => {
    await orchestratorClient.deleteWorkflow(id)
  }, [id])

  return {
    workflow,
    loading,
    error,
    refetch: fetchWorkflow,
    updateWorkflow,
    deleteWorkflow,
  }
}

// ---------------------------------------------------------------------------
// useDispatchHistory — per-workflow dispatch history
// ---------------------------------------------------------------------------

export function useDispatchHistory({
  workflowId,
  page = 1,
  pageSize = DEFAULT_PAGE_SIZE,
  status,
}: UseDispatchHistoryOptions): UseDispatchHistoryResult {
  const [allDispatches, setAllDispatches] = useState<DispatchRecord[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const fetchHistory = useCallback(async () => {
    setLoading(true)
    setError(undefined)
    try {
      const result = await orchestratorClient.getWorkflowHistory(workflowId, {
        limit: 100,
        offset: 0,
      })
      setAllDispatches(result.items)
      setTotal(result.total)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load dispatch history')
    } finally {
      setLoading(false)
    }
  }, [workflowId])

  useEffect(() => {
    fetchHistory()
  }, [fetchHistory])

  // Client-side filter + paginate
  const filtered = status ? allDispatches.filter((d) => d.status === status) : allDispatches
  const start = (page - 1) * pageSize
  const dispatches = filtered.slice(start, start + pageSize)

  return {
    dispatches,
    total: status ? filtered.length : total,
    loading,
    error,
    refetch: fetchHistory,
  }
}
