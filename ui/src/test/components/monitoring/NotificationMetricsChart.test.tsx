/**
 * Tests for NotificationMetricsChart component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { NotificationMetricsChart } from '@/components/monitoring/NotificationMetricsChart'
import type { NotificationCounts } from '@/hooks/useMetrics'

// Mock useNivoTheme to avoid ThemeProvider dependency in unit tests
vi.mock('@/hooks/useNivoTheme', () => ({ useNivoTheme: () => ({}) }))

// Mock Nivo chart
vi.mock('@nivo/bar', () => ({
  ResponsiveBar: ({ ariaLabel }: { ariaLabel: string }) => <div role="img" aria-label={ariaLabel} />,
}))

const FULL_COUNTS: NotificationCounts = {
  Low: 5,
  Normal: 12,
  High: 3,
  Urgent: 1,
  total: 100,
}

const EMPTY_COUNTS: NotificationCounts = {
  Low: 0,
  Normal: 0,
  High: 0,
  Urgent: 0,
  total: 0,
}

describe('NotificationMetricsChart', () => {
  it('renders "Notification Breakdown" heading', () => {
    render(<NotificationMetricsChart counts={FULL_COUNTS} />)
    expect(screen.getByText('Notification Breakdown')).toBeTruthy()
  })

  it('shows total count', () => {
    render(<NotificationMetricsChart counts={FULL_COUNTS} />)
    expect(screen.getByText('100')).toBeTruthy()
  })

  it('renders the bar chart with aria label', () => {
    render(<NotificationMetricsChart counts={FULL_COUNTS} />)
    expect(screen.getByLabelText('Notification count by priority')).toBeTruthy()
  })

  it('shows priority labels in legend', () => {
    render(<NotificationMetricsChart counts={FULL_COUNTS} />)
    expect(screen.getByText(/5 Low/)).toBeTruthy()
    expect(screen.getByText(/12 Normal/)).toBeTruthy()
    expect(screen.getByText(/3 High/)).toBeTruthy()
    expect(screen.getByText(/1 Urgent/)).toBeTruthy()
  })

  it('shows "No active notifications" when all counts are zero', () => {
    render(<NotificationMetricsChart counts={EMPTY_COUNTS} />)
    expect(screen.getByText('No active notifications')).toBeTruthy()
  })

  it('does not show "No active notifications" when counts exist', () => {
    render(<NotificationMetricsChart counts={FULL_COUNTS} />)
    expect(screen.queryByText('No active notifications')).toBeNull()
  })

  it('shows loading skeleton when loading=true', () => {
    const { container } = render(<NotificationMetricsChart counts={EMPTY_COUNTS} loading />)
    expect(container.querySelector('.animate-pulse')).toBeTruthy()
  })
})
