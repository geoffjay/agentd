import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { Header } from '@/layouts/Header'
import { LayoutContext } from '@/layouts/context'
import type { LayoutContextValue } from '@/layouts/context'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeContext(overrides: Partial<LayoutContextValue> = {}): LayoutContextValue {
  return {
    sidebarOpen: true,
    setSidebarOpen: vi.fn(),
    toggleSidebar: vi.fn(),
    ...overrides,
  }
}

function renderHeader(props = {}, contextOverrides: Partial<LayoutContextValue> = {}) {
  const ctx = makeContext(contextOverrides)
  return {
    ctx,
    ...render(
      <MemoryRouter>
        <LayoutContext.Provider value={ctx}>
          <Header {...props} />
        </LayoutContext.Provider>
      </MemoryRouter>,
    ),
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Header', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders a header element', () => {
    renderHeader()
    expect(screen.getByRole('banner')).toBeInTheDocument()
  })

  it('renders the agentd logo/title', () => {
    renderHeader()
    expect(screen.getByText('agentd')).toBeInTheDocument()
  })

  it('calls toggleSidebar when hamburger button is clicked', () => {
    const { ctx } = renderHeader()
    const btn = screen.getByRole('button', { name: /toggle sidebar/i })
    fireEvent.click(btn)
    expect(ctx.toggleSidebar).toHaveBeenCalledOnce()
  })

  it('renders notification link', () => {
    renderHeader()
    expect(screen.getByRole('link', { name: /notifications/i })).toBeInTheDocument()
  })

  it('renders settings link', () => {
    renderHeader()
    expect(screen.getByRole('link', { name: /settings/i })).toBeInTheDocument()
  })

  it('shows notification badge when unreadCount > 0', () => {
    renderHeader({ unreadCount: 5 })
    expect(screen.getByLabelText('5 unread notifications')).toBeInTheDocument()
  })

  it('does not show notification badge when unreadCount is 0', () => {
    renderHeader({ unreadCount: 0 })
    expect(screen.queryByLabelText(/unread notifications/i)).not.toBeInTheDocument()
  })

  it('shows 99+ for large unread counts', () => {
    renderHeader({ unreadCount: 150 })
    expect(screen.getByText('99+')).toBeInTheDocument()
  })

  it('search input has an accessible label', () => {
    renderHeader()
    expect(screen.getByRole('searchbox', { name: /global search/i })).toBeInTheDocument()
  })
})
