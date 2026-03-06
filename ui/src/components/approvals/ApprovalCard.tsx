/**
 * ApprovalCard — displays a single pending approval with urgency indicator,
 * expandable tool input, approve/deny actions, and selection checkbox.
 */

import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ChevronDown, ChevronRight, Clock } from 'lucide-react'
import type { PendingApproval } from '@/types/orchestrator'
import { ApprovalActions } from './ApprovalActions'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function minutesWaiting(createdAt: string): number {
  return Math.floor((Date.now() - new Date(createdAt).getTime()) / 60_000)
}

function urgencyClass(minutes: number): string {
  if (minutes >= 10) return 'border-l-red-500'
  if (minutes >= 5) return 'border-l-yellow-500'
  return 'border-l-green-500'
}

function urgencyLabel(minutes: number): string {
  if (minutes >= 10) return 'High urgency'
  if (minutes >= 5) return 'Medium urgency'
  return 'Low urgency'
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface ApprovalCardProps {
  approval: PendingApproval
  agentName?: string
  busy?: boolean
  selected?: boolean
  onApprove: (id: string) => void
  onDeny: (id: string) => void
  onToggleSelect?: (id: string) => void
}

export function ApprovalCard({
  approval,
  agentName,
  busy = false,
  selected = false,
  onApprove,
  onDeny,
  onToggleSelect,
}: ApprovalCardProps) {
  const [expanded, setExpanded] = useState(false)
  const minutes = minutesWaiting(approval.created_at)
  const urgency = urgencyClass(minutes)

  return (
    <article
      aria-label={`Approval request for ${approval.tool_name}`}
      className={[
        'rounded-lg border border-gray-700 bg-gray-800 border-l-4',
        urgency,
        selected ? 'ring-2 ring-primary-500' : '',
      ]
        .filter(Boolean)
        .join(' ')}
    >
      {/* Header row */}
      <div className="flex items-start gap-3 p-4">
        {/* Selection checkbox */}
        {onToggleSelect && (
          <input
            type="checkbox"
            aria-label={`Select approval for ${approval.tool_name}`}
            checked={selected}
            onChange={() => onToggleSelect(approval.id)}
            className="mt-0.5 h-4 w-4 rounded border-gray-600 bg-gray-700 text-primary-500 focus:ring-primary-500"
          />
        )}

        {/* Main content */}
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            {/* Tool name */}
            <span className="font-mono text-sm font-semibold text-white">
              {approval.tool_name}
            </span>

            {/* Agent link */}
            {agentName && (
              <Link
                to={`/agents/${approval.agent_id}`}
                className="text-xs text-primary-400 hover:text-primary-300"
              >
                {agentName}
              </Link>
            )}

            {/* Urgency + wait time */}
            <span
              aria-label={urgencyLabel(minutes)}
              className="ml-auto flex items-center gap-1 text-xs text-gray-400"
            >
              <Clock size={12} aria-hidden="true" />
              {minutes < 1 ? 'just now' : `${minutes}m ago`}
            </span>
          </div>

          {/* Expand toggle */}
          <button
            type="button"
            aria-expanded={expanded}
            aria-controls={`approval-details-${approval.id}`}
            onClick={() => setExpanded(v => !v)}
            className="mt-1 flex items-center gap-1 text-xs text-gray-400 hover:text-gray-300"
          >
            {expanded ? (
              <ChevronDown size={12} aria-hidden="true" />
            ) : (
              <ChevronRight size={12} aria-hidden="true" />
            )}
            {expanded ? 'Hide details' : 'Show details'}
          </button>

          {/* Expandable tool input */}
          {expanded && (
            <pre
              id={`approval-details-${approval.id}`}
              className="mt-2 max-h-48 overflow-auto rounded bg-gray-900 p-3 text-xs text-gray-300"
            >
              {JSON.stringify(approval.tool_input, null, 2)}
            </pre>
          )}
        </div>

        {/* Actions */}
        <ApprovalActions
          approvalId={approval.id}
          busy={busy}
          onApprove={onApprove}
          onDeny={onDeny}
          size="sm"
        />
      </div>
    </article>
  )
}

export default ApprovalCard
