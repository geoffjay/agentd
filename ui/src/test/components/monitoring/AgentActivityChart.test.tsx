/**
 * Tests for AgentActivityChart component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AgentActivityChart } from '@/components/monitoring/AgentActivityChart'
import type { AgentStatusCounts, AgentTimePoint } from '@/hooks/useMetrics'

// Mock useNivoTheme to avoid ThemeProvider dependency in unit tests
vi.mock('@/hooks/useNivoTheme', () => ({ useNivoTheme: () => ({}) }))

// Mock Nivo charts
vi.mock('@nivo/bar', () => ({
  ResponsiveBar: ({ ariaLabel }: { ariaLabel: string }) => <div role="img" aria-label={ariaLabel} />,
}))
vi.mock('@nivo/pie', () => ({
  ResponsivePie: ({ ariaLabel }: { ariaLabel: string }) => <div role="img" aria-label={ariaLabel} />,
}))
vi.mock('@nivo/line', () => ({
  ResponsiveLine: ({ ariaLabel }: { ariaLabel: string }) => <div role="img" aria-label={ariaLabel} />,
}))

const COUNTS: AgentStatusCounts = { Running: 3, Pending: 1, Stopped: 2, Failed: 0 }
const TIME_SERIES: AgentTimePoint[] = [
  { x: '2024-01-01T00:00:00Z', y: 2 },
  { x: '2024-01-01T00:00:30Z', y: 3 },
]

describe('AgentActivityChart', () => {
  it('renders "Agent Activity" heading', () => {
    render(<AgentActivityChart counts={COUNTS} timeSeries={TIME_SERIES} />)
    expect(screen.getByText('Agent Activity')).toBeTruthy()
  })

  it('defaults to bar chart view', () => {
    render(<AgentActivityChart counts={COUNTS} timeSeries={TIME_SERIES} />)
    expect(screen.getByLabelText('Agent status distribution bar chart')).toBeTruthy()
  })

  it('switches to pie chart when Pie button clicked', () => {
    render(<AgentActivityChart counts={COUNTS} timeSeries={TIME_SERIES} />)
    fireEvent.click(screen.getByText('Pie'))
    expect(screen.getByLabelText('Agent status distribution pie chart')).toBeTruthy()
  })

  it('switches to line chart when "Over time" button clicked', () => {
    render(<AgentActivityChart counts={COUNTS} timeSeries={TIME_SERIES} />)
    fireEvent.click(screen.getByText('Over time'))
    expect(screen.getByLabelText('Running agent count over time')).toBeTruthy()
  })

  it('shows loading skeleton when loading=true', () => {
    const { container } = render(<AgentActivityChart counts={COUNTS} timeSeries={[]} loading />)
    // ChartSkeleton renders with animate-pulse
    expect(container.querySelector('.animate-pulse')).toBeTruthy()
  })

  it('shows status legend with counts', () => {
    render(<AgentActivityChart counts={COUNTS} timeSeries={TIME_SERIES} />)
    expect(screen.getByText(/3 Running/)).toBeTruthy()
    expect(screen.getByText(/1 Pending/)).toBeTruthy()
  })

  it('has aria-label on chart container', () => {
    render(<AgentActivityChart counts={COUNTS} timeSeries={TIME_SERIES} />)
    expect(screen.getByLabelText('Agent activity charts')).toBeTruthy()
  })

  it('view buttons have aria-pressed attributes', () => {
    render(<AgentActivityChart counts={COUNTS} timeSeries={TIME_SERIES} />)
    const barButton = screen.getByText('Bar').closest('button')
    expect(barButton?.getAttribute('aria-pressed')).toBe('true')
    const pieButton = screen.getByText('Pie').closest('button')
    expect(pieButton?.getAttribute('aria-pressed')).toBe('false')
  })
})
