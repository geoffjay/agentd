/**
 * Tests for CostOverviewChart component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { CostOverviewChart } from '@/components/monitoring/CostOverviewChart'
import type { AgentUsageEntry, AggregateUsage } from '@/hooks/useUsageMetrics'

// Mock useNivoTheme to avoid ThemeProvider dependency in unit tests
vi.mock('@/hooks/useNivoTheme', () => ({ useNivoTheme: () => ({}) }))

// Mock Nivo charts
vi.mock('@nivo/bar', () => ({
  ResponsiveBar: ({ ariaLabel }: { ariaLabel: string }) => <div role="img" aria-label={ariaLabel} />,
}))

const AGGREGATE: AggregateUsage = {
  totalInputTokens: 2000,
  totalOutputTokens: 1000,
  totalCacheReadTokens: 500,
  totalCacheCreationTokens: 100,
  totalCostUsd: 0.25,
  totalTokens: 3600,
  cacheHitRatio: 0.19,
}

const ENTRIES: AgentUsageEntry[] = [
  {
    agentId: 'agent-1',
    name: 'Agent Alpha',
    stats: {
      agent_id: 'agent-1',
      cumulative: {
        input_tokens: 1200,
        output_tokens: 600,
        cache_read_input_tokens: 300,
        cache_creation_input_tokens: 50,
        total_cost_usd: 0.15,
        num_turns: 8,
        duration_ms: 4000,
        duration_api_ms: 2500,
        result_count: 4,
        started_at: '2024-01-01T00:00:00Z',
      },
      session_count: 1,
    },
  },
  {
    agentId: 'agent-2',
    name: 'Agent Beta',
    stats: {
      agent_id: 'agent-2',
      cumulative: {
        input_tokens: 800,
        output_tokens: 400,
        cache_read_input_tokens: 200,
        cache_creation_input_tokens: 50,
        total_cost_usd: 0.10,
        num_turns: 5,
        duration_ms: 3000,
        duration_api_ms: 2000,
        result_count: 3,
        started_at: '2024-01-01T00:00:00Z',
      },
      session_count: 1,
    },
  },
]

const EMPTY_AGGREGATE: AggregateUsage = {
  totalInputTokens: 0,
  totalOutputTokens: 0,
  totalCacheReadTokens: 0,
  totalCacheCreationTokens: 0,
  totalCostUsd: 0,
  totalTokens: 0,
  cacheHitRatio: 0,
}

describe('CostOverviewChart', () => {
  it('renders heading', () => {
    render(<CostOverviewChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByText('Cost per Agent')).toBeTruthy()
  })

  it('displays total cost', () => {
    render(<CostOverviewChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByText('$0.25')).toBeTruthy()
  })

  it('renders bar chart when data is present', () => {
    render(<CostOverviewChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByLabelText('Cost per agent bar chart')).toBeTruthy()
  })

  it('shows loading skeleton when loading=true', () => {
    const { container } = render(
      <CostOverviewChart entries={[]} aggregate={EMPTY_AGGREGATE} loading />,
    )
    expect(container.querySelector('.animate-pulse')).toBeTruthy()
  })

  it('shows empty message when no data', () => {
    render(<CostOverviewChart entries={[]} aggregate={EMPTY_AGGREGATE} />)
    expect(screen.getByText('No cost data available')).toBeTruthy()
  })

  it('shows agent count in footer', () => {
    render(<CostOverviewChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByText('2')).toBeTruthy()
  })

  it('has aria-label on outer container', () => {
    render(<CostOverviewChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByLabelText('Cost overview bar chart')).toBeTruthy()
  })
})
