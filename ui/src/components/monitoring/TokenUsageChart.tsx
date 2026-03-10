/**
 * TokenUsageChart — Nivo stacked bar chart showing per-agent token usage.
 *
 * X-axis: agent names (truncated IDs as fallback)
 * Y-axis: token count
 * Stacked series: input tokens, output tokens, cache read tokens, cache creation tokens
 */

import { ResponsiveBar } from '@nivo/bar'
import { ChartSkeleton } from '@/components/common/LoadingSkeleton'
import { useNivoTheme } from '@/hooks/useNivoTheme'
import type { AgentUsageEntry } from '@/hooks/useUsageMetrics'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TokenUsageChartProps {
  entries: AgentUsageEntry[]
  loading?: boolean
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TOKEN_KEYS = [
  'input_tokens',
  'output_tokens',
  'cache_read_tokens',
  'cache_creation_tokens',
] as const

const TOKEN_LABELS: Record<string, string> = {
  input_tokens: 'Input Tokens',
  output_tokens: 'Output Tokens',
  cache_read_tokens: 'Cache Read',
  cache_creation_tokens: 'Cache Creation',
}

const TOKEN_COLORS: Record<string, string> = {
  input_tokens: '#3b82f6',      // blue
  output_tokens: '#8b5cf6',     // violet
  cache_read_tokens: '#22c55e', // green
  cache_creation_tokens: '#f59e0b', // amber
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function truncateLabel(name: string, maxLen = 12): string {
  return name.length > maxLen ? `${name.slice(0, maxLen)}…` : name
}

function formatTokenCount(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`
  return String(value)
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function TokenUsageChart({ entries, loading }: TokenUsageChartProps) {
  const nivoTheme = useNivoTheme()

  const barData = entries.map((entry) => ({
    agent: truncateLabel(entry.name || entry.agentId),
    agentFull: entry.name || entry.agentId,
    input_tokens: entry.stats.cumulative.input_tokens,
    output_tokens: entry.stats.cumulative.output_tokens,
    cache_read_tokens: entry.stats.cumulative.cache_read_input_tokens,
    cache_creation_tokens: entry.stats.cumulative.cache_creation_input_tokens,
  }))

  const hasData = barData.some(
    (d) => d.input_tokens + d.output_tokens + d.cache_read_tokens + d.cache_creation_tokens > 0,
  )

  return (
    <div
      className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5"
      aria-label="Token usage stacked bar chart"
    >
      <h3 className="mb-4 text-sm font-semibold text-gray-900 dark:text-white">
        Token Usage by Agent
      </h3>

      {loading ? (
        <ChartSkeleton height={240} />
      ) : !hasData ? (
        <div className="flex items-center justify-center h-60 text-sm text-gray-400 dark:text-gray-500">
          No usage data available
        </div>
      ) : (
        <>
          <div style={{ height: 240 }} role="img" aria-label="Stacked bar chart of token usage per agent">
            <ResponsiveBar
              data={barData}
              keys={[...TOKEN_KEYS]}
              indexBy="agent"
              theme={nivoTheme}
              colors={({ id }) => TOKEN_COLORS[id as string] ?? '#94a3b8'}
              margin={{ top: 8, right: 8, bottom: 40, left: 56 }}
              padding={0.3}
              borderRadius={2}
              enableLabel={false}
              axisLeft={{
                tickSize: 0,
                tickPadding: 4,
                tickValues: 5,
                format: formatTokenCount,
              }}
              axisBottom={{
                tickSize: 0,
                tickPadding: 6,
                tickRotation: entries.length > 6 ? -35 : 0,
              }}
              tooltip={({ id, value, indexValue, data: rowData }) => (
                <div className="rounded-md border border-gray-200 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-xs shadow-lg">
                  <div className="font-medium text-gray-900 dark:text-white mb-1">
                    {(rowData as Record<string, unknown>).agentFull as string}
                  </div>
                  <div className="flex items-center gap-1.5">
                    <span
                      className="h-2 w-2 rounded-full flex-shrink-0"
                      style={{ background: TOKEN_COLORS[id as string] }}
                    />
                    <span className="text-gray-600 dark:text-gray-300">
                      {TOKEN_LABELS[id as string] ?? String(id)}:
                    </span>
                    <span className="font-medium text-gray-900 dark:text-white">
                      {formatTokenCount(value)}
                    </span>
                  </div>
                </div>
              )}
              role="img"
              ariaLabel="Token usage stacked bar chart"
            />
          </div>

          {/* Legend */}
          <div className="mt-3 flex flex-wrap gap-3">
            {TOKEN_KEYS.map((key) => (
              <span
                key={key}
                className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400"
              >
                <span
                  className="h-2 w-2 rounded-full flex-shrink-0"
                  style={{ background: TOKEN_COLORS[key] }}
                />
                {TOKEN_LABELS[key]}
              </span>
            ))}
          </div>
        </>
      )}
    </div>
  )
}

export default TokenUsageChart
