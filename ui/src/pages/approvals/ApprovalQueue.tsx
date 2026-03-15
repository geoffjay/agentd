/**
 * ApprovalQueue — global tool-approval queue page.
 *
 * Keyboard shortcuts (when no input focused):
 *   A  →  Approve selected
 *   D  →  Deny selected
 */

import { useCallback, useEffect, useState } from 'react'
import { Check, RefreshCw, X } from 'lucide-react'
import { useApprovals } from '@/hooks/useApprovals'
import { ApprovalBadge } from '@/components/approvals/ApprovalBadge'
import { ApprovalDetail } from '@/components/approvals/ApprovalDetail'
import { DataTable, DrawerProvider, useDrawer } from '@/components/common'
import type { ColumnDef, BulkAction } from '@/components/common'
import type { PendingApproval } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function minutesWaiting(createdAt: string): number {
  return Math.floor((Date.now() - new Date(createdAt).getTime()) / 60_000)
}

function urgencyColor(minutes: number): string {
  if (minutes >= 10) return 'text-red-400'
  if (minutes >= 5) return 'text-yellow-400'
  return 'text-green-400'
}

// ---------------------------------------------------------------------------
// Inner page (needs drawer context)
// ---------------------------------------------------------------------------

function ApprovalQueueInner() {
  const {
    approvals,
    totalPendingCount,
    loading,
    error,
    agentMap,
    busyIds,
    refetch,
    approve,
    deny,
    bulkApprove,
    bulkDeny,
  } = useApprovals({ browserNotifications: true })

  const { openDrawer, closeDrawer } = useDrawer()

  const [selectedIds, setSelectedIds] = useState<string[]>([])
  const [filterAgentId, setFilterAgentId] = useState<string>('')

  // Keep selectedIds clean when approvals list changes
  useEffect(() => {
    const ids = new Set(approvals.map((a) => a.id))
    setSelectedIds((prev) => prev.filter((id) => ids.has(id)))
  }, [approvals])

  // Filtered approvals
  const visible = filterAgentId ? approvals.filter((a) => a.agent_id === filterAgentId) : approvals

  const someSelected = selectedIds.length > 0

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName
      if (['INPUT', 'TEXTAREA', 'SELECT'].includes(tag)) return
      if (e.metaKey || e.ctrlKey || e.altKey) return

      if (e.key === 'a' || e.key === 'A') {
        e.preventDefault()
        if (someSelected) bulkApprove(selectedIds)
      }
      if (e.key === 'd' || e.key === 'D') {
        e.preventDefault()
        if (someSelected) bulkDeny(selectedIds)
      }
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [someSelected, selectedIds, bulkApprove, bulkDeny])

  // Row click → open drawer
  const handleRowClick = useCallback(
    (approval: PendingApproval) => {
      openDrawer(
        approval.tool_name,
        <ApprovalDetail
          approval={approval}
          agentName={agentMap.get(approval.agent_id)?.name}
          busy={busyIds.has(approval.id)}
          onApprove={(id) => {
            approve(id)
            closeDrawer()
          }}
          onDeny={(id) => {
            deny(id)
            closeDrawer()
          }}
        />,
      )
    },
    [agentMap, busyIds, approve, deny, openDrawer, closeDrawer],
  )

  // Unique agents for filter dropdown
  const agentOptions = [
    ...new Map(approvals.map((a) => [a.agent_id, agentMap.get(a.agent_id)])).entries(),
  ]

  // Bulk actions
  const bulkActions: BulkAction[] = [
    {
      label: 'Approve selected (A)',
      icon: <Check size={12} />,
      onClick: () => bulkApprove(selectedIds),
      variant: 'success',
    },
    {
      label: 'Deny selected (D)',
      icon: <X size={12} />,
      onClick: () => bulkDeny(selectedIds),
      variant: 'danger',
    },
  ]

  // Column definitions
  const columns: ColumnDef<PendingApproval>[] = [
    {
      key: 'tool_name',
      header: 'Tool',
      render: (a) => (
        <span className="font-mono text-sm font-semibold text-gray-900 dark:text-white">
          {a.tool_name}
        </span>
      ),
    },
    {
      key: 'agent',
      header: 'Agent',
      render: (a) => {
        const agent = agentMap.get(a.agent_id)
        return (
          <span className="text-sm text-gray-700 dark:text-gray-300">
            {agent?.name ?? a.agent_id}
          </span>
        )
      },
    },
    {
      key: 'urgency',
      header: 'Wait Time',
      render: (a) => {
        const mins = minutesWaiting(a.created_at)
        return (
          <span className={['text-sm font-medium', urgencyColor(mins)].join(' ')}>
            {mins < 1 ? 'Just now' : `${mins}m ago`}
          </span>
        )
      },
    },
    {
      key: 'status',
      header: 'Status',
      render: (a) => (
        <span className="text-sm capitalize text-gray-500 dark:text-gray-400">{a.status}</span>
      ),
    },
    {
      key: 'created_at',
      header: 'Created',
      render: (a) => (
        <span className="text-sm text-gray-500 dark:text-gray-400 whitespace-nowrap">
          {new Date(a.created_at).toLocaleString()}
        </span>
      ),
    },
    {
      key: 'actions',
      header: '',
      render: (a) => (
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          <button
            type="button"
            disabled={busyIds.has(a.id)}
            onClick={() => approve(a.id)}
            className="rounded-md px-2.5 py-1 text-xs font-medium bg-green-600 text-white hover:bg-green-700 disabled:opacity-50 transition-colors"
          >
            Approve
          </button>
          <button
            type="button"
            disabled={busyIds.has(a.id)}
            onClick={() => deny(a.id)}
            className="rounded-md px-2.5 py-1 text-xs font-medium bg-red-600 text-white hover:bg-red-700 disabled:opacity-50 transition-colors"
          >
            Deny
          </button>
        </div>
      ),
    },
  ]

  return (
    <div className="space-y-5">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Approval Queue</h1>
          <ApprovalBadge count={totalPendingCount} showZero />
        </div>

        <div className="flex items-center gap-2">
          {/* Filter by agent */}
          {agentOptions.length > 1 && (
            <select
              aria-label="Filter by agent"
              value={filterAgentId}
              onChange={(e) => setFilterAgentId(e.target.value)}
              className="rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-3 py-1.5 text-sm text-gray-900 dark:text-gray-300 focus:outline-none focus:ring-2 focus:ring-primary-500"
            >
              <option value="">All agents</option>
              {agentOptions.map(([id, agent]) => (
                <option key={id} value={id}>
                  {agent?.name ?? id}
                </option>
              ))}
            </select>
          )}

          {/* Refresh */}
          <button
            type="button"
            onClick={refetch}
            aria-label="Refresh approvals"
            className="rounded-md border border-gray-300 bg-white p-2 text-gray-500 hover:bg-gray-50 hover:text-gray-700 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-400 dark:hover:bg-gray-700 transition-colors"
          >
            <RefreshCw size={16} />
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

      {/* Table */}
      <DataTable
        columns={columns}
        data={visible}
        rowKey={(a) => a.id}
        loading={loading}
        onRowClick={handleRowClick}
        emptyTitle="No pending approvals"
        emptyDescription="All caught up!"
        selectable
        selectedIds={selectedIds}
        onSelectChange={setSelectedIds}
        bulkActions={bulkActions}
      />
    </div>
  )
}

// ---------------------------------------------------------------------------
// Exported page (wraps with DrawerProvider)
// ---------------------------------------------------------------------------

export function ApprovalQueuePage() {
  return (
    <DrawerProvider>
      <ApprovalQueueInner />
    </DrawerProvider>
  )
}

export default ApprovalQueuePage
