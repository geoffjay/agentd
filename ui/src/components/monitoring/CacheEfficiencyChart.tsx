/**
 * CacheEfficiencyChart — Nivo donut chart showing aggregate prompt cache
 * efficiency across all agents.
 *
 * Segments:
 * - Cache read tokens (hits) — green
 * - Cache creation tokens (misses) — amber
 * - Non-cached input tokens — gray
 *
 * Center label shows overall cache hit percentage.
 * Optional per-agent breakdown via a dropdown toggle.
 */

import { useState, useMemo } from 'react'
import { ResponsivePie } from '@nivo/pie'
import { ChevronDown } from 'lucide-react'
import { ChartSkeleton } from '@/components/common/LoadingSkeleton'
import { useNivoTheme } from '@/hooks/useNivoTheme'
import type { AgentUsageEntry, AggregateUsage } from '@/hooks/useUsageMetrics'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CacheEfficiencyChartProps {
  entries: AgentUsageEntry[]
  aggregate: AggregateUsage
  loading?: boolean
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SEGMENT_COLORS = {
  cacheRead: '#22c55e',      // green — cache hits
  cacheCreation: '#f59e0b',  // amber — cache misses
  nonCached: '#94a3b8',      // gray  — non-cached input
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function pct(ratio: number): string {
  return `${(ratio * 100).toFixed(1)}%`
}

function formatTokenCount(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`
  return String(value)
}

interface PieSegment {
  id: string
  label: string
  value: number
  color: string
}

function buildSegments(
  cacheRead: number,
  cacheCreation: number,
  nonCached: number,
): PieSegment[] {
  return [
    { id: 'cache_read', label: 'Cache Hits', value: cacheRead, color: SEGMENT_COLORS.cacheRead },
    { id: 'cache_creation', label: 'Cache Misses', value: cacheCreation, color: SEGMENT_COLORS.cacheCreation },
    { id: 'non_cached', label: 'Non-Cached', value: nonCached, color: SEGMENT_COLORS.nonCached },
  ].filter((s) => s.value > 0)
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function CacheEfficiencyChart({ entries, aggregate, loading }: CacheEfficiencyChartProps) {
  const nivoTheme = useNivoTheme()
  const [selectedAgent, setSelectedAgent] = useState<string>('all')

  // Build segment data for the selected view
  const { segments, hitRatio } = useMemo(() => {
    if (selectedAgent === 'all') {
      const segs = buildSegments(
        aggregate.totalCacheReadTokens,
        aggregate.totalCacheCreationTokens,
        aggregate.totalInputTokens,
      )
      return { segments: segs, hitRatio: aggregate.cacheHitRatio }
    }

    const entry = entries.find((e) => e.agentId === selectedAgent)
    if (!entry) return { segments: [] as PieSegment[], hitRatio: 0 }

    const c = entry.stats.cumulative
    const total = c.cache_read_input_tokens + c.cache_creation_input_tokens + c.input_tokens
    const ratio = total > 0 ? c.cache_read_input_tokens / total : 0

    return {
      segments: buildSegments(
        c.cache_read_input_tokens,
        c.cache_creation_input_tokens,
        c.input_tokens,
      ),
      hitRatio: ratio,
    }
  }, [selectedAgent, entries, aggregate])

  const hasData = segments.length > 0

  return (
    <div
      className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5"
      aria-label="Cache efficiency donut chart"
    >
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
          Cache Efficiency
        </h3>

        {/* Agent dropdown */}
        {entries.length > 0 && (
          <div className="relative">
            <select
              value={selectedAgent}
              onChange={(e) => setSelectedAgent(e.target.value)}
              aria-label="Select agent for cache breakdown"
              className="appearance-none rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 pl-2.5 pr-7 py-1 text-xs text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors cursor-pointer"
            >
              <option value="all">All Agents</option>
              {entries.map((entry) => (
                <option key={entry.agentId} value={entry.agentId}>
                  {entry.name || entry.agentId.slice(0, 12)}
                </option>
              ))}
            </select>
            <ChevronDown
              size={12}
              className="absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none text-gray-400"
            />
          </div>
        )}
      </div>

      {loading ? (
        <ChartSkeleton height={240} />
      ) : !hasData ? (
        <div className="flex items-center justify-center h-60 text-sm text-gray-400 dark:text-gray-500">
          No cache data available
        </div>
      ) : (
        <>
          <div style={{ height: 240 }} className="relative" role="img" aria-label="Donut chart showing cache hit ratio">
            <ResponsivePie
              data={segments}
              theme={nivoTheme}
              colors={({ data }) => data.color}
              margin={{ top: 8, right: 60, bottom: 8, left: 60 }}
              innerRadius={0.6}
              padAngle={2}
              cornerRadius={3}
              arcLinkLabel="label"
              arcLinkLabelsSkipAngle={10}
              arcLinkLabelsThickness={1}
              arcLinkLabelsColor={{ from: 'color' }}
              enableArcLabels={false}
              tooltip={({ datum }) => (
                <div className="rounded-md border border-gray-200 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-xs shadow-lg">
                  <div className="flex items-center gap-1.5">
                    <span
                      className="h-2 w-2 rounded-full flex-shrink-0"
                      style={{ background: datum.color }}
                    />
                    <span className="text-gray-600 dark:text-gray-300">{datum.label}:</span>
                    <span className="font-medium text-gray-900 dark:text-white">
                      {formatTokenCount(datum.value)}
                    </span>
                  </div>
                </div>
              )}
              role="img"
            />

            {/* Center label — cache hit percentage */}
            <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
              <div className="text-center">
                <div className="text-2xl font-bold text-gray-900 dark:text-white">
                  {pct(hitRatio)}
                </div>
                <div className="text-xs text-gray-500 dark:text-gray-400">Cache Hit</div>
              </div>
            </div>
          </div>

          {/* Legend */}
          <div className="mt-3 flex flex-wrap gap-3">
            {[
              { label: 'Cache Hits', color: SEGMENT_COLORS.cacheRead },
              { label: 'Cache Misses', color: SEGMENT_COLORS.cacheCreation },
              { label: 'Non-Cached', color: SEGMENT_COLORS.nonCached },
            ].map((item) => (
              <span
                key={item.label}
                className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400"
              >
                <span
                  className="h-2 w-2 rounded-full flex-shrink-0"
                  style={{ background: item.color }}
                />
                {item.label}
              </span>
            ))}
          </div>
        </>
      )}
    </div>
  )
}

export default CacheEfficiencyChart
