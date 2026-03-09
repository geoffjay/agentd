/**
 * ApprovalActions — Approve / Deny buttons for a single pending approval.
 */

import { CheckCircle2, XCircle } from 'lucide-react'

export interface ApprovalActionsProps {
  approvalId: string
  busy?: boolean
  onApprove: (id: string) => void
  onDeny: (id: string) => void
  size?: 'sm' | 'md'
}

export function ApprovalActions({
  approvalId,
  busy = false,
  onApprove,
  onDeny,
  size = 'md',
}: ApprovalActionsProps) {
  const isSmall = size === 'sm'

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        disabled={busy}
        onClick={() => onApprove(approvalId)}
        aria-label="Approve"
        className={[
          'inline-flex items-center gap-1 rounded-md font-medium transition-colors',
          'bg-green-600 text-white hover:bg-green-500 disabled:cursor-not-allowed disabled:opacity-50',
          isSmall ? 'px-2 py-1 text-xs' : 'px-3 py-1.5 text-sm',
        ].join(' ')}
      >
        <CheckCircle2 size={isSmall ? 12 : 14} aria-hidden="true" />
        {busy ? 'Working…' : 'Approve'}
      </button>

      <button
        type="button"
        disabled={busy}
        onClick={() => onDeny(approvalId)}
        aria-label="Deny"
        className={[
          'inline-flex items-center gap-1 rounded-md font-medium transition-colors',
          'bg-red-600 text-white hover:bg-red-500 disabled:cursor-not-allowed disabled:opacity-50',
          isSmall ? 'px-2 py-1 text-xs' : 'px-3 py-1.5 text-sm',
        ].join(' ')}
      >
        <XCircle size={isSmall ? 12 : 14} aria-hidden="true" />
        {busy ? 'Working…' : 'Deny'}
      </button>
    </div>
  )
}

export default ApprovalActions
