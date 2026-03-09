/**
 * Tests for CheckControls component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { CheckControls } from '@/components/questions/CheckControls'
import { makeTriggerResponse } from '@/test/mocks/factories'
import type { AutoTriggerInterval } from '@/hooks/useAskService'

const DEFAULT_PROPS = {
  triggering: false,
  triggerError: undefined,
  autoTrigger: false,
  autoTriggerInterval: 60_000 as AutoTriggerInterval,
  onRunTrigger: vi.fn(),
  onSetAutoTrigger: vi.fn(),
  onSetAutoTriggerInterval: vi.fn(),
}

describe('CheckControls', () => {
  it('renders the "Run Checks" button', () => {
    render(<CheckControls {...DEFAULT_PROPS} />)
    expect(screen.getByText('Run Checks')).toBeTruthy()
  })

  it('shows "Running…" label while triggering', () => {
    render(<CheckControls {...DEFAULT_PROPS} triggering />)
    expect(screen.getByText('Running…')).toBeTruthy()
  })

  it('disables the button while triggering', () => {
    render(<CheckControls {...DEFAULT_PROPS} triggering />)
    const btn = screen.getByLabelText('Run environment checks')
    expect((btn as HTMLButtonElement).disabled).toBe(true)
  })

  it('calls onRunTrigger when button clicked', () => {
    const onRunTrigger = vi.fn()
    render(<CheckControls {...DEFAULT_PROPS} onRunTrigger={onRunTrigger} />)
    fireEvent.click(screen.getByText('Run Checks'))
    expect(onRunTrigger).toHaveBeenCalledOnce()
  })

  it('shows error message when triggerError is set', () => {
    render(<CheckControls {...DEFAULT_PROPS} triggerError="Connection refused" />)
    expect(screen.getByText('Connection refused')).toBeTruthy()
  })

  it('shows trigger results after successful check', () => {
    const result = makeTriggerResponse({ checks_run: ['TmuxSessions'] })
    render(<CheckControls {...DEFAULT_PROPS} lastTriggerResult={result} lastTriggerAt={new Date()} />)
    expect(screen.getByText('TmuxSessions')).toBeTruthy()
    expect(screen.getByText('Checks run')).toBeTruthy()
  })

  it('shows "None" when no notifications were sent', () => {
    const result = makeTriggerResponse({ notifications_sent: [] })
    render(<CheckControls {...DEFAULT_PROPS} lastTriggerResult={result} lastTriggerAt={new Date()} />)
    expect(screen.getByText('None')).toBeTruthy()
  })

  it('shows notification IDs when sent', () => {
    const result = makeTriggerResponse({ notifications_sent: ['notif-abc'] })
    render(<CheckControls {...DEFAULT_PROPS} lastTriggerResult={result} lastTriggerAt={new Date()} />)
    expect(screen.getByText('notif-abc')).toBeTruthy()
  })

  it('shows the auto-trigger toggle', () => {
    render(<CheckControls {...DEFAULT_PROPS} />)
    expect(screen.getByText('Auto-trigger')).toBeTruthy()
  })

  it('calls onSetAutoTrigger when toggle clicked', () => {
    const onSetAutoTrigger = vi.fn()
    render(<CheckControls {...DEFAULT_PROPS} onSetAutoTrigger={onSetAutoTrigger} />)
    const toggle = screen.getByRole('switch', { name: /auto-trigger/i })
    fireEvent.click(toggle)
    expect(onSetAutoTrigger).toHaveBeenCalledWith(true)
  })

  it('shows interval picker when auto-trigger is on', () => {
    render(<CheckControls {...DEFAULT_PROPS} autoTrigger />)
    expect(screen.getByRole('group', { name: 'Auto-trigger interval' })).toBeTruthy()
  })

  it('does not show interval picker when auto-trigger is off', () => {
    render(<CheckControls {...DEFAULT_PROPS} autoTrigger={false} />)
    expect(screen.queryByRole('group', { name: 'Auto-trigger interval' })).toBeNull()
  })

  it('shows tmux result when available', () => {
    const result = makeTriggerResponse({
      results: {
        tmux_sessions: { running: true, session_count: 2, sessions: ['main', 'dev'] },
      },
    })
    render(<CheckControls {...DEFAULT_PROPS} lastTriggerResult={result} lastTriggerAt={new Date()} />)
    expect(screen.getByText('tmux_sessions result')).toBeTruthy()
    expect(screen.getByText(/Running — 2 sessions/)).toBeTruthy()
  })
})
