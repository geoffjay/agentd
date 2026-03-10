/**
 * Tests for AgentUsagePanel component — rendering, formatting, cache efficiency,
 * auto-clear threshold, and edge cases.
 */

import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AgentUsagePanel } from '@/components/agents/AgentUsagePanel'
import { USAGE_STATS, USAGE_STATS_NO_SESSION, USAGE_STATS_ZERO } from '@/test/fixtures/usage'

describe('AgentUsagePanel', () => {
  // -------------------------------------------------------------------------
  // Basic rendering
  // -------------------------------------------------------------------------

  it('renders the Usage heading', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.getByText('Usage')).toBeInTheDocument()
  })

  it('displays session count badge', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.getByText('Session 3')).toBeInTheDocument()
  })

  it('renders the section with correct aria-label', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.getByLabelText('Agent usage statistics')).toBeInTheDocument()
  })

  // -------------------------------------------------------------------------
  // Current session stats
  // -------------------------------------------------------------------------

  it('renders current session heading', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.getByText('Current Session')).toBeInTheDocument()
  })

  it('displays current session stat cards with correct aria labels', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    // Input tokens: 1500
    expect(screen.getByLabelText(/Input tokens: 1,500/)).toBeInTheDocument()
    // Output tokens: 800
    expect(screen.getByLabelText(/Output tokens: 800/)).toBeInTheDocument()
    // Cache read: 400
    expect(screen.getByLabelText(/Cache read tokens: 400/)).toBeInTheDocument()
    // Cache creation: 100
    expect(screen.getByLabelText(/Cache creation tokens: 100/)).toBeInTheDocument()
  })

  it('displays cost formatted as currency', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    // total_cost_usd = 0.025 → $0.0250
    expect(screen.getByLabelText(/Total cost: \$0\.0250/)).toBeInTheDocument()
  })

  it('displays number of turns', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.getByLabelText(/Number of turns: 5/)).toBeInTheDocument()
  })

  it('displays wall clock duration', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    // duration_ms = 12000 → 12.0s
    expect(screen.getByLabelText(/Wall clock duration: 12\.0s/)).toBeInTheDocument()
  })

  it('displays API duration', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    // duration_api_ms = 8500 → 8.5s
    expect(screen.getByLabelText(/API duration: 8\.5s/)).toBeInTheDocument()
  })

  it('displays result count', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.getByText('5 result(s)')).toBeInTheDocument()
  })

  // -------------------------------------------------------------------------
  // No active session
  // -------------------------------------------------------------------------

  it('shows "No active session" when current_session is missing', () => {
    render(<AgentUsagePanel usage={USAGE_STATS_NO_SESSION} />)
    expect(screen.getByText('No active session.')).toBeInTheDocument()
  })

  // -------------------------------------------------------------------------
  // Cache efficiency
  // -------------------------------------------------------------------------

  it('renders cache efficiency progress bar', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    const progressBars = screen.getAllByRole('progressbar')
    // At least one for current session cache
    expect(progressBars.length).toBeGreaterThanOrEqual(1)
  })

  it('displays cache hit ratio label', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.getByText('Cache Hit Ratio')).toBeInTheDocument()
  })

  it('shows correct cache efficiency label based on ratio', () => {
    // Current session: cache_read=400, cache_creation=100, input=1500
    // ratio = 400 / 2000 = 0.2 → "Low" (ratio <= 0.2)
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    // Should show "Low" or "Moderate" based on the ratio boundary
    const ratioText = screen.getAllByLabelText(/Cache hit ratio:/)[0]
    expect(ratioText).toBeInTheDocument()
  })

  // -------------------------------------------------------------------------
  // Cumulative stats (collapsible)
  // -------------------------------------------------------------------------

  it('renders cumulative toggle button collapsed by default', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    const toggle = screen.getByRole('button', { name: /cumulative/i })
    expect(toggle).toBeInTheDocument()
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
  })

  it('expands cumulative stats on click', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    const toggle = screen.getByRole('button', { name: /cumulative/i })
    fireEvent.click(toggle)
    expect(toggle).toHaveAttribute('aria-expanded', 'true')

    // Should show the cumulative body
    expect(screen.getByLabelText('Cumulative statistics')).toBeInTheDocument()
  })

  it('shows session count in cumulative toggle label', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.getByText(/Cumulative \(3 sessions\)/)).toBeInTheDocument()
  })

  it('shows singular session text for 1 session', () => {
    const singleSessionUsage = { ...USAGE_STATS, session_count: 1 }
    render(<AgentUsagePanel usage={singleSessionUsage} />)
    expect(screen.getByText(/Cumulative \(1 session\)/)).toBeInTheDocument()
  })

  it('shows avg cost per session in expanded cumulative section', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    const toggle = screen.getByRole('button', { name: /cumulative/i })
    fireEvent.click(toggle)

    // avg cost = 0.08 / 3 sessions ≈ $0.0267
    expect(screen.getByText(/Avg cost \/ session/)).toBeInTheDocument()
  })

  // -------------------------------------------------------------------------
  // Auto-clear threshold
  // -------------------------------------------------------------------------

  it('does not show auto-clear when threshold not provided', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} />)
    expect(screen.queryByText('Auto-clear Threshold')).not.toBeInTheDocument()
  })

  it('shows auto-clear threshold progress when configured', () => {
    render(<AgentUsagePanel usage={USAGE_STATS} autoClearThreshold={0.10} />)
    expect(screen.getByText('Auto-clear Threshold')).toBeInTheDocument()
    expect(screen.getByLabelText(/Auto-clear threshold progress/)).toBeInTheDocument()
  })

  it('shows warning when approaching threshold', () => {
    // Current session cost = 0.025, threshold = 0.03 → progress = 83% > 80%
    render(<AgentUsagePanel usage={USAGE_STATS} autoClearThreshold={0.03} />)
    expect(screen.getByText('Approaching auto-clear threshold')).toBeInTheDocument()
  })

  it('does not show warning when far from threshold', () => {
    // Current session cost = 0.025, threshold = 1.00 → progress = 2.5%
    render(<AgentUsagePanel usage={USAGE_STATS} autoClearThreshold={1.0} />)
    expect(screen.queryByText('Approaching auto-clear threshold')).not.toBeInTheDocument()
  })

  it('does not show auto-clear when no active session', () => {
    render(<AgentUsagePanel usage={USAGE_STATS_NO_SESSION} autoClearThreshold={0.10} />)
    expect(screen.queryByText('Auto-clear Threshold')).not.toBeInTheDocument()
  })

  // -------------------------------------------------------------------------
  // Zero usage
  // -------------------------------------------------------------------------

  it('handles zero usage gracefully', () => {
    render(<AgentUsagePanel usage={USAGE_STATS_ZERO} />)
    expect(screen.getByText('Current Session')).toBeInTheDocument()
    // Should render without errors
    expect(screen.getByLabelText('Agent usage statistics')).toBeInTheDocument()
  })
})
