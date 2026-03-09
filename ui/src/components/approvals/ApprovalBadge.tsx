/**
 * ApprovalBadge — compact count badge for pending approvals.
 *
 * Used in the sidebar nav item and header area.
 * Renders nothing when count is 0 to keep the UI clean.
 */

export interface ApprovalBadgeProps {
  count: number
  /** When true show even if count is 0 (for testing / placeholder) */
  showZero?: boolean
  className?: string
}

export function ApprovalBadge({ count, showZero = false, className = '' }: ApprovalBadgeProps) {
  if (count === 0 && !showZero) return null

  const displayCount = count > 99 ? '99+' : String(count)

  return (
    <span
      aria-label={`${count} pending approval${count !== 1 ? 's' : ''}`}
      className={[
        'inline-flex min-w-[1.25rem] items-center justify-center rounded-full px-1',
        'bg-yellow-500 text-[10px] font-bold leading-none text-gray-900',
        // Pulse animation when count > 0 to draw attention
        count > 0 ? 'animate-pulse' : '',
        className,
      ]
        .filter(Boolean)
        .join(' ')}
    >
      {displayCount}
    </span>
  )
}

export default ApprovalBadge
