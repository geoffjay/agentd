/**
 * AgentApprovals — list of pending tool approval requests for this agent.
 *
 * Shows:
 * - Tool name, request time, expiry
 * - Tool input (collapsed by default)
 * - Approve / Deny buttons per request
 * - Count badge in section header
 * - Loading / empty / error states
 */

import { useState } from 'react'
import { Check, ChevronDown, ChevronRight, X } from 'lucide-react'
import type { PendingApproval } from '@/types/orchestrator'
import { ListItemSkeleton } from '@/components/common/LoadingSkeleton'

// ---------------------------------------------------------------------------
// ApprovalRow
// ---------------------------------------------------------------------------

interface ApprovalRowProps {
  approval: PendingApproval
  onApprove: (id: string) => Promise<void>
  onDeny: (id: string) => Promise<void>
}

function ApprovalRow({ approval, onApprove, onDeny }: ApprovalRowProps) {
  const [expanded, setExpanded] = useState(false)
  const [approving, setApproving] = useState(false)
  const [denying, setDenying] = useState(false)
  const [error, setError] = useState<string | undefined>()

  const busy = approving || denying

  const requestedAt = new Date(approval.created_at).toLocaleString()
  const expiresAt = new Date(approval.expires_at).toLocaleString()

  async function handleApprove() {
    setError(undefined)
    setApproving(true)
    try {
      await onApprove(approval.id)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Approve failed')
    } finally {
      setApproving(false)
    }
  }

  async function handleDeny() {
    setError(undefined)
    setDenying(true)
    try {
      await onDeny(approval.id)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Deny failed')
    } finally {
      setDenying(false)
    }
  }

  return (
    <li className="flex flex-col gap-2 rounded-lg border border-gray-200 p-3 dark:border-gray-700">
      {/* Header row */}
      <div className="flex items-start justify-between gap-3">
        <div className="flex flex-col gap-0.5">
          <span className="text-sm font-medium text-gray-900 dark:text-white">
            {approval.tool_name}
          </span>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            Requested: {requestedAt}
          </span>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            Expires: {expiresAt}
          </span>
        </div>

        {/* Actions */}
        <div className="flex flex-shrink-0 items-center gap-2">
          <button
            type="button"
            aria-label={`Approve ${approval.tool_name}`}
            onClick={handleApprove}
            disabled={busy}
            className="flex items-center gap-1 rounded-md bg-green-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-green-500 disabled:opacity-50"
          >
            <Check size={12} aria-hidden="true" />
            {approving ? 'Approving…' : 'Approve'}
          </button>
          <button
            type="button"
            aria-label={`Deny ${approval.tool_name}`}
            onClick={handleDeny}
            disabled={busy}
            className="flex items-center gap-1 rounded-md bg-red-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-500 disabled:opacity-50"
          >
            <X size={12} aria-hidden="true" />
            {denying ? 'Denying…' : 'Deny'}
          </button>
        </div>
      </div>

      {/* Tool input toggle */}
      {approval.tool_input !== undefined && (
        <div>
          <button
            type="button"
            aria-expanded={expanded}
            onClick={() => setExpanded(e => !e)}
            className="flex items-center gap-1 text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
          >
            {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            Tool input
          </button>
          {expanded && (
            <pre className="mt-1 overflow-x-auto rounded bg-gray-100 p-2 text-xs text-gray-700 dark:bg-gray-800 dark:text-gray-300">
              {JSON.stringify(approval.tool_input, null, 2)}
            </pre>
          )}
        </div>
      )}

      {/* Error */}
      {error && (
        <p role="alert" className="text-xs text-red-500 dark:text-red-400">
          {error}
        </p>
      )}
    </li>
  )
}

// ---------------------------------------------------------------------------
// AgentApprovals
// ---------------------------------------------------------------------------

export interface AgentApprovalsProps {
  approvals: PendingApproval[]
  loading: boolean
  error?: string
  onApprove: (id: string) => Promise<void>
  onDeny: (id: string) => Promise<void>
}

export function AgentApprovals({
  approvals,
  loading,
  error,
  onApprove,
  onDeny,
}: AgentApprovalsProps) {
  return (
    <section aria-label="Pending approvals">
      <div className="mb-3 flex items-center gap-2">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Pending Approvals
        </h3>
        {approvals.length > 0 && (
          <span className="rounded-full bg-yellow-100 px-2 py-0.5 text-xs font-medium text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300">
            {approvals.length}
          </span>
        )}
      </div>

      {loading ? (
        <ListItemSkeleton rows={2} />
      ) : error ? (
        <p
          role="alert"
          className="text-sm text-red-600 dark:text-red-400"
        >
          {error}
        </p>
      ) : approvals.length === 0 ? (
        <p className="text-sm text-gray-500 dark:text-gray-400">
          No pending approvals.
        </p>
      ) : (
        <ul className="flex flex-col gap-2" aria-label="Approval requests">
          {approvals.map(a => (
            <ApprovalRow
              key={a.id}
              approval={a}
              onApprove={onApprove}
              onDeny={onDeny}
            />
          ))}
        </ul>
      )}
    </section>
  )
}

export default AgentApprovals
