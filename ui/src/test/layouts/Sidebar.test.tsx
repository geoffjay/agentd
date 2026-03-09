import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { Sidebar } from '@/layouts/Sidebar'
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
    searchOpen: false,
    openSearch: vi.fn(),
    closeSearch: vi.fn(),
    ...overrides,
  }
}

function renderSidebar(contextOverrides: Partial<LayoutContextValue> = {}) {
  const ctx = makeContext(contextOverrides)
  return {
    ctx,
    ...render(
      <MemoryRouter initialEntries={['/']}>
        <LayoutContext.Provider value={ctx}>
          <Sidebar />
        </LayoutContext.Provider>
      </MemoryRouter>,
    ),
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Sidebar', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.stubGlobal('innerWidth', 1280)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('renders a complementary navigation landmark', () => {
    renderSidebar()
    expect(screen.getByRole('complementary')).toBeInTheDocument()
    expect(screen.getByRole('navigation')).toBeInTheDocument()
  })

  it('renders all navigation items when open', () => {
    renderSidebar({ sidebarOpen: true })
    expect(screen.getByRole('link', { name: /dashboard/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /agents/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /notifications/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /questions/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /workflows/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /monitoring/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /hooks/i })).toBeInTheDocument()
  })

  it('has w-60 class when open', () => {
    renderSidebar({ sidebarOpen: true })
    const aside = screen.getByRole('complementary')
    expect(aside.className).toContain('w-60')
  })

  it('has w-16 class when collapsed', () => {
    renderSidebar({ sidebarOpen: false })
    const aside = screen.getByRole('complementary')
    expect(aside.className).toContain('w-16')
  })

  it('marks active route with aria-current=page', () => {
    renderSidebar({ sidebarOpen: true })
    const dashLink = screen.getByRole('link', { name: /dashboard/i })
    expect(dashLink).toHaveAttribute('aria-current', 'page')
  })

  it('does not mark inactive routes with aria-current', () => {
    renderSidebar({ sidebarOpen: true })
    const agentsLink = screen.getByRole('link', { name: /agents/i })
    expect(agentsLink).not.toHaveAttribute('aria-current', 'page')
  })

  it('closes on Escape key press', () => {
    const { ctx } = renderSidebar({ sidebarOpen: true })
    act(() => {
      fireEvent.keyDown(document, { key: 'Escape' })
    })
    expect(ctx.setSidebarOpen).toHaveBeenCalledWith(false)
  })

  it('renders mobile close button', () => {
    renderSidebar({ sidebarOpen: true })
    expect(screen.getByRole('button', { name: /close sidebar/i })).toBeInTheDocument()
  })

  it('calls setSidebarOpen(false) when close button clicked', () => {
    const { ctx } = renderSidebar({ sidebarOpen: true })
    const closeBtn = screen.getByRole('button', { name: /close sidebar/i })
    fireEvent.click(closeBtn)
    expect(ctx.setSidebarOpen).toHaveBeenCalledWith(false)
  })

  it('shows backdrop when open on mobile', () => {
    vi.stubGlobal('innerWidth', 375) // mobile width
    renderSidebar({ sidebarOpen: true })
    // Backdrop is a div with aria-hidden="true"
    const backdrops = document.querySelectorAll('[aria-hidden="true"]')
    expect(backdrops.length).toBeGreaterThan(0)
  })

  it('hides labels when collapsed (nav links use title attribute)', () => {
    renderSidebar({ sidebarOpen: false })
    // All nav links should have title attribute for tooltip when collapsed
    const navLinks = screen.getAllByRole('link').filter((link) => link.getAttribute('title'))
    expect(navLinks.length).toBeGreaterThan(0)
  })
})
