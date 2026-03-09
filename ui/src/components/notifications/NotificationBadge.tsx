/**
 * NotificationBadge — compact count badge for pending/unread notifications.
 *
 * Used in the sidebar nav item and header bell icon.
 * Renders nothing when count is 0 to keep the UI clean.
 */

export interface NotificationBadgeProps {
  count: number
  /** When true show even if count is 0 (for testing / placeholder) */
  showZero?: boolean
  className?: string
}

export function NotificationBadge({
  count,
  showZero = false,
  className = '',
}: NotificationBadgeProps) {
  if (count === 0 && !showZero) return null

  const displayCount = count > 99 ? '99+' : String(count)

  return (
    <span
      aria-label={`${count} pending notification${count !== 1 ? 's' : ''}`}
      className={[
        'inline-flex min-w-[1.25rem] items-center justify-center rounded-full px-1',
        'bg-red-500 text-[10px] font-bold leading-none text-white',
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

export default NotificationBadge
