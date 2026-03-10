/**
 * AgentSummary — shows agent status distribution as a donut chart
 * plus the 5 most recently updated agents.
 */

import { useNavigate } from 'react-router-dom'
import { ResponsivePie } from '@nivo/pie'
import { DollarSign, Hash, Layers, Plus, RefreshCw } from 'lucide-react'
import { StatusBadge } from '@/components/common/StatusBadge'
import { ChartSkeleton, ListItemSkeleton } from '@/components/common/LoadingSkeleton'
import type { UseAgentSummaryResult } from '@/hooks/useAgentSummary'
import type { Agent } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Pie chart colour map
// ---------------------------------------------------------------------------

const STATUS_COLORS: Record<string, string> = {
  Running: '#22c55e',
  Pending: '#f59e0b',
  Stopped: '#94a3b8',
  Failed: '#ef4444',
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface AgentRowProps {
  agent: Agent
}

function AgentRow({ agent }: AgentRowProps) {
  const navigate = useNavigate()
  return (
    <li
      className="flex cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 hover:bg-gray-50 dark:hover:bg-gray-700/50"
      onClick={() => navigate(`/agents`)}
    >
      <div className="min-w-0">
        <p className="truncate text-sm font-medium text-gray-900 dark:text-white">{agent.name}</p>
        <p className="text-xs text-gray-500 dark:text-gray-400">
          {formatRelativeTime(new Date(agent.updated_at))}
        </p>
      </div>
      <StatusBadge status={agent.status} />
    </li>
  )
}

// ---------------------------------------------------------------------------
// AgentSummary
// ---------------------------------------------------------------------------

interface AgentSummaryProps extends UseAgentSummaryResult {
  onCreateAgent?: () => void
}

export function AgentSummary({
  counts,
  recentAgents,
  total,
  aggregateUsage,
  loading,
  error,
  onCreateAgent,
}: AgentSummaryProps) {
  const pieData = [
    { id: 'Running', label: 'Running', value: counts.Running, color: STATUS_COLORS.Running },
    { id: 'Pending', label: 'Pending', value: counts.Pending, color: STATUS_COLORS.Pending },
    { id: 'Stopped', label: 'Stopped', value: counts.Stopped, color: STATUS_COLORS.Stopped },
    { id: 'Failed', label: 'Failed', value: counts.Failed, color: STATUS_COLORS.Failed },
  ].filter((d) => d.value > 0)

  const hasData = total > 0

  return (
    <section
      aria-labelledby="agent-summary-heading"
      className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2
          id="agent-summary-heading"
          className="text-base font-semibold text-gray-900 dark:text-white"
        >
          Agents
          {!loading && (
            <span className="ml-2 text-sm font-normal text-gray-500 dark:text-gray-400">
              ({total} total)
            </span>
          )}
        </h2>
        <button
          type="button"
          onClick={onCreateAgent}
          aria-label="Create new agent"
          className="flex items-center gap-1.5 rounded-md bg-primary-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-primary-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500"
        >
          <Plus size={14} />
          Create Agent
        </button>
      </div>

      {/* Error state */}
      {error && (
        <div className="mt-4 flex items-center gap-2 text-sm text-red-500">
          <RefreshCw size={14} />
          {error}
        </div>
      )}

      {/* Loading */}
      {loading && !error && (
        <div className="mt-4">
          <ChartSkeleton height={160} />
          <div className="mt-4">
            <ListItemSkeleton rows={3} />
          </div>
        </div>
      )}

      {/* Content */}
      {!loading && !error && (
        <>
          {/* Donut chart */}
          {hasData ? (
            <div className="mt-4 h-40">
              <ResponsivePie
                data={pieData}
                colors={{ datum: 'data.color' }}
                innerRadius={0.6}
                padAngle={2}
                cornerRadius={3}
                enableArcLabels={false}
                enableArcLinkLabels={false}
                tooltip={({ datum }) => (
                  <div className="rounded bg-gray-900 px-2 py-1 text-xs text-white shadow">
                    {datum.label}: {datum.value}
                  </div>
                )}
                legends={[
                  {
                    anchor: 'right',
                    direction: 'column',
                    itemWidth: 80,
                    itemHeight: 16,
                    itemsSpacing: 4,
                    symbolSize: 10,
                    symbolShape: 'circle',
                    itemTextColor: '#94a3b8',
                    translateX: 90,
                  },
                ]}
                margin={{ top: 8, right: 100, bottom: 8, left: 8 }}
              />
            </div>
          ) : (
            <p className="mt-4 text-sm text-gray-500 dark:text-gray-400">No agents yet.</p>
          )}

          {/* Status count pills */}
          {hasData && (
            <div className="mt-3 flex flex-wrap gap-2">
              {Object.entries(counts).map(([status, count]) =>
                count > 0 ? (
                  <span
                    key={status}
                    className="flex items-center gap-1 rounded-full bg-gray-100 px-2.5 py-0.5 text-xs font-medium text-gray-700 dark:bg-gray-700 dark:text-gray-300"
                  >
                    <span
                      className="h-2 w-2 rounded-full"
                      style={{ background: STATUS_COLORS[status] }}
                    />
                    {count} {status}
                  </span>
                ) : null,
              )}
            </div>
          )}

          {/* Aggregate usage stats */}
          {aggregateUsage && (
            <div className="mt-3 grid grid-cols-3 gap-2" data-testid="aggregate-usage">
              <div className="rounded-md bg-gray-50 px-3 py-2 dark:bg-gray-700/50">
                <div className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
                  <DollarSign size={12} />
                  Total Cost
                </div>
                <p className="mt-0.5 text-sm font-semibold text-gray-900 dark:text-white">
                  ${aggregateUsage.totalCostUsd < 0.01 && aggregateUsage.totalCostUsd > 0
                    ? '<0.01'
                    : aggregateUsage.totalCostUsd.toFixed(2)}
                </p>
              </div>
              <div className="rounded-md bg-gray-50 px-3 py-2 dark:bg-gray-700/50">
                <div className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
                  <Hash size={12} />
                  Tokens
                </div>
                <p className="mt-0.5 text-sm font-semibold text-gray-900 dark:text-white">
                  {aggregateUsage.totalTokens >= 1_000_000
                    ? `${(aggregateUsage.totalTokens / 1_000_000).toFixed(1)}M`
                    : aggregateUsage.totalTokens >= 1_000
                      ? `${(aggregateUsage.totalTokens / 1_000).toFixed(1)}k`
                      : aggregateUsage.totalTokens}
                </p>
              </div>
              <div className="rounded-md bg-gray-50 px-3 py-2 dark:bg-gray-700/50">
                <div className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
                  <Layers size={12} />
                  Cache Hit
                </div>
                <p className="mt-0.5 text-sm font-semibold text-gray-900 dark:text-white">
                  {aggregateUsage.cacheHitPercent.toFixed(0)}%
                </p>
              </div>
            </div>
          )}

          {/* Recent agents list */}
          {recentAgents.length > 0 && (
            <div className="mt-4">
              <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
                Recently Active
              </p>
              <ul role="list" className="space-y-0.5">
                {recentAgents.map((agent) => (
                  <AgentRow key={agent.id} agent={agent} />
                ))}
              </ul>
            </div>
          )}
        </>
      )}
    </section>
  )
}

export default AgentSummary

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
