/**
 * AgentList — main agents list page.
 *
 * Composes:
 * - AgentFilters (status dropdown + name search)
 * - AgentTable (sortable, paginated, bulk-selectable)
 * - Pagination
 * - CreateAgentDialog (slide-over)
 *
 * URL query params are synced with the filter / page / sort state so
 * bookmarking and back-navigation work as expected.
 */

import { useCallback, useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Plus, RefreshCw } from 'lucide-react'
import { AgentFilters } from '@/components/agents/AgentFilters'
import { AgentTable } from '@/components/agents/AgentTable'
import { Pagination } from '@/components/common/Pagination'
import { CreateAgentDialog } from './CreateAgentDialog'
import { useAgents } from '@/hooks/useAgents'
import type { AgentStatus } from '@/types/orchestrator'
import type { SortDir, SortField } from '@/hooks/useAgents'

// ---------------------------------------------------------------------------
// URL param helpers
// ---------------------------------------------------------------------------

function toStatus(raw: string | null): AgentStatus | '' {
  const valid: AgentStatus[] = ['Running', 'Pending', 'Stopped', 'Failed']
  return valid.includes(raw as AgentStatus) ? (raw as AgentStatus) : ''
}

function toPage(raw: string | null, fallback = 1): number {
  const n = Number(raw)
  return Number.isFinite(n) && n >= 1 ? Math.floor(n) : fallback
}

function toSortField(raw: string | null): SortField {
  const valid: SortField[] = ['name', 'status', 'created_at']
  return valid.includes(raw as SortField) ? (raw as SortField) : 'created_at'
}

function toSortDir(raw: string | null): SortDir {
  return raw === 'asc' ? 'asc' : 'desc'
}

// ---------------------------------------------------------------------------
// Page size constant (not user-configurable for now)
// ---------------------------------------------------------------------------

const PAGE_SIZE = 20

// ---------------------------------------------------------------------------
// AgentList
// ---------------------------------------------------------------------------

export function AgentList() {
  const [searchParams, setSearchParams] = useSearchParams()

  // Derive filter/sort/page state from URL params
  const status = toStatus(searchParams.get('status'))
  const search = searchParams.get('q') ?? ''
  const page = toPage(searchParams.get('page'))
  const sortBy = toSortField(searchParams.get('sort'))
  const sortDir = toSortDir(searchParams.get('dir'))

  // Dialog open state; also pauses auto-refresh
  const [dialogOpen, setDialogOpen] = useState(false)

  // Selection state (for bulk operations)
  const [selectedIds, setSelectedIds] = useState<string[]>([])

  const {
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
  } = useAgents({
    status,
    search,
    page,
    pageSize: PAGE_SIZE,
    sortBy,
    sortDir,
    paused: dialogOpen,
  })

  // Clear selection whenever the list data changes
  useEffect(() => {
    setSelectedIds([])
  }, [status, search, page, sortBy, sortDir])

  // ---------------------------------------------------------------------------
  // URL update helpers
  // ---------------------------------------------------------------------------

  const setParam = useCallback(
    (updates: Record<string, string | null>) => {
      setSearchParams(
        prev => {
          const next = new URLSearchParams(prev)
          for (const [k, v] of Object.entries(updates)) {
            if (v === null || v === '') {
              next.delete(k)
            } else {
              next.set(k, v)
            }
          }
          return next
        },
        { replace: true },
      )
    },
    [setSearchParams],
  )

  function handleStatusChange(newStatus: AgentStatus | '') {
    setParam({ status: newStatus, page: null })
  }

  function handleSearchChange(newSearch: string) {
    setParam({ q: newSearch, page: null })
  }

  function handlePageChange(newPage: number) {
    setParam({ page: String(newPage) })
  }

  function handleSort(field: SortField) {
    const newDir: SortDir =
      field === sortBy ? (sortDir === 'asc' ? 'desc' : 'asc') : 'desc'
    setParam({ sort: field, dir: newDir, page: null })
  }

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  async function handleCreate(request: Parameters<typeof createAgent>[0]) {
    await createAgent(request)
  }

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  return (
    <div className="space-y-5">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">
            Agents
          </h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
            Manage and monitor running agents.
          </p>
        </div>

        <div className="flex items-center gap-2">
          {/* Refresh indicator */}
          {refreshing && (
            <RefreshCw
              size={14}
              aria-label="Refreshing…"
              className="animate-spin text-gray-400"
            />
          )}

          {/* Manual refresh button */}
          <button
            type="button"
            aria-label="Refresh agents list"
            onClick={refetch}
            disabled={loading}
            className="rounded-md border border-gray-300 bg-white p-2 text-gray-500 hover:bg-gray-50 hover:text-gray-700 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1 disabled:opacity-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-400 dark:hover:bg-gray-700"
          >
            <RefreshCw size={15} />
          </button>

          {/* Create agent button */}
          <button
            type="button"
            onClick={() => setDialogOpen(true)}
            className="flex items-center gap-1.5 rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1 transition-colors"
          >
            <Plus size={16} aria-hidden="true" />
            Create Agent
          </button>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div
          role="alert"
          className="rounded-md bg-red-50 px-4 py-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400"
        >
          {error}
        </div>
      )}

      {/* Filter toolbar */}
      <AgentFilters
        status={status}
        onStatusChange={handleStatusChange}
        search={search}
        onSearchChange={handleSearchChange}
        displayCount={agents.length}
        totalCount={allAgents.length}
      />

      {/* Agent table */}
      <AgentTable
        agents={agents}
        loading={loading}
        sortBy={sortBy}
        sortDir={sortDir}
        onSort={handleSort}
        onDelete={deleteAgent}
        onBulkDelete={bulkDelete}
        selectedIds={selectedIds}
        onSelectChange={setSelectedIds}
      />

      {/* Pagination */}
      {!loading && total > PAGE_SIZE && (
        <Pagination
          page={page}
          totalPages={totalPages}
          totalItems={total}
          pageSize={PAGE_SIZE}
          onPageChange={handlePageChange}
        />
      )}

      {/* Create agent dialog */}
      <CreateAgentDialog
        open={dialogOpen}
        onClose={() => setDialogOpen(false)}
        onCreate={handleCreate}
      />
    </div>
  )
}

export default AgentList
