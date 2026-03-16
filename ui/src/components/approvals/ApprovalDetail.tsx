/**
 * ApprovalDetail — drawer content for a single approval.
 *
 * Shows all approval details including the full tool input JSON,
 * urgency indicator, agent link, and approve/deny actions.
 */

import { Link } from 'react-router-dom'
import { Clock } from 'lucide-react'
import type { PendingApproval } from '@/types/orchestrator'
import { ApprovalActions } from './ApprovalActions'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function minutesWaiting(createdAt: string): number {
  return Math.floor((Date.now() - new Date(createdAt).getTime()) / 60_000)
}

function urgencyLabel(minutes: number): string {
  if (minutes >= 10) return 'High urgency'
  if (minutes >= 5) return 'Medium urgency'
  return 'Low urgency'
}

function urgencyColor(minutes: number): string {
  if (minutes >= 10) return 'text-red-400'
  if (minutes >= 5) return 'text-yellow-400'
  return 'text-green-400'
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface ApprovalDetailProps {
  approval: PendingApproval
  agentName?: string
  busy?: boolean
  onApprove: (id: string) => void
  onDeny: (id: string) => void
}

export function ApprovalDetail({
  approval,
  agentName,
  busy = false,
  onApprove,
  onDeny,
}: ApprovalDetailProps) {
  const minutes = minutesWaiting(approval.created_at)

  return (
    <div className="space-y-5">
      {/* Tool name */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          Tool
        </h3>
        <p className="mt-1 font-mono text-sm font-semibold text-gray-900 dark:text-white">
          {approval.tool_name}
        </p>
      </div>

      {/* Agent */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          Agent
        </h3>
        <p className="mt-1">
          {agentName ? (
            <Link
              to={`/agents/${approval.agent_id}`}
              className="text-sm text-primary-600 hover:text-primary-500 dark:text-primary-400 dark:hover:text-primary-300"
            >
              {agentName}
            </Link>
          ) : (
            <span className="text-sm text-gray-500 dark:text-gray-400">{approval.agent_id}</span>
          )}
        </p>
      </div>

      {/* Urgency & timing */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          Wait Time
        </h3>
        <div className="mt-1 flex items-center gap-2">
          <Clock size={14} className={urgencyColor(minutes)} />
          <span className={['text-sm font-medium', urgencyColor(minutes)].join(' ')}>
            {minutes < 1 ? 'Just now' : `${minutes}m ago`}
          </span>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            ({urgencyLabel(minutes)})
          </span>
        </div>
      </div>

      {/* Status */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          Status
        </h3>
        <p className="mt-1 text-sm capitalize text-gray-700 dark:text-gray-300">
          {approval.status}
        </p>
      </div>

      {/* Created / Expires */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            Created
          </h3>
          <p className="mt-1 text-sm text-gray-700 dark:text-gray-300">
            {new Date(approval.created_at).toLocaleString()}
          </p>
        </div>
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            Expires
          </h3>
          <p className="mt-1 text-sm text-gray-700 dark:text-gray-300">
            {new Date(approval.expires_at).toLocaleString()}
          </p>
        </div>
      </div>

      {/* Tool input */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          Tool Input
        </h3>
        <pre className="mt-2 max-h-72 overflow-auto rounded-lg bg-gray-100 p-4 text-xs text-gray-800 dark:bg-gray-800 dark:text-gray-300">
          {JSON.stringify(approval.tool_input, null, 2)}
        </pre>
      </div>

      {/* Actions */}
      {approval.status === 'pending' && (
        <div className="border-t border-gray-200 pt-4 dark:border-gray-700">
          <ApprovalActions
            approvalId={approval.id}
            busy={busy}
            onApprove={onApprove}
            onDeny={onDeny}
            size="md"
          />
        </div>
      )}
    </div>
  )
}

export default ApprovalDetail
