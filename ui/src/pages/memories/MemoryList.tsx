/**
 * MemoryList — full memory management page.
 *
 * Features:
 * - Tab toggle between Browse (list) and Semantic Search modes
 * - Filter by type, visibility, creator, tag, and content search
 * - Sort by created_at, updated_at, type
 * - Paginated list of memory cards
 * - Create memory dialog
 * - Delete confirmation dialog
 * - URL query param sync for filters/sort/pagination
 * - Loading skeleton, error state, and empty state
 */

import { useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Brain, List, Plus, RefreshCw, Search } from 'lucide-react'
import {
  useMemories,
  type MemoryFilters as MemoryFiltersType,
  type MemorySortField,
  type MemorySortDir,
} from '@/hooks/useMemories'
import { MemoryCard } from '@/components/memories/MemoryCard'
import { MemoryFilters } from '@/components/memories/MemoryFilters'
import { MemorySearch } from '@/components/memories/MemorySearch'
import { CreateMemoryDialog } from '@/components/memories/CreateMemoryDialog'
import { Pagination } from '@/components/common/Pagination'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'
import { CardSkeleton } from '@/components/common/LoadingSkeleton'
import type { Memory } from '@/types/memory'

// ---------------------------------------------------------------------------
// View mode
// ---------------------------------------------------------------------------

type ViewMode = 'list' | 'search'

// ---------------------------------------------------------------------------
// URL sync helpers
// ---------------------------------------------------------------------------

function filtersFromParams(p: URLSearchParams): MemoryFiltersType {
  return {
    type: (p.get('type') as MemoryFiltersType['type']) || undefined,
    visibility: (p.get('visibility') as MemoryFiltersType['visibility']) || undefined,
    created_by: p.get('created_by') || undefined,
    tag: p.get('tag') || undefined,
  }
}

function sortFieldFromParam(p: string | null): MemorySortField {
  if (p === 'updated_at' || p === 'type') return p
  return 'created_at'
}

function sortDirFromParam(p: string | null): MemorySortDir {
  if (p === 'asc') return 'asc'
  return 'desc'
}

function viewModeFromParam(p: string | null): ViewMode {
  if (p === 'search') return 'search'
  return 'list'
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_PAGE_SIZE = 20

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MemoryList() {
  const [searchParams, setSearchParams] = useSearchParams()

  // View mode
  const [viewMode, setViewModeState] = useState<ViewMode>(() =>
    viewModeFromParam(searchParams.get('view')),
  )

  // State initialised from URL params
  const [filters, setFiltersState] = useState<MemoryFiltersType>(() =>
    filtersFromParams(searchParams),
  )
  const [search, setSearchState] = useState(() => searchParams.get('search') || '')
  const [sortBy, setSortByState] = useState<MemorySortField>(() =>
    sortFieldFromParam(searchParams.get('sortBy')),
  )
  const [sortDir, setSortDirState] = useState<MemorySortDir>(() =>
    sortDirFromParam(searchParams.get('sortDir')),
  )
  const [page, setPageState] = useState(() => Number(searchParams.get('page')) || 1)

  // Sync state → URL
  useEffect(() => {
    const params: Record<string, string> = {}
    if (viewMode !== 'list') params['view'] = viewMode
    if (filters.type) params['type'] = filters.type
    if (filters.visibility) params['visibility'] = filters.visibility
    if (filters.created_by) params['created_by'] = filters.created_by
    if (filters.tag) params['tag'] = filters.tag
    if (search) params['search'] = search
    if (sortBy !== 'created_at') params['sortBy'] = sortBy
    if (sortDir !== 'desc') params['sortDir'] = sortDir
    if (page > 1) params['page'] = String(page)
    setSearchParams(params, { replace: true })
  }, [viewMode, filters, search, sortBy, sortDir, page, setSearchParams])

  // Reset page to 1 when filters change
  const setFilters = (f: MemoryFiltersType) => {
    setFiltersState(f)
    setPageState(1)
  }
  const setSearch = (s: string) => {
    setSearchState(s)
    setPageState(1)
  }
  const setSort = (field: MemorySortField, dir: MemorySortDir) => {
    setSortByState(field)
    setSortDirState(dir)
    setPageState(1)
  }
  const setViewMode = (mode: ViewMode) => {
    setViewModeState(mode)
    setPageState(1)
  }

  // Hook
  const {
    memories,
    total,
    loading,
    refreshing,
    error,
    refetch,
    createMemory,
    deleteMemory,
    // updateVisibility will be used when the visibility edit dialog is built
  } = useMemories({
    filters,
    search,
    page,
    pageSize: DEFAULT_PAGE_SIZE,
    sortBy,
    sortDir,
    // Pause auto-refresh when in search view (search has its own fetch)
    paused: viewMode === 'search',
  })

  const totalPages = Math.ceil(total / DEFAULT_PAGE_SIZE)

  // Create dialog state
  const [showCreateDialog, setShowCreateDialog] = useState(false)

  // Delete confirmation state
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null)
  const [deleteLoading, setDeleteLoading] = useState(false)

  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return
    setDeleteLoading(true)
    try {
      await deleteMemory(deleteTarget)
    } finally {
      setDeleteLoading(false)
      setDeleteTarget(null)
    }
  }

  // Edit visibility — placeholder for future dialog
  const handleEditVisibility = (_memory: Memory) => {
    // TODO: Open visibility edit dialog (future issue)
  }

  return (
    <main className="mx-auto max-w-4xl px-4 py-6">
      {/* Page header */}
      <div className="mb-6 flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold text-gray-900 dark:text-white">Memories</h1>
        {viewMode === 'list' && (
          <span className="rounded-full bg-gray-700 px-2.5 py-0.5 text-xs font-medium text-gray-300">
            {total}
          </span>
        )}
        <div className="flex-1" />

        {/* View mode toggle */}
        <div className="flex rounded-md border border-gray-600" role="group" aria-label="View mode">
          <button
            type="button"
            onClick={() => setViewMode('list')}
            aria-pressed={viewMode === 'list'}
            className={[
              'flex items-center gap-1.5 rounded-l-md px-3 py-1.5 text-xs font-medium transition-colors',
              viewMode === 'list'
                ? 'bg-primary-600 text-white'
                : 'bg-gray-800 text-gray-400 hover:text-white',
            ].join(' ')}
          >
            <List size={14} aria-hidden="true" />
            Browse
          </button>
          <button
            type="button"
            onClick={() => setViewMode('search')}
            aria-pressed={viewMode === 'search'}
            className={[
              'flex items-center gap-1.5 rounded-r-md px-3 py-1.5 text-xs font-medium transition-colors',
              viewMode === 'search'
                ? 'bg-primary-600 text-white'
                : 'bg-gray-800 text-gray-400 hover:text-white',
            ].join(' ')}
          >
            <Search size={14} aria-hidden="true" />
            Search
          </button>
        </div>

        {/* Create button */}
        <button
          type="button"
          onClick={() => setShowCreateDialog(true)}
          className="flex items-center gap-1.5 rounded-md bg-primary-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-primary-500 transition-colors"
        >
          <Plus size={14} aria-hidden="true" />
          New Memory
        </button>

        {/* Refresh (list mode only) */}
        {viewMode === 'list' && (
          <button
            type="button"
            onClick={refetch}
            aria-label="Refresh memories"
            className={[
              'rounded-md p-2 text-gray-400 hover:bg-gray-700 hover:text-white transition-colors',
              refreshing ? 'animate-spin' : '',
            ].join(' ')}
          >
            <RefreshCw size={16} />
          </button>
        )}
      </div>

      {/* ============================================================ */}
      {/* Search mode                                                  */}
      {/* ============================================================ */}
      {viewMode === 'search' && (
        <MemorySearch
          onSwitchToList={() => setViewMode('list')}
          onEditVisibility={handleEditVisibility}
          onDelete={setDeleteTarget}
        />
      )}

      {/* ============================================================ */}
      {/* List mode                                                    */}
      {/* ============================================================ */}
      {viewMode === 'list' && (
        <>
          {/* Filters row */}
          <div className="mb-4">
            <MemoryFilters
              filters={filters}
              sortBy={sortBy}
              sortDir={sortDir}
              search={search}
              onFiltersChange={setFilters}
              onSortChange={setSort}
              onSearchChange={setSearch}
            />
          </div>

          {/* Count summary */}
          {!loading && !error && memories.length > 0 && (
            <div className="mb-4 flex items-center rounded-lg border border-gray-700 bg-gray-800 px-4 py-2">
              <span className="text-xs text-gray-500">
                {total} memor{total !== 1 ? 'ies' : 'y'}
                {search && ` matching "${search}"`}
              </span>
            </div>
          )}

          {/* Loading state */}
          {loading && (
            <div className="space-y-3">
              {Array.from({ length: 3 }).map((_, i) => (
                <CardSkeleton key={i} />
              ))}
            </div>
          )}

          {/* Error state */}
          {!loading && error && (
            <div className="rounded-lg border border-red-800 bg-red-900/20 px-4 py-3 text-sm text-red-400">
              <p>{error}</p>
              <button
                type="button"
                onClick={refetch}
                className="mt-2 rounded-md px-3 py-1 text-xs font-medium bg-red-900/40 text-red-300 hover:bg-red-900/60 transition-colors"
              >
                Retry
              </button>
            </div>
          )}

          {/* Empty state */}
          {!loading && !error && memories.length === 0 && (
            <div className="py-16 text-center">
              <Brain size={40} className="mx-auto mb-3 text-gray-600" aria-hidden="true" />
              <p className="text-gray-400">No memories found</p>
              <p className="mt-1 text-xs text-gray-600">
                {search || filters.type || filters.visibility || filters.tag || filters.created_by
                  ? 'Try adjusting your filters or search query.'
                  : 'Get started by creating your first memory.'}
              </p>
              {!search && !filters.type && !filters.visibility && (
                <button
                  type="button"
                  onClick={() => setShowCreateDialog(true)}
                  className="mt-4 inline-flex items-center gap-1.5 rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-500 transition-colors"
                >
                  <Plus size={14} aria-hidden="true" />
                  Create Memory
                </button>
              )}
            </div>
          )}

          {/* Memory list */}
          {!loading && memories.length > 0 && (
            <ul className="space-y-3" aria-label="Memories">
              {memories.map((m) => (
                <li key={m.id}>
                  <MemoryCard
                    memory={m}
                    onEditVisibility={handleEditVisibility}
                    onDelete={setDeleteTarget}
                  />
                </li>
              ))}
            </ul>
          )}

          {/* Pagination */}
          {!loading && totalPages > 1 && (
            <div className="mt-6">
              <Pagination
                page={page}
                totalPages={totalPages}
                totalItems={total}
                pageSize={DEFAULT_PAGE_SIZE}
                onPageChange={setPageState}
              />
            </div>
          )}
        </>
      )}

      {/* Create memory dialog */}
      <CreateMemoryDialog
        open={showCreateDialog}
        onSave={createMemory}
        onClose={() => setShowCreateDialog(false)}
      />

      {/* Delete confirmation dialog */}
      <ConfirmDialog
        open={deleteTarget !== null}
        title="Delete memory"
        description="Are you sure you want to delete this memory? This action cannot be undone."
        confirmLabel="Delete"
        variant="danger"
        loading={deleteLoading}
        onConfirm={handleDeleteConfirm}
        onCancel={() => setDeleteTarget(null)}
      />
    </main>
  )
}

export default MemoryList
