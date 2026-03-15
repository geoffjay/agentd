/**
 * NotificationList — full notifications management page.
 *
 * Features:
 * - Tab navigation: All | Actionable | History
 * - Filter by status, priority, source
 * - Sort by newest, oldest, priority
 * - Bulk select, dismiss, delete, mark-all-viewed
 * - Response dialog for actionable notifications
 * - Drawer for notification details on row click
 * - URL query param sync for filters/tab/sort
 */

import { useCallback, useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { RefreshCw, Trash2 } from 'lucide-react'
import {
  useNotifications,
  type NotificationFilters,
  type NotificationTab,
  type SortOrder,
} from '@/hooks/useNotifications'
import { NotificationFilters as FiltersControl } from '@/components/notifications/NotificationFilters'
import { NotificationResponseDialog } from '@/components/notifications/NotificationResponseDialog'
import { NotificationBadge } from '@/components/notifications/NotificationBadge'
import { NotificationDetail } from '@/components/notifications/NotificationDetail'
import { StatusBadge } from '@/components/common/StatusBadge'
import { DataTable, DrawerProvider, useDrawer } from '@/components/common'
import type { ColumnDef, BulkAction } from '@/components/common'
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
// Helpers
// ---------------------------------------------------------------------------

const SOURCE_LABELS: Record<string, string> = {
  system: 'System',
  ask_service: 'Ask',
  agent_hook: 'Agent Hook',
  monitor_service: 'Monitor',
}

function formatRelativeTime(dateStr: string): string {
  const diffMs = Date.now() - new Date(dateStr).getTime()
  const diffSec = Math.floor(diffMs / 1000)
  if (diffSec < 60) return 'just now'
  const diffMin = Math.floor(diffSec / 60)
  if (diffMin < 60) return `${diffMin} min ago`
  const diffHour = Math.floor(diffMin / 60)
  if (diffHour < 24) return `${diffHour}h ago`
  const diffDay = Math.floor(diffHour / 24)
  return `${diffDay}d ago`
}

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
// Inner page (needs drawer context)
// ---------------------------------------------------------------------------

function NotificationListInner() {
  const [searchParams, setSearchParams] = useSearchParams()
  const { openDrawer, closeDrawer } = useDrawer()

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
    setSelectedIds([])
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
  const [selectedIds, setSelectedIds] = useState<string[]>([])

  // Keep selectedIds in sync when list changes
  useEffect(() => {
    const ids = new Set(notifications.map((n) => n.id))
    setSelectedIds((prev) => {
      const next = prev.filter((id) => ids.has(id))
      return next.length === prev.length ? prev : next
    })
  }, [notifications])

  // Response dialog state
  const [respondingTo, setRespondingTo] = useState<Notification | null>(null)
  const respondingBusy = respondingTo ? busyIds.has(respondingTo.id) : false

  // Row click → open drawer
  const handleRowClick = useCallback(
    (n: Notification) => {
      openDrawer(
        n.title,
        <NotificationDetail
          notification={n}
          busy={busyIds.has(n.id)}
          onView={(id) => {
            markViewed(id)
          }}
          onRespond={(notif) => {
            closeDrawer()
            setRespondingTo(notif)
          }}
          onDismiss={(id) => {
            dismiss(id)
            closeDrawer()
          }}
          onDelete={(id) => {
            remove(id)
            closeDrawer()
          }}
        />,
      )
    },
    [busyIds, markViewed, dismiss, remove, openDrawer, closeDrawer],
  )

  // Bulk handlers
  const handleBulkDismiss = async () => {
    if (!window.confirm(`Dismiss ${selectedIds.length} notification${selectedIds.length !== 1 ? 's' : ''}?`))
      return
    await bulkDismiss(selectedIds)
    setSelectedIds([])
  }

  const handleBulkDelete = async () => {
    if (!window.confirm(`Delete ${selectedIds.length} notification${selectedIds.length !== 1 ? 's' : ''}?`))
      return
    await bulkDelete(selectedIds)
    setSelectedIds([])
  }

  const handleMarkAllViewed = async () => {
    const pendingCount = notifications.filter((n) => n.status === 'pending').length
    if (pendingCount === 0) return
    if (!window.confirm(`Mark ${pendingCount} notification${pendingCount !== 1 ? 's' : ''} as viewed?`))
      return
    await markAllViewed()
  }

  const pendingCount = notifications.filter((n) => n.status === 'pending').length
  const someSelected = selectedIds.length > 0

  // Bulk actions
  const bulkActions: BulkAction[] = [
    {
      label: 'Dismiss selected',
      onClick: handleBulkDismiss,
    },
    {
      label: 'Delete selected',
      icon: <Trash2 size={12} />,
      onClick: handleBulkDelete,
      variant: 'danger',
    },
  ]

  // Column definitions
  const columns: ColumnDef<Notification>[] = [
    {
      key: 'title',
      header: 'Title',
      render: (n) => (
        <span className="text-sm font-medium text-gray-900 dark:text-white">{n.title}</span>
      ),
    },
    {
      key: 'source',
      header: 'Source',
      render: (n) => (
        <span className="text-sm text-gray-500 dark:text-gray-400">
          {SOURCE_LABELS[n.source.type] ?? n.source.type}
        </span>
      ),
    },
    {
      key: 'status',
      header: 'Status',
      render: (n) => <StatusBadge status={n.status} />,
    },
    {
      key: 'priority',
      header: 'Priority',
      render: (n) => (
        <span className="rounded-full bg-gray-100 px-2.5 py-0.5 text-xs font-medium capitalize text-gray-600 dark:bg-gray-800 dark:text-gray-400">
          {n.priority}
        </span>
      ),
    },
    {
      key: 'created_at',
      header: 'Created',
      render: (n) => (
        <span className="text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">
          {formatRelativeTime(n.created_at)}
        </span>
      ),
    },
  ]

  return (
    <div className="space-y-5">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Notifications</h1>
          <NotificationBadge count={pendingCount} showZero />
        </div>

        <div className="flex items-center gap-2">
          {/* Mark all viewed */}
          {pendingCount > 0 && (
            <button
              type="button"
              onClick={handleMarkAllViewed}
              className="rounded-md border border-gray-300 dark:border-gray-600 px-3 py-1.5 text-xs font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
            >
              Mark all viewed
            </button>
          )}

          {/* Refresh */}
          <button
            type="button"
            onClick={refetch}
            aria-label="Refresh notifications"
            className="rounded-md border border-gray-300 bg-white p-2 text-gray-500 hover:bg-gray-50 hover:text-gray-700 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-400 dark:hover:bg-gray-700 transition-colors"
          >
            <RefreshCw size={16} />
          </button>
        </div>
      </div>

      {/* Tab navigation */}
      <div className="border-b border-gray-200 dark:border-gray-700">
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
                  ? 'border-primary-500 text-primary-600 dark:text-primary-400'
                  : 'border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-white hover:border-gray-300 dark:hover:border-gray-500',
              ].join(' ')}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </div>

      {/* Filters row */}
      <FiltersControl
        filters={filters}
        sort={sort}
        onFiltersChange={setFilters}
        onSortChange={setSort}
      />

      {/* Error banner */}
      {error && (
        <div
          role="alert"
          className="rounded-md bg-red-50 px-4 py-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400"
        >
          {error}
        </div>
      )}

      {/* Table */}
      <DataTable
        columns={columns}
        data={notifications}
        rowKey={(n) => n.id}
        loading={loading}
        onRowClick={handleRowClick}
        emptyTitle={
          tab === 'actionable'
            ? 'No actionable notifications'
            : tab === 'history'
              ? 'No notification history'
              : 'No notifications'
        }
        emptyDescription={
          tab === 'actionable'
            ? 'No actionable notifications requiring your response.'
            : tab === 'history'
              ? 'No notification history yet.'
              : 'All caught up!'
        }
        selectable
        selectedIds={selectedIds}
        onSelectChange={setSelectedIds}
        bulkActions={bulkActions}
      />

      {/* Response dialog */}
      <NotificationResponseDialog
        notification={respondingTo}
        busy={respondingBusy}
        onSubmit={respond}
        onClose={() => setRespondingTo(null)}
      />
    </div>
  )
}

// ---------------------------------------------------------------------------
// Exported page (wraps with DrawerProvider)
// ---------------------------------------------------------------------------

export function NotificationList() {
  return (
    <DrawerProvider>
      <NotificationListInner />
    </DrawerProvider>
  )
}

export default NotificationList
