/**
 * NotificationCard — displays a single notification with:
 * - Priority-coded left border (blue/green/orange/red)
 * - Title and message with expand/collapse
 * - Source badge, status badge, timestamp, lifetime indicator
 * - Action buttons: View, Respond, Dismiss, Delete
 * - Selection checkbox for bulk operations
 */

import { useState } from 'react'
import { ChevronDown, ChevronRight, Clock, Infinity as InfinityIcon, Timer } from 'lucide-react'
import type { Notification } from '@/types/notify'
import { StatusBadge } from '@/components/common/StatusBadge'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const PRIORITY_BORDER: Record<string, string> = {
  Low: 'border-l-blue-500',
  Normal: 'border-l-green-500',
  High: 'border-l-orange-500',
  Urgent: 'border-l-red-500',
}

const SOURCE_LABELS: Record<string, string> = {
  System: 'System',
  AskService: 'Ask',
  AgentHook: 'Agent Hook',
  MonitorService: 'Monitor',
}

const SOURCE_COLORS: Record<string, string> = {
  System: 'bg-gray-700 text-gray-300',
  AskService: 'bg-blue-900/50 text-blue-300',
  AgentHook: 'bg-purple-900/50 text-purple-300',
  MonitorService: 'bg-teal-900/50 text-teal-300',
}

function formatRelativeTime(dateStr: string): string {
  const diffMs = Date.now() - new Date(dateStr).getTime()
  const diffSec = Math.floor(diffMs / 1000)
  if (diffSec < 60) return 'just now'
  const diffMin = Math.floor(diffSec / 60)
  if (diffMin < 60) return `${diffMin} min ago`
  const diffHour = Math.floor(diffMin / 60)
  if (diffHour < 24) return `${diffHour}h ago`
  const diffDay = Math.floor(diffHour / 24)
  return `${diffDay}d ago`
}

function formatCountdown(expiresAt: string): string {
  const diffMs = new Date(expiresAt).getTime() - Date.now()
  if (diffMs <= 0) return 'Expired'
  const diffMin = Math.floor(diffMs / 60_000)
  if (diffMin < 60) return `${diffMin}m left`
  const diffHour = Math.floor(diffMin / 60)
  if (diffHour < 24) return `${diffHour}h left`
  return `${Math.floor(diffHour / 24)}d left`
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface NotificationCardProps {
  notification: Notification
  busy?: boolean
  selected?: boolean
  onView: (id: string) => void
  onRespond: (notification: Notification) => void
  onDismiss: (id: string) => void
  onDelete: (id: string) => void
  onToggleSelect?: (id: string) => void
}

export function NotificationCard({
  notification,
  busy = false,
  selected = false,
  onView,
  onRespond,
  onDismiss,
  onDelete,
  onToggleSelect,
}: NotificationCardProps) {
  const [expanded, setExpanded] = useState(false)
  const borderClass = PRIORITY_BORDER[notification.priority] ?? 'border-l-gray-500'
  const isEphemeral = notification.lifetime.type === 'Ephemeral'
  const expiresAt = isEphemeral ? (notification.lifetime as { type: 'Ephemeral'; expires_at: string }).expires_at : null
  const isDone =
    notification.status === 'Dismissed' ||
    notification.status === 'Expired' ||
    notification.status === 'Responded'

  return (
    <article
      aria-label={`Notification: ${notification.title}`}
      className={[
        'rounded-lg border border-gray-700 bg-gray-800 border-l-4 transition-opacity',
        borderClass,
        selected ? 'ring-2 ring-primary-500' : '',
        isDone ? 'opacity-70' : '',
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <div className="flex items-start gap-3 p-4">
        {/* Selection checkbox */}
        {onToggleSelect && (
          <input
            type="checkbox"
            aria-label={`Select notification: ${notification.title}`}
            checked={selected}
            onChange={() => onToggleSelect(notification.id)}
            className="mt-0.5 h-4 w-4 rounded border-gray-600 bg-gray-700 text-primary-500 focus:ring-primary-500 shrink-0"
          />
        )}

        {/* Main content */}
        <div className="min-w-0 flex-1">
          {/* Top row: title + badges */}
          <div className="flex flex-wrap items-start gap-2">
            <span className="font-semibold text-white text-sm leading-tight flex-1 min-w-0">
              {notification.title}
            </span>

            {/* Source badge */}
            <span
              className={[
                'shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide',
                SOURCE_COLORS[notification.source] ?? 'bg-gray-700 text-gray-300',
              ].join(' ')}
            >
              {SOURCE_LABELS[notification.source] ?? notification.source}
            </span>

            {/* Status badge */}
            <StatusBadge status={notification.status} />
          </div>

          {/* Meta row: timestamp + lifetime */}
          <div className="mt-1 flex flex-wrap items-center gap-3 text-xs text-gray-400">
            <span className="flex items-center gap-1">
              <Clock size={11} aria-hidden="true" />
              {formatRelativeTime(notification.created_at)}
            </span>

            {isEphemeral && expiresAt ? (
              <span className="flex items-center gap-1 text-amber-400">
                <Timer size={11} aria-hidden="true" />
                {formatCountdown(expiresAt)}
              </span>
            ) : (
              <span className="flex items-center gap-1 text-gray-500">
                <InfinityIcon size={11} aria-hidden="true" />
                Persistent
              </span>
            )}

            {/* Priority label */}
            <span className="capitalize text-gray-500">{notification.priority}</span>
          </div>

          {/* Expand toggle */}
          <button
            type="button"
            aria-expanded={expanded}
            aria-controls={`notif-msg-${notification.id}`}
            onClick={() => setExpanded((v) => !v)}
            className="mt-1 flex items-center gap-1 text-xs text-gray-400 hover:text-gray-300"
          >
            {expanded ? (
              <ChevronDown size={12} aria-hidden="true" />
            ) : (
              <ChevronRight size={12} aria-hidden="true" />
            )}
            {expanded ? 'Hide message' : 'Show message'}
          </button>

          {/* Expandable message */}
          {expanded && (
            <p
              id={`notif-msg-${notification.id}`}
              className="mt-2 rounded bg-gray-900 p-3 text-xs text-gray-300 whitespace-pre-wrap"
            >
              {notification.message}
            </p>
          )}

          {/* Response (if already responded) */}
          {notification.status === 'Responded' && notification.response && (
            <div className="mt-2 rounded bg-gray-900/60 p-2 text-xs text-gray-400">
              <span className="font-semibold text-gray-300">Response:</span>{' '}
              {notification.response}
            </div>
          )}

          {/* Action buttons */}
          <div className="mt-3 flex flex-wrap gap-2">
            {notification.status === 'Pending' && (
              <button
                type="button"
                disabled={busy}
                onClick={() => onView(notification.id)}
                className="rounded px-2.5 py-1 text-xs font-medium bg-gray-700 text-gray-300 hover:bg-gray-600 disabled:opacity-50 transition-colors"
              >
                Mark Viewed
              </button>
            )}

            {notification.requires_response &&
              (notification.status === 'Pending' || notification.status === 'Viewed') && (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onRespond(notification)}
                  className="rounded px-2.5 py-1 text-xs font-medium bg-primary-700 text-white hover:bg-primary-600 disabled:opacity-50 transition-colors"
                >
                  Respond
                </button>
              )}

            {(notification.status === 'Pending' || notification.status === 'Viewed') && (
              <button
                type="button"
                disabled={busy}
                onClick={() => onDismiss(notification.id)}
                className="rounded px-2.5 py-1 text-xs font-medium bg-gray-700 text-gray-300 hover:bg-gray-600 disabled:opacity-50 transition-colors"
              >
                Dismiss
              </button>
            )}

            <button
              type="button"
              disabled={busy}
              onClick={() => onDelete(notification.id)}
              className="rounded px-2.5 py-1 text-xs font-medium bg-red-900/40 text-red-400 hover:bg-red-900/70 disabled:opacity-50 transition-colors"
            >
              Delete
            </button>
          </div>
        </div>
      </div>
    </article>
  )
}

export default NotificationCard
