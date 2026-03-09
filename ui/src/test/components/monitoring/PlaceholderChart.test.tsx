/**
 * Tests for PlaceholderChart component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { PlaceholderChart } from '@/components/monitoring/PlaceholderChart'

// Mock useNivoTheme to avoid ThemeProvider dependency in unit tests
vi.mock('@/hooks/useNivoTheme', () => ({ useNivoTheme: () => ({}) }))

// Nivo charts use canvas/SVG; mock them to avoid heavy rendering
vi.mock('@nivo/line', () => ({
  ResponsiveLine: () => <div data-testid="responsive-line" />,
}))
vi.mock('@nivo/bar', () => ({
  ResponsiveBar: () => <div data-testid="responsive-bar" />,
}))

describe('PlaceholderChart', () => {
  it('renders the chart title', () => {
    render(<PlaceholderChart variant="cpu" title="CPU Usage" />)
    expect(screen.getByText('CPU Usage')).toBeTruthy()
  })

  it('renders "Coming Soon" badge', () => {
    render(<PlaceholderChart variant="memory" title="Memory" />)
    expect(screen.getByText('Coming Soon')).toBeTruthy()
  })

  it('renders description when provided', () => {
    render(<PlaceholderChart variant="disk" title="Disk" description="Per-mount utilisation" />)
    expect(screen.getByText('Per-mount utilisation')).toBeTruthy()
  })

  it('does not render description when not provided', () => {
    render(<PlaceholderChart variant="network" title="Network" />)
    expect(screen.queryByText('Per-mount utilisation')).toBeNull()
  })

  it('shows "Monitor service not yet available" overlay', () => {
    render(<PlaceholderChart variant="cpu" title="CPU" />)
    expect(screen.getByText('Monitor service not yet available')).toBeTruthy()
  })

  it('renders for all variants without crashing', () => {
    const variants = ['cpu', 'memory', 'disk', 'network'] as const
    for (const variant of variants) {
      const { unmount } = render(<PlaceholderChart variant={variant} title={variant} />)
      unmount()
    }
  })
})
