/**
 * NotificationMetricsChart — Nivo charts for notification metrics.
 *
 * Shows:
 * - Bar chart: notifications by priority level
 * - Total count summary
 */

import { ResponsiveBar } from '@nivo/bar'
import { ChartSkeleton } from '@/components/common/LoadingSkeleton'
import { useNivoTheme } from '@/hooks/useNivoTheme'
import type { NotificationCounts } from '@/hooks/useMetrics'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface NotificationMetricsChartProps {
  counts: NotificationCounts
  loading?: boolean
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PRIORITY_COLORS: Record<string, string> = {
  Low: '#94a3b8',
  Normal: '#60a5fa',
  High: '#f59e0b',
  Urgent: '#ef4444',
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function NotificationMetricsChart({ counts, loading }: NotificationMetricsChartProps) {
  const nivoTheme = useNivoTheme()

  const barData = [
    { priority: 'Low', count: counts.Low, color: PRIORITY_COLORS.Low },
    { priority: 'Normal', count: counts.Normal, color: PRIORITY_COLORS.Normal },
    { priority: 'High', count: counts.High, color: PRIORITY_COLORS.High },
    { priority: 'Urgent', count: counts.Urgent, color: PRIORITY_COLORS.Urgent },
  ]

  const activeCount = counts.Low + counts.Normal + counts.High + counts.Urgent

  return (
    <div
      className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5"
      aria-label="Notification metrics"
    >
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
          Notification Breakdown
        </h3>
        <div className="text-right">
          <p className="text-lg font-semibold tabular-nums text-gray-900 dark:text-white">
            {counts.total}
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500">total</p>
        </div>
      </div>

      {loading ? (
        <ChartSkeleton />
      ) : (
        <div style={{ height: 160 }} aria-label="Notifications by priority bar chart">
          <ResponsiveBar
            data={barData}
            keys={['count']}
            indexBy="priority"
            theme={nivoTheme}
            colors={({ data }) => PRIORITY_COLORS[data.priority as string] ?? '#94a3b8'}
            enableLabel={true}
            labelSkipHeight={12}
            axisLeft={null}
            axisBottom={{ tickSize: 0, tickPadding: 4 }}
            margin={{ top: 8, right: 8, bottom: 28, left: 8 }}
            borderRadius={3}
            padding={0.3}
            role="img"
            ariaLabel="Notification count by priority"
          />
        </div>
      )}

      {/* Summary counts */}
      {!loading && (
        <div className="mt-3 flex gap-4">
          {Object.entries(PRIORITY_COLORS).map(([priority, color]) => (
            <div key={priority} className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full flex-shrink-0" style={{ background: color }} />
              <span className="text-xs text-gray-500 dark:text-gray-400">
                {counts[priority as keyof Omit<NotificationCounts, 'total'>]} {priority}
              </span>
            </div>
          ))}
        </div>
      )}

      {!loading && activeCount === 0 && (
        <p className="mt-2 text-xs text-gray-400 dark:text-gray-500">
          No active notifications
        </p>
      )}
    </div>
  )
}

export default NotificationMetricsChart
