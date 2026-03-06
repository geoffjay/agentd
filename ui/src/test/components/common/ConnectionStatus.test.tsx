/**
 * Tests for ConnectionStatus component.
 */

import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ConnectionStatus } from '@/components/common/ConnectionStatus'
import type { ConnectionState } from '@/services/websocket'

describe('ConnectionStatus', () => {
  const states: ConnectionState[] = ['Connected', 'Connecting', 'Reconnecting', 'Disconnected']

  it.each(states)('renders a status element for %s', (state) => {
    render(<ConnectionStatus connectionState={state} />)
    expect(screen.getByRole('status')).toBeInTheDocument()
  })

  it('has aria-label reflecting the connection state', () => {
    render(<ConnectionStatus connectionState="Connected" />)
    expect(screen.getByRole('status')).toHaveAttribute('aria-label', 'Stream: Connected')
  })

  it('uses custom label when provided', () => {
    render(<ConnectionStatus connectionState="Disconnected" label="Offline" />)
    expect(screen.getByRole('status')).toHaveAttribute('aria-label', 'Stream: Offline')
  })

  it('shows text label by default', () => {
    render(<ConnectionStatus connectionState="Connecting" />)
    expect(screen.getByText('Connecting')).toBeInTheDocument()
  })

  it('hides text label when iconOnly=true', () => {
    render(<ConnectionStatus connectionState="Connected" iconOnly />)
    expect(screen.queryByText('Connected')).not.toBeInTheDocument()
  })

  it.each([
    ['Connected', 'bg-green-500'],
    ['Connecting', 'animate-pulse'],
    ['Reconnecting', 'animate-pulse'],
    ['Disconnected', 'bg-red-500'],
  ] as [ConnectionState, string][])('%s state has expected dot class', (state, expectedClass) => {
    const { container } = render(<ConnectionStatus connectionState={state} />)
    const dot = container.querySelector('[aria-hidden="true"]')
    expect(dot?.className).toContain(expectedClass)
  })
})
