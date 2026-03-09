/**
 * Tests for EnvironmentStatus component.
 */

import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { EnvironmentStatus } from '@/components/questions/EnvironmentStatus'
import type { TmuxCheckResult } from '@/types/ask'

const TMUX_RUNNING: TmuxCheckResult = {
  running: true,
  session_count: 2,
  sessions: ['main', 'dev'],
}

const TMUX_STOPPED: TmuxCheckResult = {
  running: false,
  session_count: 0,
  sessions: [],
}

describe('EnvironmentStatus', () => {
  it('renders "Environment Status" heading', () => {
    render(<EnvironmentStatus />)
    expect(screen.getByText('Environment Status')).toBeTruthy()
  })

  it('shows placeholder message when no tmux data', () => {
    render(<EnvironmentStatus />)
    expect(screen.getByText(/No check results yet/)).toBeTruthy()
  })

  it('shows "Running" when tmux is running', () => {
    render(<EnvironmentStatus tmux={TMUX_RUNNING} />)
    expect(screen.getByText('Running')).toBeTruthy()
  })

  it('shows "Not running" when tmux is stopped', () => {
    render(<EnvironmentStatus tmux={TMUX_STOPPED} />)
    expect(screen.getByText('Not running')).toBeTruthy()
  })

  it('shows session count', () => {
    render(<EnvironmentStatus tmux={TMUX_RUNNING} />)
    expect(screen.getByText('2 active sessions')).toBeTruthy()
  })

  it('shows "No active sessions" when count is zero', () => {
    render(<EnvironmentStatus tmux={TMUX_STOPPED} />)
    expect(screen.getByText('No active sessions')).toBeTruthy()
  })

  it('shows session names as badges', () => {
    render(<EnvironmentStatus tmux={TMUX_RUNNING} />)
    expect(screen.getByText('main')).toBeTruthy()
    expect(screen.getByText('dev')).toBeTruthy()
  })

  it('shows loading skeletons when loading=true', () => {
    const { container } = render(<EnvironmentStatus loading />)
    expect(container.querySelector('.animate-pulse')).toBeTruthy()
  })

  it('toggles raw JSON view on button click', () => {
    render(<EnvironmentStatus tmux={TMUX_RUNNING} />)
    expect(screen.queryByText(/"running": true/)).toBeNull()
    fireEvent.click(screen.getByText('Show raw result'))
    expect(screen.getByText(/"running": true/)).toBeTruthy()
    fireEvent.click(screen.getByText('Hide raw result'))
    expect(screen.queryByText(/"running": true/)).toBeNull()
  })

  it('shows last checked timestamp when provided', () => {
    const ts = new Date('2024-06-01T14:30:00Z')
    render(<EnvironmentStatus tmux={TMUX_RUNNING} lastCheckedAt={ts} />)
    expect(screen.getByText(/Checked at/)).toBeTruthy()
  })
})
