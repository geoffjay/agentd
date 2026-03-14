/**
 * useMemories — hook for memory list management.
 *
 * Provides:
 * - Paginated memory list with filtering (type, tag, visibility, created_by)
 * - CRUD actions: create, delete, updateVisibility
 * - Auto-refresh every 30 seconds (configurable)
 * - URL search params sync for bookmarkable filter/pagination state
 * - Error handling with mapApiError and toast notifications
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { memoryClient } from '@/services/memory'
import { useToast, mapApiError } from '@/hooks/useToast'
import type {
  CreateMemoryRequest,
  Memory,
  MemoryListParams,
  MemoryType,
  UpdateVisibilityRequest,
  VisibilityLevel,
} from '@/types/memory'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type MemorySortField = 'created_at' | 'updated_at' | 'type'
export type MemorySortDir = 'asc' | 'desc'

export interface MemoryFilters {
  /** Filter by memory type. */
  type?: MemoryType
  /** Filter by tag. */
  tag?: string
  /** Filter by creator identity. */
  created_by?: string
  /** Filter by visibility level. */
  visibility?: VisibilityLevel
}

export interface UseMemoriesOptions {
  /** Filters to apply on the server side. */
  filters?: MemoryFilters
  /** Client-side content search string. */
  search?: string
  /** Page number (1-based). */
  page?: number
  /** Items per page. */
  pageSize?: number
  /** Sort column. */
  sortBy?: MemorySortField
  /** Sort direction. */
  sortDir?: MemorySortDir
  /** Pause auto-refresh (e.g. when a dialog is open). */
  paused?: boolean
  /** Override auto-refresh interval (ms); default 30 000. */
  refreshInterval?: number
}

export interface UseMemoriesResult {
  /** Memories on the current page (after filter + sort). */
  memories: Memory[]
  /** Total matching memories (before pagination). */
  total: number
  /** All memories returned by the API (before client-side filtering). */
  allMemories: Memory[]
  loading: boolean
  /** True while a background refresh is running (initial load uses `loading`). */
  refreshing: boolean
  error?: string
  /** Trigger a manual refresh. */
  refetch: () => void
  /** Create a new memory; returns the created Memory. */
  createMemory: (request: CreateMemoryRequest) => Promise<Memory>
  /** Delete a memory by id. */
  deleteMemory: (id: string) => Promise<void>
  /** Update visibility and share list for a memory. */
  updateVisibility: (id: string, request: UpdateVisibilityRequest) => Promise<Memory>
}

// ---------------------------------------------------------------------------
// Sorting helpers
// ---------------------------------------------------------------------------

function compareMemories(
  a: Memory,
  b: Memory,
  field: MemorySortField,
  dir: MemorySortDir,
): number {
  let result = 0
  if (field === 'created_at') {
    result = new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
  } else if (field === 'updated_at') {
    result = new Date(a.updated_at).getTime() - new Date(b.updated_at).getTime()
  } else if (field === 'type') {
    result = a.type.localeCompare(b.type)
  }
  return dir === 'asc' ? result : -result
}

// ---------------------------------------------------------------------------
// URL search params helpers
// ---------------------------------------------------------------------------

/** Read memory filter/pagination state from URL search params. */
export function parseMemorySearchParams(params: URLSearchParams): {
  filters: MemoryFilters
  page: number
  pageSize: number
  search: string
  sortBy: MemorySortField
  sortDir: MemorySortDir
} {
  return {
    filters: {
      type: (params.get('type') as MemoryType) || undefined,
      tag: params.get('tag') || undefined,
      created_by: params.get('created_by') || undefined,
      visibility: (params.get('visibility') as VisibilityLevel) || undefined,
    },
    page: Number(params.get('page')) || 1,
    pageSize: Number(params.get('pageSize')) || DEFAULT_PAGE_SIZE,
    search: params.get('search') || '',
    sortBy: (params.get('sortBy') as MemorySortField) || 'created_at',
    sortDir: (params.get('sortDir') as MemorySortDir) || 'desc',
  }
}

/** Write memory filter/pagination state to URL search params. */
export function buildMemorySearchParams(options: {
  filters?: MemoryFilters
  page?: number
  pageSize?: number
  search?: string
  sortBy?: MemorySortField
  sortDir?: MemorySortDir
}): URLSearchParams {
  const params = new URLSearchParams()
  if (options.filters?.type) params.set('type', options.filters.type)
  if (options.filters?.tag) params.set('tag', options.filters.tag)
  if (options.filters?.created_by) params.set('created_by', options.filters.created_by)
  if (options.filters?.visibility) params.set('visibility', options.filters.visibility)
  if (options.page && options.page > 1) params.set('page', String(options.page))
  if (options.pageSize && options.pageSize !== DEFAULT_PAGE_SIZE)
    params.set('pageSize', String(options.pageSize))
  if (options.search) params.set('search', options.search)
  if (options.sortBy && options.sortBy !== 'created_at') params.set('sortBy', options.sortBy)
  if (options.sortDir && options.sortDir !== 'desc') params.set('sortDir', options.sortDir)
  return params
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_PAGE_SIZE = 50
const DEFAULT_REFRESH_INTERVAL = 30_000

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useMemories({
  filters = {},
  search = '',
  page = 1,
  pageSize = DEFAULT_PAGE_SIZE,
  sortBy = 'created_at',
  sortDir = 'desc',
  paused = false,
  refreshInterval = DEFAULT_REFRESH_INTERVAL,
}: UseMemoriesOptions = {}): UseMemoriesResult {
  const [allMemories, setAllMemories] = useState<Memory[]>([])
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | undefined>()
  const toast = useToast()

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // -------------------------------------------------------------------------
  // Fetch
  // -------------------------------------------------------------------------

  const fetchMemories = useCallback(
    async (isBackground = false) => {
      if (isBackground) {
        setRefreshing(true)
      } else {
        setLoading(true)
        setError(undefined)
      }

      try {
        const params: MemoryListParams = {
          limit: 200,
          ...(filters.type ? { type: filters.type } : {}),
          ...(filters.tag ? { tag: filters.tag } : {}),
          ...(filters.created_by ? { created_by: filters.created_by } : {}),
          ...(filters.visibility ? { visibility: filters.visibility } : {}),
        }
        const result = await memoryClient.listMemories(params)
        setAllMemories(result.items)
        setError(undefined)
      } catch (err) {
        const msg = mapApiError(err)
        setError(msg)
      } finally {
        setLoading(false)
        setRefreshing(false)
      }
    },
    [filters.type, filters.tag, filters.created_by, filters.visibility],
  )

  // -------------------------------------------------------------------------
  // Initial fetch + auto-refresh
  // -------------------------------------------------------------------------

  useEffect(() => {
    fetchMemories(false)
  }, [fetchMemories])

  useEffect(() => {
    if (paused) {
      if (timerRef.current) clearInterval(timerRef.current)
      return
    }
    timerRef.current = setInterval(() => fetchMemories(true), refreshInterval)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [paused, refreshInterval, fetchMemories])

  // -------------------------------------------------------------------------
  // Client-side filter, sort, paginate
  // -------------------------------------------------------------------------

  const filtered = allMemories.filter((m) => {
    if (search) {
      return m.content.toLowerCase().includes(search.toLowerCase())
    }
    return true
  })

  const sorted = [...filtered].sort((a, b) => compareMemories(a, b, sortBy, sortDir))
  const total = sorted.length
  const start = (page - 1) * pageSize
  const memories = sorted.slice(start, start + pageSize)

  // -------------------------------------------------------------------------
  // Actions
  // -------------------------------------------------------------------------

  const refetch = useCallback(() => fetchMemories(false), [fetchMemories])

  const createMemory = useCallback(
    async (request: CreateMemoryRequest): Promise<Memory> => {
      try {
        const created = await memoryClient.createMemory(request)
        await fetchMemories(true)
        return created
      } catch (err) {
        toast.apiError(err, 'Failed to create memory')
        throw err
      }
    },
    [fetchMemories, toast],
  )

  const deleteMemory = useCallback(
    async (id: string): Promise<void> => {
      try {
        await memoryClient.deleteMemory(id)
        setAllMemories((prev) => prev.filter((m) => m.id !== id))
      } catch (err) {
        toast.apiError(err, 'Failed to delete memory')
        throw err
      }
    },
    [toast],
  )

  const updateVisibility = useCallback(
    async (id: string, request: UpdateVisibilityRequest): Promise<Memory> => {
      try {
        const updated = await memoryClient.updateVisibility(id, request)
        setAllMemories((prev) => prev.map((m) => (m.id === id ? updated : m)))
        return updated
      } catch (err) {
        toast.apiError(err, 'Failed to update visibility')
        throw err
      }
    },
    [toast],
  )

  return {
    memories,
    total,
    allMemories,
    loading,
    refreshing,
    error,
    refetch,
    createMemory,
    deleteMemory,
    updateVisibility,
  }
}
