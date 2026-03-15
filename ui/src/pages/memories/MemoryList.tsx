/**
 * MemoryList — full memory management page.
 *
 * Features:
 * - Tab toggle between Browse (list) and Semantic Search modes
 * - Filter by type, visibility, creator, tag, and content search
 * - Sort by created_at, updated_at, type
 * - Paginated table of memories
 * - Create memory dialog
 * - Delete confirmation dialog
 * - Drawer for memory details on row click
 * - URL query param sync for filters/sort/pagination
 * - Loading skeleton, error state, and empty state
 */

import { useCallback, useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Globe, List, Lock, Plus, RefreshCw, Search, Users } from 'lucide-react'
import {
  useMemories,
  type MemoryFilters as MemoryFiltersType,
  type MemorySortField,
  type MemorySortDir,
} from '@/hooks/useMemories'
import { MemoryFilters } from '@/components/memories/MemoryFilters'
import { MemorySearch } from '@/components/memories/MemorySearch'
import { MemoryDetail } from '@/components/memories/MemoryDetail'
import { CreateMemoryDialog } from '@/components/memories/CreateMemoryDialog'
import { Pagination } from '@/components/common/Pagination'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'
import { DataTable, DrawerProvider, useDrawer } from '@/components/common'
import type { ColumnDef } from '@/components/common'
import type { Memory, VisibilityLevel } from '@/types/memory'

// ---------------------------------------------------------------------------
// View mode
// ---------------------------------------------------------------------------

type ViewMode = 'list' | 'search'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const TYPE_STYLES: Record<string, string> = {
  information: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400',
  question: 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400',
  request: 'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400',
}

const TYPE_LABELS: Record<string, string> = {
  information: 'Information',
  question: 'Question',
  request: 'Request',
}

const VISIBILITY_STYLES: Record<string, string> = {
  public: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
  shared: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
  private: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
}

const VISIBILITY_ICONS: Record<VisibilityLevel, React.ReactNode> = {
  public: <Globe size={12} aria-hidden="true" />,
  shared: <Users size={12} aria-hidden="true" />,
  private: <Lock size={12} aria-hidden="true" />,
}

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
// Inner page (needs drawer context)
// ---------------------------------------------------------------------------

function MemoryListInner() {
  const [searchParams, setSearchParams] = useSearchParams()
  const { openDrawer, closeDrawer } = useDrawer()

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
  } = useMemories({
    filters,
    search,
    page,
    pageSize: DEFAULT_PAGE_SIZE,
    sortBy,
    sortDir,
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

  // Row click → open drawer
  const handleRowClick = useCallback(
    (m: Memory) => {
      openDrawer(
        m.content.length > 50 ? m.content.slice(0, 50) + '…' : m.content,
        <MemoryDetail
          memory={m}
          onEditVisibility={(mem) => {
            closeDrawer()
            handleEditVisibility(mem)
          }}
          onDelete={(id) => {
            closeDrawer()
            setDeleteTarget(id)
          }}
        />,
      )
    },
    [openDrawer, closeDrawer],
  )

  // Handle sort from DataTable
  const handleTableSort = useCallback(
    (field: string) => {
      if (field === sortBy) {
        setSort(field as MemorySortField, sortDir === 'asc' ? 'desc' : 'asc')
      } else {
        setSort(field as MemorySortField, 'desc')
      }
    },
    [sortBy, sortDir],
  )

  // Column definitions
  const columns: ColumnDef<Memory>[] = [
    {
      key: 'type',
      header: 'Type',
      render: (m) => (
        <span
          className={[
            'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium',
            TYPE_STYLES[m.type] ?? 'bg-gray-100 text-gray-600',
          ].join(' ')}
        >
          {TYPE_LABELS[m.type] ?? m.type}
        </span>
      ),
    },
    {
      key: 'content',
      header: 'Content',
      render: (m) => (
        <span className="text-sm text-gray-700 dark:text-gray-300 line-clamp-2">
          {m.content}
        </span>
      ),
    },
    {
      key: 'visibility',
      header: 'Visibility',
      render: (m) => (
        <span
          className={[
            'inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium',
            VISIBILITY_STYLES[m.visibility] ?? 'bg-gray-100 text-gray-600',
          ].join(' ')}
        >
          {VISIBILITY_ICONS[m.visibility]}
          {m.visibility}
        </span>
      ),
    },
    {
      key: 'tags',
      header: 'Tags',
      render: (m) =>
        m.tags.length > 0 ? (
          <div className="flex flex-wrap gap-1">
            {m.tags.slice(0, 3).map((tag) => (
              <span
                key={tag}
                className="rounded-full bg-gray-100 px-2 py-0.5 text-xs font-medium text-gray-700 dark:bg-gray-800 dark:text-gray-300"
              >
                {tag}
              </span>
            ))}
            {m.tags.length > 3 && (
              <span className="text-xs text-gray-400">+{m.tags.length - 3}</span>
            )}
          </div>
        ) : (
          <span className="text-xs text-gray-400">—</span>
        ),
    },
    {
      key: 'created_by',
      header: 'Creator',
      render: (m) => (
        <span className="text-sm text-gray-500 dark:text-gray-400">{m.created_by}</span>
      ),
    },
    {
      key: 'created_at',
      header: 'Created',
      sortable: true,
      render: (m) => (
        <span className="text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">
          {new Date(m.created_at).toLocaleDateString()}
        </span>
      ),
    },
  ]

  return (
    <div className="space-y-5">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Memories</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
            Manage stored knowledge and context.
            {viewMode === 'list' && total > 0 && (
              <span className="ml-2 text-gray-400">({total} total)</span>
            )}
          </p>
        </div>

        <div className="flex items-center gap-2">
          {/* View mode toggle */}
          <div className="flex rounded-md border border-gray-300 dark:border-gray-600" role="group" aria-label="View mode">
            <button
              type="button"
              onClick={() => setViewMode('list')}
              aria-pressed={viewMode === 'list'}
              className={[
                'flex items-center gap-1.5 rounded-l-md px-3 py-1.5 text-xs font-medium transition-colors',
                viewMode === 'list'
                  ? 'bg-primary-600 text-white'
                  : 'bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-white',
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
                  : 'bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-white',
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
            className="flex items-center gap-1.5 rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 transition-colors"
          >
            <Plus size={16} aria-hidden="true" />
            New Memory
          </button>

          {/* Refresh (list mode only) */}
          {viewMode === 'list' && (
            <button
              type="button"
              onClick={refetch}
              aria-label="Refresh memories"
              className={[
                'rounded-md border border-gray-300 bg-white p-2 text-gray-500 hover:bg-gray-50 hover:text-gray-700 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-400 dark:hover:bg-gray-700 transition-colors',
                refreshing ? 'animate-spin' : '',
              ].join(' ')}
            >
              <RefreshCw size={16} />
            </button>
          )}
        </div>
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
          <MemoryFilters
            filters={filters}
            sortBy={sortBy}
            sortDir={sortDir}
            search={search}
            onFiltersChange={setFilters}
            onSortChange={setSort}
            onSearchChange={setSearch}
          />

          {/* Error banner */}
          {error && (
            <div
              role="alert"
              className="rounded-md bg-red-50 px-4 py-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400"
            >
              <p>{error}</p>
              <button
                type="button"
                onClick={refetch}
                className="mt-2 rounded-md px-3 py-1 text-xs font-medium bg-red-100 dark:bg-red-900/40 text-red-700 dark:text-red-300 hover:bg-red-200 dark:hover:bg-red-900/60 transition-colors"
              >
                Retry
              </button>
            </div>
          )}

          {/* Table */}
          <DataTable
            columns={columns}
            data={memories}
            rowKey={(m) => m.id}
            loading={loading}
            sortBy={sortBy}
            sortDir={sortDir}
            onSort={handleTableSort}
            onRowClick={handleRowClick}
            emptyTitle="No memories found"
            emptyDescription={
              search || filters.type || filters.visibility || filters.tag || filters.created_by
                ? 'Try adjusting your filters or search query.'
                : 'Get started by creating your first memory.'
            }
          />

          {/* Pagination */}
          {!loading && totalPages > 1 && (
            <Pagination
              page={page}
              totalPages={totalPages}
              totalItems={total}
              pageSize={DEFAULT_PAGE_SIZE}
              onPageChange={setPageState}
            />
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
    </div>
  )
}

// ---------------------------------------------------------------------------
// Exported page (wraps with DrawerProvider)
// ---------------------------------------------------------------------------

export function MemoryList() {
  return (
    <DrawerProvider>
      <MemoryListInner />
    </DrawerProvider>
  )
}

export default MemoryList
