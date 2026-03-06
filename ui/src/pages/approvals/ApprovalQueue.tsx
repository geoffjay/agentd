/**
 * ApprovalQueue — global tool-approval queue page.
 *
 * Keyboard shortcuts (when no input focused):
 *   A  →  Approve selected
 *   D  →  Deny selected
 */

import { useCallback, useEffect, useState } from 'react'
import { CheckSquare, RefreshCw, Square } from 'lucide-react'
import { useApprovals } from '@/hooks/useApprovals'
import { ApprovalBadge } from '@/components/approvals/ApprovalBadge'
import { ApprovalCard } from '@/components/approvals/ApprovalCard'

export function ApprovalQueuePage() {
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

  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [filterAgentId, setFilterAgentId] = useState<string>('')

  // Keep selectedIds clean when approvals list changes
  useEffect(() => {
    const ids = new Set(approvals.map((a) => a.id))
    setSelectedIds((prev) => {
      const next = new Set<string>()
      for (const id of prev) {
        if (ids.has(id)) next.add(id)
      }
      return next
    })
  }, [approvals])

  // Filtered approvals
  const visible = filterAgentId ? approvals.filter((a) => a.agent_id === filterAgentId) : approvals

  // Selection helpers
  const allSelected = visible.length > 0 && visible.every((a) => selectedIds.has(a.id))
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
      setSelectedIds(new Set(visible.map((a) => a.id)))
    }
  }

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName
      if (['INPUT', 'TEXTAREA', 'SELECT'].includes(tag)) return
      if (e.metaKey || e.ctrlKey || e.altKey) return

      if (e.key === 'a' || e.key === 'A') {
        e.preventDefault()
        if (someSelected) bulkApprove([...selectedIds])
      }
      if (e.key === 'd' || e.key === 'D') {
        e.preventDefault()
        if (someSelected) bulkDeny([...selectedIds])
      }
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [someSelected, selectedIds, bulkApprove, bulkDeny])

  // Bulk handlers
  const handleApproveAll = async () => {
    const ids = visible.map((a) => a.id)
    if (!window.confirm(`Approve all ${ids.length} pending approvals?`)) return
    await bulkApprove(ids)
  }

  const handleDenyAll = async () => {
    const ids = visible.map((a) => a.id)
    if (!window.confirm(`Deny all ${ids.length} pending approvals?`)) return
    await bulkDeny(ids)
  }

  const handleApproveSelected = () => bulkApprove([...selectedIds])
  const handleDenySelected = () => bulkDeny([...selectedIds])

  // Unique agents for filter dropdown
  const agentOptions = [
    ...new Map(approvals.map((a) => [a.agent_id, agentMap.get(a.agent_id)])).entries(),
  ]

  return (
    <main className="mx-auto max-w-4xl px-4 py-6">
      {/* Page header */}
      <div className="mb-6 flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-semibold text-white">Approval Queue</h1>
        <ApprovalBadge count={totalPendingCount} showZero />
        <div className="flex-1" />

        {/* Refresh */}
        <button
          type="button"
          onClick={refetch}
          aria-label="Refresh approvals"
          className="rounded-md p-2 text-gray-400 hover:bg-gray-700 hover:text-white"
        >
          <RefreshCw size={16} />
        </button>

        {/* Filter by agent */}
        {agentOptions.length > 1 && (
          <select
            aria-label="Filter by agent"
            value={filterAgentId}
            onChange={(e) => setFilterAgentId(e.target.value)}
            className="rounded-md border border-gray-600 bg-gray-800 px-3 py-1.5 text-sm text-gray-300 focus:outline-none focus:ring-2 focus:ring-primary-500"
          >
            <option value="">All agents</option>
            {agentOptions.map(([id, agent]) => (
              <option key={id} value={id}>
                {agent?.name ?? id}
              </option>
            ))}
          </select>
        )}
      </div>

      {/* Bulk action toolbar */}
      {visible.length > 0 && (
        <div className="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-gray-700 bg-gray-800 px-4 py-2">
          {/* Select all */}
          <button
            type="button"
            onClick={toggleAll}
            aria-label={allSelected ? 'Deselect all' : 'Select all'}
            className="flex items-center gap-1.5 text-sm text-gray-400 hover:text-white"
          >
            {allSelected ? <CheckSquare size={16} /> : <Square size={16} />}
            {allSelected ? 'Deselect all' : 'Select all'}
          </button>

          <span className="text-gray-600">|</span>

          {someSelected && (
            <>
              <span className="text-xs text-gray-400">{selectedIds.size} selected</span>
              <button
                type="button"
                onClick={handleApproveSelected}
                className="rounded-md bg-green-700 px-2.5 py-1 text-xs font-medium text-white hover:bg-green-600"
              >
                Approve selected (A)
              </button>
              <button
                type="button"
                onClick={handleDenySelected}
                className="rounded-md bg-red-700 px-2.5 py-1 text-xs font-medium text-white hover:bg-red-600"
              >
                Deny selected (D)
              </button>
            </>
          )}

          <div className="flex-1" />

          {/* Approve All / Deny All */}
          <button
            type="button"
            onClick={handleApproveAll}
            className="rounded-md bg-green-900 px-3 py-1 text-xs font-medium text-green-300 hover:bg-green-800"
          >
            Approve all ({visible.length})
          </button>
          <button
            type="button"
            onClick={handleDenyAll}
            className="rounded-md bg-red-900 px-3 py-1 text-xs font-medium text-red-300 hover:bg-red-800"
          >
            Deny all ({visible.length})
          </button>
        </div>
      )}

      {/* States */}
      {loading && <p className="py-12 text-center text-sm text-gray-400">Loading approvals…</p>}

      {!loading && error && (
        <div className="rounded-lg border border-red-800 bg-red-900/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      )}

      {!loading && !error && visible.length === 0 && (
        <div className="py-16 text-center">
          <p className="text-gray-400">No pending approvals 🎉</p>
        </div>
      )}

      {/* Approval list */}
      {!loading && visible.length > 0 && (
        <ul className="space-y-3" aria-label="Pending approvals">
          {visible.map((approval) => (
            <li key={approval.id}>
              <ApprovalCard
                approval={approval}
                agentName={agentMap.get(approval.agent_id)?.name}
                busy={busyIds.has(approval.id)}
                selected={selectedIds.has(approval.id)}
                onApprove={approve}
                onDeny={deny}
                onToggleSelect={toggleSelect}
              />
            </li>
          ))}
        </ul>
      )}
    </main>
  )
}

export default ApprovalQueuePage
