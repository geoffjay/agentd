import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ContentArea } from '@/layouts/ContentArea'
import { LayoutContext } from '@/layouts/context'
import type { LayoutContextValue } from '@/layouts/context'

function makeContext(overrides: Partial<LayoutContextValue> = {}): LayoutContextValue {
  return {
    sidebarOpen: true,
    setSidebarOpen: vi.fn(),
    toggleSidebar: vi.fn(),
    ...overrides,
  }
}

function renderContentArea(children: React.ReactNode, sidebarOpen = true) {
  const ctx = makeContext({ sidebarOpen })
  return render(
    <LayoutContext.Provider value={ctx}>
      <ContentArea>{children}</ContentArea>
    </LayoutContext.Provider>,
  )
}

describe('ContentArea', () => {
  it('renders children', () => {
    renderContentArea(<p>Page content</p>)
    expect(screen.getByText('Page content')).toBeInTheDocument()
  })

  it('has role=main', () => {
    renderContentArea(<p>Content</p>)
    expect(screen.getByRole('main')).toBeInTheDocument()
  })

  it('applies lg:ml-60 when sidebar is open', () => {
    renderContentArea(<p>Content</p>, true)
    const main = screen.getByRole('main')
    expect(main.className).toContain('lg:ml-60')
  })

  it('applies lg:ml-16 when sidebar is closed', () => {
    renderContentArea(<p>Content</p>, false)
    const main = screen.getByRole('main')
    expect(main.className).toContain('lg:ml-16')
  })

  it('has id=main-content for skip-navigation links', () => {
    renderContentArea(<p>Content</p>)
    expect(document.getElementById('main-content')).toBeInTheDocument()
  })
})
