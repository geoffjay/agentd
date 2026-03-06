/**
 * AgentActivityChart — Nivo charts for agent activity.
 *
 * Shows:
 * - Bar chart: agent status distribution (Running / Pending / Stopped / Failed)
 * - Pie chart: same data as a donut
 * - Line chart: running agent count over time (from time-series snapshots)
 */

import { useState } from 'react'
import { ResponsiveBar } from '@nivo/bar'
import { ResponsivePie } from '@nivo/pie'
import { ResponsiveLine } from '@nivo/line'
import { BarChart2, Clock, PieChart } from 'lucide-react'
import { ChartSkeleton } from '@/components/common/LoadingSkeleton'
import { useNivoTheme } from '@/hooks/useNivoTheme'
import type { AgentStatusCounts, AgentTimePoint } from '@/hooks/useMetrics'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AgentActivityChartProps {
  counts: AgentStatusCounts
  timeSeries: AgentTimePoint[]
  loading?: boolean
}

type ChartView = 'bar' | 'pie' | 'line'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STATUS_COLORS: Record<string, string> = {
  Running: '#22c55e',
  Pending: '#f59e0b',
  Stopped: '#94a3b8',
  Failed: '#ef4444',
}

// ---------------------------------------------------------------------------
// AgentActivityChart
// ---------------------------------------------------------------------------

export function AgentActivityChart({ counts, timeSeries, loading }: AgentActivityChartProps) {
  const [view, setView] = useState<ChartView>('bar')
  const nivoTheme = useNivoTheme()

  const barData = Object.entries(counts).map(([status, count]) => ({
    status,
    count,
    color: STATUS_COLORS[status] ?? '#94a3b8',
  }))

  const pieData = Object.entries(counts)
    .filter(([, count]) => count > 0)
    .map(([status, count]) => ({
      id: status,
      label: status,
      value: count,
      color: STATUS_COLORS[status] ?? '#94a3b8',
    }))

  const lineData = [
    {
      id: 'Running',
      color: STATUS_COLORS.Running,
      data: timeSeries.length > 0
        ? timeSeries.map((pt, i) => ({ x: i, y: pt.y }))
        : [{ x: 0, y: 0 }],
    },
  ]

  const views: Array<{ value: ChartView; icon: React.ReactNode; label: string }> = [
    { value: 'bar', icon: <BarChart2 size={14} />, label: 'Bar' },
    { value: 'pie', icon: <PieChart size={14} />, label: 'Pie' },
    { value: 'line', icon: <Clock size={14} />, label: 'Over time' },
  ]

  return (
    <div
      className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5"
      aria-label="Agent activity charts"
    >
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-white">Agent Activity</h3>
        <div
          className="flex items-center rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden"
          role="group"
          aria-label="Chart view"
        >
          {views.map((v) => (
            <button
              key={v.value}
              type="button"
              onClick={() => setView(v.value)}
              aria-pressed={view === v.value}
              className={[
                'flex items-center gap-1 px-2.5 py-1 text-xs transition-colors',
                view === v.value
                  ? 'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400 font-medium'
                  : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200',
              ].join(' ')}
            >
              {v.icon}
              {v.label}
            </button>
          ))}
        </div>
      </div>

      {loading ? (
        <ChartSkeleton />
      ) : (
        <div style={{ height: 200 }} aria-label={`Agent ${view} chart`}>
          {view === 'bar' && (
            <ResponsiveBar
              data={barData}
              keys={['count']}
              indexBy="status"
              theme={nivoTheme}
              colors={({ data }) => STATUS_COLORS[data.status as string] ?? '#94a3b8'}
              enableLabel={true}
              labelSkipHeight={14}
              axisLeft={{ tickSize: 0, tickPadding: 4, tickValues: 5 }}
              axisBottom={{ tickSize: 0, tickPadding: 4 }}
              margin={{ top: 8, right: 8, bottom: 30, left: 32 }}
              borderRadius={3}
              padding={0.3}
              role="img"
              ariaLabel="Agent status distribution bar chart"
            />
          )}

          {view === 'pie' && (
            <ResponsivePie
              data={pieData.length > 0 ? pieData : [{ id: 'none', label: 'No agents', value: 1, color: '#e2e8f0' }]}
              theme={nivoTheme}
              colors={({ data }) => data.color}
              margin={{ top: 8, right: 60, bottom: 8, left: 60 }}
              innerRadius={0.55}
              padAngle={2}
              cornerRadius={3}
              arcLinkLabel="label"
              arcLinkLabelsSkipAngle={10}
              arcLinkLabelsThickness={1}
              role="img"
              ariaLabel="Agent status distribution pie chart"
            />
          )}

          {view === 'line' && (
            <ResponsiveLine
              data={lineData}
              theme={nivoTheme}
              colors={[STATUS_COLORS.Running]}
              margin={{ top: 8, right: 8, bottom: 30, left: 36 }}
              xScale={{ type: 'linear' }}
              yScale={{ type: 'linear', min: 0, stacked: false }}
              axisLeft={{ tickSize: 0, tickPadding: 4, tickValues: 5 }}
              axisBottom={{
                tickSize: 0,
                tickPadding: 4,
                legend: 'Samples (most recent →)',
                legendOffset: 26,
                legendPosition: 'middle',
              }}
              enablePoints={timeSeries.length < 20}
              pointSize={4}
              curve="monotoneX"
              enableGridX={false}
              areaOpacity={0.15}
              enableArea
              role="img"
              ariaLabel="Running agent count over time"
            />
          )}
        </div>
      )}

      {/* Legend */}
      {!loading && view !== 'line' && (
        <div className="mt-3 flex flex-wrap gap-3">
          {Object.entries(STATUS_COLORS).map(([status, color]) => (
            <span key={status} className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
              <span className="h-2 w-2 rounded-full flex-shrink-0" style={{ background: color }} />
              {counts[status as keyof AgentStatusCounts]} {status}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}

export default AgentActivityChart
