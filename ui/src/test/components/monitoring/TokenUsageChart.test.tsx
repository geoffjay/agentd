/**
 * Tests for TokenUsageChart component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TokenUsageChart } from '@/components/monitoring/TokenUsageChart'
import type { AgentUsageEntry } from '@/hooks/useUsageMetrics'

// Mock useNivoTheme to avoid ThemeProvider dependency in unit tests
vi.mock('@/hooks/useNivoTheme', () => ({ useNivoTheme: () => ({}) }))

// Mock Nivo charts
vi.mock('@nivo/bar', () => ({
  ResponsiveBar: ({ ariaLabel }: { ariaLabel: string }) => <div role="img" aria-label={ariaLabel} />,
}))

const ENTRIES: AgentUsageEntry[] = [
  {
    agentId: 'agent-1',
    name: 'Test Agent',
    stats: {
      agent_id: 'agent-1',
      cumulative: {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_input_tokens: 200,
        cache_creation_input_tokens: 100,
        total_cost_usd: 0.05,
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

const EMPTY_ENTRIES: AgentUsageEntry[] = []

describe('TokenUsageChart', () => {
  it('renders heading', () => {
    render(<TokenUsageChart entries={ENTRIES} />)
    expect(screen.getByText('Token Usage by Agent')).toBeTruthy()
  })

  it('renders bar chart when data is present', () => {
    render(<TokenUsageChart entries={ENTRIES} />)
    expect(screen.getByLabelText('Stacked bar chart of token usage per agent')).toBeTruthy()
  })

  it('shows loading skeleton when loading=true', () => {
    const { container } = render(<TokenUsageChart entries={[]} loading />)
    expect(container.querySelector('.animate-pulse')).toBeTruthy()
  })

  it('shows empty message when no data', () => {
    render(<TokenUsageChart entries={EMPTY_ENTRIES} />)
    expect(screen.getByText('No usage data available')).toBeTruthy()
  })

  it('renders legend with all token types', () => {
    render(<TokenUsageChart entries={ENTRIES} />)
    expect(screen.getByText('Input Tokens')).toBeTruthy()
    expect(screen.getByText('Output Tokens')).toBeTruthy()
    expect(screen.getByText('Cache Read')).toBeTruthy()
    expect(screen.getByText('Cache Creation')).toBeTruthy()
  })

  it('has aria-label on outer container', () => {
    render(<TokenUsageChart entries={ENTRIES} />)
    expect(screen.getAllByLabelText('Token usage stacked bar chart').length).toBeGreaterThanOrEqual(1)
  })
})
