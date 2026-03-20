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

  return (
    <span
      aria-label={`${count} pending notification${count !== 1 ? 's' : ''}`}
      className={`inline-block h-2 w-2 rounded-full bg-red-500 ${className}`}
    />
  )
}

export default NotificationBadge
