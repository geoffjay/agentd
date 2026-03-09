/**
 * NotificationSummary — shows pending/unread counts and priority breakdown bar chart.
 */

import { Link } from 'react-router-dom'
import { ResponsiveBar } from '@nivo/bar'
import { Bell, ExternalLink } from 'lucide-react'
import { ChartSkeleton } from '@/components/common/LoadingSkeleton'
import type { UseNotificationSummaryResult } from '@/hooks/useNotificationSummary'

const PRIORITY_COLORS: Record<string, string> = {
  low: '#94a3b8',
  normal: '#60a5fa',
  high: '#f59e0b',
  urgent: '#ef4444',
}

type NotificationSummaryProps = UseNotificationSummaryResult

export function NotificationSummary({
  pending,
  unread,
  total,
  priorityCounts,
  loading,
  error,
}: NotificationSummaryProps) {
  const barData = [
    { priority: 'low', count: priorityCounts.low, color: PRIORITY_COLORS.low },
    { priority: 'normal', count: priorityCounts.normal, color: PRIORITY_COLORS.normal },
    { priority: 'high', count: priorityCounts.high, color: PRIORITY_COLORS.high },
    { priority: 'urgent', count: priorityCounts.urgent, color: PRIORITY_COLORS.urgent },
  ]

  const hasData = total > 0

  return (
    <section
      aria-labelledby="notification-summary-heading"
      className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2
          id="notification-summary-heading"
          className="text-base font-semibold text-gray-900 dark:text-white"
        >
          Notifications
        </h2>
        <Link
          to="/notifications"
          className="flex items-center gap-1 text-xs font-medium text-primary-600 hover:text-primary-700 dark:text-primary-400"
        >
          View All <ExternalLink size={12} />
        </Link>
      </div>

      {/* Error state */}
      {error && <p className="mt-3 text-sm text-red-500">{error}</p>}

      {/* Loading */}
      {loading && !error && (
        <div className="mt-4">
          <ChartSkeleton height={120} />
        </div>
      )}

      {/* Content */}
      {!loading && !error && (
        <>
          {/* Summary counts */}
          <div className="mt-4 grid grid-cols-2 gap-3">
            <div className="rounded-md bg-yellow-50 p-3 dark:bg-yellow-900/20">
              <div className="flex items-center gap-2">
                <Bell size={16} className="text-yellow-600 dark:text-yellow-400" />
                <span className="text-xs text-yellow-700 dark:text-yellow-300">Pending</span>
              </div>
              <p className="mt-1 text-2xl font-bold text-yellow-800 dark:text-yellow-200">
                {pending}
              </p>
            </div>
            <div className="rounded-md bg-blue-50 p-3 dark:bg-blue-900/20">
              <div className="flex items-center gap-2">
                <Bell size={16} className="text-blue-600 dark:text-blue-400" />
                <span className="text-xs text-blue-700 dark:text-blue-300">Unread</span>
              </div>
              <p className="mt-1 text-2xl font-bold text-blue-800 dark:text-blue-200">{unread}</p>
            </div>
          </div>

          {/* Priority bar chart */}
          {hasData ? (
            <div className="mt-4">
              <p className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
                By Priority (active)
              </p>
              <div className="h-28">
                <ResponsiveBar
                  data={barData}
                  keys={['count']}
                  indexBy="priority"
                  colors={({ data }) => PRIORITY_COLORS[data.priority as string] ?? '#94a3b8'}
                  enableLabel={false}
                  axisLeft={null}
                  axisBottom={{
                    tickSize: 0,
                    tickPadding: 6,
                  }}
                  borderRadius={3}
                  padding={0.3}
                  margin={{ top: 0, right: 0, bottom: 24, left: 0 }}
                  tooltip={({ indexValue, value }) => (
                    <div className="rounded bg-gray-900 px-2 py-1 text-xs text-white shadow">
                      {indexValue}: {value}
                    </div>
                  )}
                  theme={{
                    axis: {
                      ticks: {
                        text: { fill: '#94a3b8', fontSize: 11 },
                      },
                    },
                  }}
                />
              </div>
            </div>
          ) : (
            <p className="mt-4 text-sm text-gray-500 dark:text-gray-400">
              No active notifications.
            </p>
          )}
        </>
      )}
    </section>
  )
}

export default NotificationSummary
