/**
 * StatusBadge — coloured pill/dot for entity status values.
 */

import type { AgentStatus } from '@/types/orchestrator'
import type { NotificationStatus } from '@/types/notify'

export type ServiceStatus = 'healthy' | 'degraded' | 'down' | 'unknown'

type KnownStatus = AgentStatus | NotificationStatus | ServiceStatus

interface StatusBadgeProps {
  status: KnownStatus
  /** 'badge' renders a pill with text; 'dot' renders a coloured circle only */
  variant?: 'badge' | 'dot'
  className?: string
}

const STATUS_STYLES: Record<string, string> = {
  // Agent statuses
  Running: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
  Pending: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
  Stopped: 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400',
  Failed: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
  // Notification statuses
  Viewed: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400',
  Responded: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
  Dismissed: 'bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-500',
  Expired: 'bg-gray-100 text-gray-400 dark:bg-gray-800 dark:text-gray-600',
  // Service health
  healthy: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
  degraded: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
  down: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
  unknown: 'bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-400',
}

const DOT_STYLES: Record<string, string> = {
  Running: 'bg-green-500',
  Pending: 'bg-yellow-500',
  Stopped: 'bg-gray-400',
  Failed: 'bg-red-500',
  Viewed: 'bg-blue-500',
  Responded: 'bg-green-500',
  Dismissed: 'bg-gray-400',
  Expired: 'bg-gray-300',
  healthy: 'bg-green-500',
  degraded: 'bg-yellow-500',
  down: 'bg-red-500',
  unknown: 'bg-gray-400',
}

export function StatusBadge({ status, variant = 'badge', className = '' }: StatusBadgeProps) {
  if (variant === 'dot') {
    return (
      <span
        role="status"
        aria-label={status}
        className={[
          'inline-block h-2.5 w-2.5 rounded-full',
          DOT_STYLES[status] ?? 'bg-gray-400',
          className,
        ].join(' ')}
      />
    )
  }

  return (
    <span
      role="status"
      className={[
        'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium',
        STATUS_STYLES[status] ?? 'bg-gray-100 text-gray-600',
        className,
      ].join(' ')}
    >
      {status}
    </span>
  )
}

export default StatusBadge
