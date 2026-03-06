/**
 * PlaceholderChart — a chart placeholder with a "Coming Soon" overlay.
 *
 * Shows a Nivo chart rendered with sample/synthetic data, overlaid with a
 * translucent "Coming Soon" message indicating the chart will show real data
 * once the monitor service is fully implemented.
 */

import { Info } from 'lucide-react'
import { ResponsiveBar } from '@nivo/bar'
import { ResponsiveLine } from '@nivo/line'
import { useNivoTheme } from '@/hooks/useNivoTheme'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type PlaceholderChartVariant = 'cpu' | 'memory' | 'disk' | 'network'

export interface PlaceholderChartProps {
  variant: PlaceholderChartVariant
  title: string
  description?: string
  /** Height of the chart area in pixels (default 140) */
  height?: number
}

// ---------------------------------------------------------------------------
// Sample data generators
// ---------------------------------------------------------------------------

function makeSineData(base: number, amplitude: number, points = 20) {
  return Array.from({ length: points }, (_, i) => ({
    x: i,
    y: Math.max(0, Math.min(100, base + amplitude * Math.sin(i * 0.5) + Math.random() * 5)),
  }))
}

const SAMPLE_DATA: Record<
  PlaceholderChartVariant,
  {
    color: string
    lineData: Array<{ x: number; y: number }>
    barData: Array<{ label: string; value: number }>
    unit: string
  }
> = {
  cpu: {
    color: '#3b82f6',
    lineData: makeSineData(45, 20),
    barData: [
      { label: 'User', value: 32 },
      { label: 'System', value: 14 },
      { label: 'IO', value: 8 },
      { label: 'Idle', value: 46 },
    ],
    unit: '%',
  },
  memory: {
    color: '#8b5cf6',
    lineData: makeSineData(62, 10),
    barData: [
      { label: 'Used', value: 62 },
      { label: 'Cache', value: 18 },
      { label: 'Free', value: 20 },
    ],
    unit: '%',
  },
  disk: {
    color: '#f59e0b',
    lineData: makeSineData(38, 5),
    barData: [
      { label: '/', value: 38 },
      { label: '/data', value: 55 },
      { label: '/tmp', value: 12 },
    ],
    unit: '%',
  },
  network: {
    color: '#22c55e',
    lineData: makeSineData(30, 25),
    barData: [
      { label: 'In', value: 42 },
      { label: 'Out', value: 28 },
    ],
    unit: 'MB/s',
  },
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function PlaceholderChart({ variant, title, description, height = 140 }: PlaceholderChartProps) {
  const nivoTheme = useNivoTheme()
  const data = SAMPLE_DATA[variant]

  const lineChartData = [
    {
      id: title,
      color: data.color,
      data: data.lineData,
    },
  ]

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 overflow-hidden">
      {/* Header */}
      <div className="px-4 pt-4 pb-2">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-gray-900 dark:text-white">{title}</h3>
          <span className="rounded-full bg-amber-100 dark:bg-amber-900/30 px-2 py-0.5 text-xs font-medium text-amber-700 dark:text-amber-400">
            Coming Soon
          </span>
        </div>
        {description && (
          <p className="mt-0.5 text-xs text-gray-400 dark:text-gray-500">{description}</p>
        )}
      </div>

      {/* Chart with overlay */}
      <div className="relative px-2" style={{ height }}>
        {/* Sample chart — blurred */}
        <div className="absolute inset-0 blur-sm opacity-60 pointer-events-none" aria-hidden="true">
          <ResponsiveLine
            data={lineChartData}
            theme={nivoTheme}
            margin={{ top: 8, right: 8, bottom: 24, left: 32 }}
            xScale={{ type: 'linear' }}
            yScale={{ type: 'linear', min: 0, max: 100 }}
            colors={[data.color]}
            enablePoints={false}
            enableGridX={false}
            curve="monotoneX"
            axisBottom={null}
            axisLeft={{ tickSize: 0, tickPadding: 4, tickValues: 3 }}
            isInteractive={false}
          />
        </div>

        {/* Coming Soon overlay */}
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-1">
          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-amber-100 dark:bg-amber-900/30">
            <Info size={16} className="text-amber-600 dark:text-amber-400" />
          </div>
          <p className="text-xs font-medium text-gray-600 dark:text-gray-400">
            Monitor service not yet available
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500">
            Port 17003 · Planned feature
          </p>
        </div>
      </div>

      {/* Sample bar chart */}
      <div className="px-4 pb-3">
        <div className="h-12 opacity-30 pointer-events-none" aria-hidden="true">
          <ResponsiveBar
            data={data.barData}
            keys={['value']}
            indexBy="label"
            theme={nivoTheme}
            colors={[data.color]}
            enableLabel={false}
            axisLeft={null}
            axisBottom={{ tickSize: 0, tickPadding: 2 }}
            margin={{ top: 0, right: 0, bottom: 20, left: 0 }}
            isInteractive={false}
            borderRadius={2}
          />
        </div>
      </div>
    </div>
  )
}

export default PlaceholderChart
