/**
 * Tests for HookPlaceholder coming-soon component.
 */

import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { HookPlaceholder } from '@/components/hooks/HookPlaceholder'

function renderWithRouter(ui: React.ReactNode) {
  return render(<MemoryRouter>{ui}</MemoryRouter>)
}

describe('HookPlaceholder', () => {
  it('renders the "Hooks — Coming Soon" heading', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('Hooks — Coming Soon')).toBeInTheDocument()
  })

  it('shows the service status section', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('Service Status')).toBeInTheDocument()
    expect(screen.getByText('Hook Service')).toBeInTheDocument()
  })

  it('displays the hook service port 17002', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('Port 17002')).toBeInTheDocument()
  })

  it('shows an "unknown" service status badge', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('unknown')).toBeInTheDocument()
  })

  it('renders the Planned Features section', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('Planned Features')).toBeInTheDocument()
  })

  it('renders Git Hooks feature card', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('Git Hooks')).toBeInTheDocument()
    expect(screen.getByText(/Monitor git lifecycle events/)).toBeInTheDocument()
  })

  it('renders System Hooks feature card', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('System Hooks')).toBeInTheDocument()
  })

  it('renders Event Log feature card', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('Event Log')).toBeInTheDocument()
  })

  it('renders Hook Configuration feature card', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('Hook Configuration')).toBeInTheDocument()
  })

  it('renders Notification Triggers feature card', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText('Notification Triggers')).toBeInTheDocument()
  })

  it('shows GitHub issue link', () => {
    renderWithRouter(<HookPlaceholder />)
    const link = screen.getByRole('link', { name: /GitHub issue #179/i })
    expect(link).toBeInTheDocument()
    expect(link).toHaveAttribute('href', 'https://github.com/geoffjay/agentd/issues/179')
  })

  it('includes explanation of what hooks will do', () => {
    renderWithRouter(<HookPlaceholder />)
    expect(screen.getByText(/monitor git hooks and system hooks/i)).toBeInTheDocument()
  })
})
