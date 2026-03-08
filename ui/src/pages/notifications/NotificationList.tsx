/**
 * NotificationList — full notifications management page.
 *
 * Features:
 * - Tab navigation: All | Actionable | History
 * - Filter by status, priority, source
 * - Sort by newest, oldest, priority
 * - Bulk select, dismiss, delete, mark-all-viewed
 * - Response dialog for actionable notifications
 * - URL query param sync for filters/tab/sort
 */

import { useCallback, useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Bell, CheckSquare, RefreshCw, Square, Trash2 } from 'lucide-react'
import {
  useNotifications,
  type NotificationFilters,
  type NotificationTab,
  type SortOrder,
} from '@/hooks/useNotifications'
import { NotificationCard } from '@/components/notifications/NotificationCard'
import { NotificationFilters as FiltersControl } from '@/components/notifications/NotificationFilters'
import { NotificationResponseDialog } from '@/components/notifications/NotificationResponseDialog'
import { NotificationBadge } from '@/components/notifications/NotificationBadge'
import type { Notification } from '@/types/notify'

// ---------------------------------------------------------------------------
// Tab definitions
// ---------------------------------------------------------------------------

const TABS: Array<{ value: NotificationTab; label: string }> = [
  { value: 'all', label: 'All' },
  { value: 'actionable', label: 'Actionable' },
  { value: 'history', label: 'History' },
]

// ---------------------------------------------------------------------------
// URL sync helpers
// ---------------------------------------------------------------------------

function tabFromParam(p: string | null): NotificationTab {
  if (p === 'actionable' || p === 'history') return p
  return 'all'
}

function sortFromParam(p: string | null): SortOrder {
  if (p === 'oldest' || p === 'priority') return p
  return 'newest'
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function NotificationList() {
  const [searchParams, setSearchParams] = useSearchParams()

  const [tab, setTabState] = useState<NotificationTab>(() =>
    tabFromParam(searchParams.get('tab')),
  )
  const [sort, setSortState] = useState<SortOrder>(() =>
    sortFromParam(searchParams.get('sort')),
  )
  const [filters, setFiltersState] = useState<NotificationFilters>({
    status: (searchParams.get('status') as NotificationFilters['status']) ?? 'All',
    priority: (searchParams.get('priority') as NotificationFilters['priority']) ?? 'All',
    source: (searchParams.get('source') as NotificationFilters['source']) ?? 'All',
  })

  // Sync state → URL
  useEffect(() => {
    const params: Record<string, string> = {}
    if (tab !== 'all') params['tab'] = tab
    if (sort !== 'newest') params['sort'] = sort
    if (filters.status && filters.status !== 'All') params['status'] = filters.status
    if (filters.priority && filters.priority !== 'All') params['priority'] = filters.priority
    if (filters.source && filters.source !== 'All') params['source'] = filters.source
    setSearchParams(params, { replace: true })
  }, [tab, sort, filters, setSearchParams])

  const setTab = (t: NotificationTab) => {
    setTabState(t)
    setSelectedIds(new Set())
  }

  const setSort = (s: SortOrder) => setSortState(s)
  const setFilters = (f: NotificationFilters) => setFiltersState(f)

  const {
    notifications,
    loading,
    error,
    busyIds,
    refetch,
    markViewed,
    respond,
    dismiss,
    remove,
    bulkDismiss,
    bulkDelete,
    markAllViewed,
  } = useNotifications({ tab, filters, sort })

  // Selection state
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())

  // Keep selectedIds in sync when list changes
  useEffect(() => {
    const ids = new Set(notifications.map((n) => n.id))
    setSelectedIds((prev) => {
      const next = new Set<string>()
      for (const id of prev) {
        if (ids.has(id)) next.add(id)
      }
      return next
    })
  }, [notifications])

  const allSelected =
    notifications.length > 0 && notifications.every((n) => selectedIds.has(n.id))
  const someSelected = selectedIds.size > 0

  const toggleSelect = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }, [])

  const toggleAll = () => {
    if (allSelected) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(notifications.map((n) => n.id)))
    }
  }

  // Response dialog state
  const [respondingTo, setRespondingTo] = useState<Notification | null>(null)
  const respondingBusy = respondingTo ? busyIds.has(respondingTo.id) : false

  // Bulk handlers
  const handleBulkDismiss = async () => {
    const ids = [...selectedIds]
    if (!window.confirm(`Dismiss ${ids.length} notification${ids.length !== 1 ? 's' : ''}?`))
      return
    await bulkDismiss(ids)
    setSelectedIds(new Set())
  }

  const handleBulkDelete = async () => {
    const ids = [...selectedIds]
    if (!window.confirm(`Delete ${ids.length} notification${ids.length !== 1 ? 's' : ''}?`))
      return
    await bulkDelete(ids)
    setSelectedIds(new Set())
  }

  const handleMarkAllViewed = async () => {
    const pendingCount = notifications.filter((n) => n.status === 'pending').length
    if (pendingCount === 0) return
    if (!window.confirm(`Mark ${pendingCount} notification${pendingCount !== 1 ? 's' : ''} as viewed?`))
      return
    await markAllViewed()
  }

  const pendingCount = notifications.filter((n) => n.status === 'pending').length

  return (
    <main className="mx-auto max-w-4xl px-4 py-6">
      {/* Page header */}
      <div className="mb-6 flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold text-white">Notifications</h1>
        <NotificationBadge count={pendingCount} showZero />
        <div className="flex-1" />

        {/* Mark all viewed */}
        {pendingCount > 0 && (
          <button
            type="button"
            onClick={handleMarkAllViewed}
            className="rounded-md px-3 py-1.5 text-xs font-medium text-gray-300 bg-gray-700 hover:bg-gray-600 transition-colors"
          >
            Mark all viewed
          </button>
        )}

        {/* Refresh */}
        <button
          type="button"
          onClick={refetch}
          aria-label="Refresh notifications"
          className="rounded-md p-2 text-gray-400 hover:bg-gray-700 hover:text-white transition-colors"
        >
          <RefreshCw size={16} />
        </button>
      </div>

      {/* Tab navigation */}
      <div className="mb-4 border-b border-gray-700">
        <nav className="-mb-px flex gap-1" aria-label="Notification tabs">
          {TABS.map((t) => (
            <button
              key={t.value}
              type="button"
              role="tab"
              aria-selected={tab === t.value}
              onClick={() => setTab(t.value)}
              className={[
                'rounded-t-md px-4 py-2 text-sm font-medium transition-colors border-b-2',
                tab === t.value
                  ? 'border-primary-500 text-primary-400'
                  : 'border-transparent text-gray-400 hover:text-white hover:border-gray-500',
              ].join(' ')}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </div>

      {/* Filters row */}
      <div className="mb-4">
        <FiltersControl
          filters={filters}
          sort={sort}
          onFiltersChange={setFilters}
          onSortChange={setSort}
        />
      </div>

      {/* Bulk action toolbar */}
      {notifications.length > 0 && (
        <div className="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-gray-700 bg-gray-800 px-4 py-2">
          <button
            type="button"
            onClick={toggleAll}
            aria-label={allSelected ? 'Deselect all' : 'Select all'}
            className="flex items-center gap-1.5 text-sm text-gray-400 hover:text-white"
          >
            {allSelected ? <CheckSquare size={16} /> : <Square size={16} />}
            {allSelected ? 'Deselect all' : 'Select all'}
          </button>

          {someSelected && (
            <>
              <span className="text-gray-600">|</span>
              <span className="text-xs text-gray-400">{selectedIds.size} selected</span>
              <button
                type="button"
                onClick={handleBulkDismiss}
                className="flex items-center gap-1 rounded-md px-2.5 py-1 text-xs font-medium bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
              >
                Dismiss selected
              </button>
              <button
                type="button"
                onClick={handleBulkDelete}
                className="flex items-center gap-1 rounded-md px-2.5 py-1 text-xs font-medium bg-red-900/40 text-red-400 hover:bg-red-900/70 transition-colors"
              >
                <Trash2 size={12} aria-hidden="true" />
                Delete selected
              </button>
            </>
          )}

          <div className="flex-1" />

          <span className="text-xs text-gray-500">
            {notifications.length} notification{notifications.length !== 1 ? 's' : ''}
          </span>
        </div>
      )}

      {/* States */}
      {loading && (
        <p className="py-12 text-center text-sm text-gray-400">Loading notifications…</p>
      )}

      {!loading && error && (
        <div className="rounded-lg border border-red-800 bg-red-900/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      )}

      {!loading && !error && notifications.length === 0 && (
        <div className="py-16 text-center">
          <Bell size={40} className="mx-auto mb-3 text-gray-600" aria-hidden="true" />
          <p className="text-gray-400">No notifications</p>
          <p className="mt-1 text-xs text-gray-600">
            {tab === 'actionable'
              ? 'No actionable notifications requiring your response.'
              : tab === 'history'
                ? 'No notification history yet.'
                : 'All caught up!'}
          </p>
        </div>
      )}

      {/* Notification list */}
      {!loading && notifications.length > 0 && (
        <ul className="space-y-3" aria-label="Notifications">
          {notifications.map((n) => (
            <li key={n.id}>
              <NotificationCard
                notification={n}
                busy={busyIds.has(n.id)}
                selected={selectedIds.has(n.id)}
                onView={markViewed}
                onRespond={setRespondingTo}
                onDismiss={dismiss}
                onDelete={remove}
                onToggleSelect={toggleSelect}
              />
            </li>
          ))}
        </ul>
      )}

      {/* Response dialog */}
      <NotificationResponseDialog
        notification={respondingTo}
        busy={respondingBusy}
        onSubmit={respond}
        onClose={() => setRespondingTo(null)}
      />
    </main>
  )
}

export default NotificationList
