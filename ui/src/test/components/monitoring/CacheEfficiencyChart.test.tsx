/**
 * Tests for CacheEfficiencyChart component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { CacheEfficiencyChart } from '@/components/monitoring/CacheEfficiencyChart'
import type { AgentUsageEntry, AggregateUsage } from '@/hooks/useUsageMetrics'

// Mock useNivoTheme to avoid ThemeProvider dependency in unit tests
vi.mock('@/hooks/useNivoTheme', () => ({ useNivoTheme: () => ({}) }))

// Mock Nivo charts
vi.mock('@nivo/pie', () => ({
  ResponsivePie: () => <div role="img" aria-label="mock-pie" />,
}))

const AGGREGATE: AggregateUsage = {
  totalInputTokens: 1000,
  totalOutputTokens: 500,
  totalCacheReadTokens: 800,
  totalCacheCreationTokens: 200,
  totalCostUsd: 0.10,
  totalTokens: 2500,
  cacheHitRatio: 0.4,
}

const ENTRIES: AgentUsageEntry[] = [
  {
    agentId: 'agent-1',
    name: 'Agent Alpha',
    stats: {
      agent_id: 'agent-1',
      cumulative: {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_input_tokens: 800,
        cache_creation_input_tokens: 200,
        total_cost_usd: 0.10,
        num_turns: 10,
        duration_ms: 5000,
        duration_api_ms: 3000,
        result_count: 5,
        started_at: '2024-01-01T00:00:00Z',
      },
      session_count: 2,
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

describe('CacheEfficiencyChart', () => {
  it('renders heading', () => {
    render(<CacheEfficiencyChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByText('Cache Efficiency')).toBeTruthy()
  })

  it('shows cache hit percentage in center', () => {
    render(<CacheEfficiencyChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByText('40.0%')).toBeTruthy()
    expect(screen.getByText('Cache Hit')).toBeTruthy()
  })

  it('shows loading skeleton when loading=true', () => {
    const { container } = render(
      <CacheEfficiencyChart entries={[]} aggregate={EMPTY_AGGREGATE} loading />,
    )
    expect(container.querySelector('.animate-pulse')).toBeTruthy()
  })

  it('shows empty message when no data', () => {
    render(<CacheEfficiencyChart entries={[]} aggregate={EMPTY_AGGREGATE} />)
    expect(screen.getByText('No cache data available')).toBeTruthy()
  })

  it('renders agent dropdown when entries exist', () => {
    render(<CacheEfficiencyChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByLabelText('Select agent for cache breakdown')).toBeTruthy()
  })

  it('shows All Agents as default option', () => {
    render(<CacheEfficiencyChart entries={ENTRIES} aggregate={AGGREGATE} />)
    const select = screen.getByLabelText('Select agent for cache breakdown') as HTMLSelectElement
    expect(select.value).toBe('all')
  })

  it('can switch to per-agent view', () => {
    render(<CacheEfficiencyChart entries={ENTRIES} aggregate={AGGREGATE} />)
    const select = screen.getByLabelText('Select agent for cache breakdown')
    fireEvent.change(select, { target: { value: 'agent-1' } })
    expect((select as HTMLSelectElement).value).toBe('agent-1')
  })

  it('renders legend items', () => {
    render(<CacheEfficiencyChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByText('Cache Hits')).toBeTruthy()
    expect(screen.getByText('Cache Misses')).toBeTruthy()
    expect(screen.getByText('Non-Cached')).toBeTruthy()
  })

  it('has aria-label on outer container', () => {
    render(<CacheEfficiencyChart entries={ENTRIES} aggregate={AGGREGATE} />)
    expect(screen.getByLabelText('Cache efficiency donut chart')).toBeTruthy()
  })
})
