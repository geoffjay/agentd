/**
 * Tests for AgentSummary aggregate usage stats display.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import type { ReactNode } from 'react'
import { AgentSummary } from '@/components/dashboard/AgentSummary'
import type { UseAgentSummaryResult } from '@/hooks/useAgentSummary'

// Mock @nivo/pie to avoid rendering actual SVGs in tests
vi.mock('@nivo/pie', () => ({
  ResponsivePie: () => <div data-testid="mock-pie" />,
}))

function wrapper({ children }: { children: ReactNode }) {
  return <MemoryRouter>{children}</MemoryRouter>
}

const baseProps: UseAgentSummaryResult = {
  counts: { Running: 2, Pending: 0, Stopped: 1, Failed: 0 },
  recentAgents: [],
  total: 3,
  aggregateUsage: null,
  loading: false,
  error: undefined,
}

describe('AgentSummary aggregate usage', () => {
  it('does not render aggregate usage when aggregateUsage is null', () => {
    render(<AgentSummary {...baseProps} />, { wrapper })
    expect(screen.queryByTestId('aggregate-usage')).not.toBeInTheDocument()
  })

  it('renders total cost', () => {
    const props = {
      ...baseProps,
      aggregateUsage: { totalCostUsd: 12.34, totalTokens: 50000, cacheHitPercent: 45 },
    }
    render(<AgentSummary {...props} />, { wrapper })
    expect(screen.getByTestId('aggregate-usage')).toBeInTheDocument()
    expect(screen.getByText('$12.34')).toBeInTheDocument()
  })

  it('renders total tokens formatted as k', () => {
    const props = {
      ...baseProps,
      aggregateUsage: { totalCostUsd: 0.5, totalTokens: 150000, cacheHitPercent: 30 },
    }
    render(<AgentSummary {...props} />, { wrapper })
    expect(screen.getByText('150.0k')).toBeInTheDocument()
  })

  it('renders total tokens formatted as M', () => {
    const props = {
      ...baseProps,
      aggregateUsage: { totalCostUsd: 5.0, totalTokens: 2500000, cacheHitPercent: 60 },
    }
    render(<AgentSummary {...props} />, { wrapper })
    expect(screen.getByText('2.5M')).toBeInTheDocument()
  })

  it('renders cache hit percentage', () => {
    const props = {
      ...baseProps,
      aggregateUsage: { totalCostUsd: 1.0, totalTokens: 10000, cacheHitPercent: 72 },
    }
    render(<AgentSummary {...props} />, { wrapper })
    expect(screen.getByText('72%')).toBeInTheDocument()
  })

  it('renders small cost as <0.01', () => {
    const props = {
      ...baseProps,
      aggregateUsage: { totalCostUsd: 0.005, totalTokens: 100, cacheHitPercent: 0 },
    }
    render(<AgentSummary {...props} />, { wrapper })
    expect(screen.getByText('$<0.01')).toBeInTheDocument()
  })

  it('renders labels for all three stats', () => {
    const props = {
      ...baseProps,
      aggregateUsage: { totalCostUsd: 1.0, totalTokens: 1000, cacheHitPercent: 50 },
    }
    render(<AgentSummary {...props} />, { wrapper })
    expect(screen.getByText('Total Cost')).toBeInTheDocument()
    expect(screen.getByText('Tokens')).toBeInTheDocument()
    expect(screen.getByText('Cache Hit')).toBeInTheDocument()
  })

  it('does not render aggregate usage when loading', () => {
    const props = {
      ...baseProps,
      loading: true,
      aggregateUsage: { totalCostUsd: 1.0, totalTokens: 1000, cacheHitPercent: 50 },
    }
    render(<AgentSummary {...props} />, { wrapper })
    // Loading state shows skeletons, not the content
    expect(screen.queryByTestId('aggregate-usage')).not.toBeInTheDocument()
  })
})
