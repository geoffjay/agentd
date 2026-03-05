/**
 * ActivityTimeline — recent activity feed combining agent state changes,
 * notifications, and questions.
 *
 * For the initial release this component accepts pre-fetched data as props
 * (passed down from the Dashboard page) rather than fetching independently.
 */

import { Link } from 'react-router-dom'
import { Bot, Bell, HelpCircle, ExternalLink } from 'lucide-react'
import { ListItemSkeleton } from '@/components/common/LoadingSkeleton'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ActivityEventType = 'agent' | 'notification' | 'question'

export interface ActivityEvent {
  id: string
  type: ActivityEventType
  description: string
  timestamp: Date
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatRelativeTime(date: Date): string {
  const diffMs = Date.now() - date.getTime()
  const diffSec = Math.floor(diffMs / 1000)
  if (diffSec < 60) return 'just now'
  const diffMin = Math.floor(diffSec / 60)
  if (diffMin < 60) return `${diffMin} min ago`
  const diffHr = Math.floor(diffMin / 60)
  if (diffHr < 24) return `${diffHr}h ago`
  return `${Math.floor(diffHr / 24)}d ago`
}

const EVENT_ICONS: Record<ActivityEventType, React.ReactNode> = {
  agent: <Bot size={16} className="text-primary-500" />,
  notification: <Bell size={16} className="text-yellow-500" />,
  question: <HelpCircle size={16} className="text-blue-500" />,
}

const EVENT_BG: Record<ActivityEventType, string> = {
  agent: 'bg-primary-100 dark:bg-primary-900/30',
  notification: 'bg-yellow-100 dark:bg-yellow-900/30',
  question: 'bg-blue-100 dark:bg-blue-900/30',
}

// ---------------------------------------------------------------------------
// ActivityTimeline
// ---------------------------------------------------------------------------

interface ActivityTimelineProps {
  events: ActivityEvent[]
  loading?: boolean
  error?: string
}

export function ActivityTimeline({ events, loading = false, error }: ActivityTimelineProps) {
  return (
    <section
      aria-labelledby="activity-timeline-heading"
      className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2
          id="activity-timeline-heading"
          className="text-base font-semibold text-gray-900 dark:text-white"
        >
          Recent Activity
        </h2>
        <Link
          to="/notifications"
          className="flex items-center gap-1 text-xs font-medium text-primary-600 hover:text-primary-700 dark:text-primary-400"
        >
          View All <ExternalLink size={12} />
        </Link>
      </div>

      {/* Error */}
      {error && <p className="mt-3 text-sm text-red-500">{error}</p>}

      {/* Loading */}
      {loading && !error && (
        <div className="mt-4">
          <ListItemSkeleton rows={5} />
        </div>
      )}

      {/* Empty state */}
      {!loading && !error && events.length === 0 && (
        <p className="mt-4 text-sm text-gray-500 dark:text-gray-400">No recent activity.</p>
      )}

      {/* Event list */}
      {!loading && !error && events.length > 0 && (
        <ol role="list" aria-label="Activity feed" className="mt-4 space-y-3">
          {events.slice(0, 10).map((event) => (
            <li key={event.id} className="flex items-start gap-3">
              {/* Icon bubble */}
              <div
                aria-hidden="true"
                className={[
                  'mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full',
                  EVENT_BG[event.type],
                ].join(' ')}
              >
                {EVENT_ICONS[event.type]}
              </div>
              {/* Text */}
              <div className="min-w-0 flex-1">
                <p className="text-sm text-gray-800 dark:text-gray-200">{event.description}</p>
                <time
                  dateTime={event.timestamp.toISOString()}
                  className="text-xs text-gray-400 dark:text-gray-500"
                >
                  {formatRelativeTime(event.timestamp)}
                </time>
              </div>
            </li>
          ))}
        </ol>
      )}
    </section>
  )
}

export default ActivityTimeline
