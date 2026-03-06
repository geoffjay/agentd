/**
 * Accessibility tests using jest-axe.
 *
 * Runs axe-core against the rendered HTML of key components to detect
 * structural accessibility violations automatically.
 */

import { describe, it, expect } from 'vitest'
import { render } from '@testing-library/react'
import { axe } from '@/test/setup'
import { MemoryRouter } from 'react-router-dom'
import { LayoutContext } from '@/layouts/context'
import { ThemeProvider } from '@/hooks/useTheme'
import type { LayoutContextValue } from '@/layouts/context'
import { vi } from 'vitest'
import { StatusBadge } from '@/components/common/StatusBadge'
import {
  Skeleton,
  CardSkeleton,
} from '@/components/common/LoadingSkeleton'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeCtx(): LayoutContextValue {
  return {
    sidebarOpen: true,
    setSidebarOpen: vi.fn(),
    toggleSidebar: vi.fn(),
    searchOpen: false,
    openSearch: vi.fn(),
    closeSearch: vi.fn(),
  }
}

function withRouter(element: React.ReactElement) {
  return (
    <ThemeProvider>
      <MemoryRouter>
        <LayoutContext.Provider value={makeCtx()}>{element}</LayoutContext.Provider>
      </MemoryRouter>
    </ThemeProvider>
  )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Accessibility (axe-core)', () => {
  describe('StatusBadge', () => {
    it('has no accessibility violations (Running badge)', async () => {
      const { container } = render(<StatusBadge status="Running" />)
      const results = await axe(container)
      expect(results).toHaveNoViolations()
    })

    it('has no accessibility violations (dot variant)', async () => {
      const { container } = render(<StatusBadge status="Failed" variant="dot" />)
      const results = await axe(container)
      expect(results).toHaveNoViolations()
    })

    it('has no accessibility violations (healthy service)', async () => {
      const { container } = render(<StatusBadge status="healthy" />)
      const results = await axe(container)
      expect(results).toHaveNoViolations()
    })
  })

  describe('LoadingSkeleton', () => {
    it('Skeleton has no accessibility violations', async () => {
      const { container } = render(<Skeleton />)
      const results = await axe(container)
      expect(results).toHaveNoViolations()
    })

    it('CardSkeleton has no accessibility violations', async () => {
      const { container } = render(<CardSkeleton />)
      const results = await axe(container)
      expect(results).toHaveNoViolations()
    })
  })

  describe('Header', () => {
    it('has no accessibility violations', async () => {
      // Lazy import to avoid issues with missing context at module level
      const { Header } = await import('@/layouts/Header')
      const { container } = render(withRouter(<Header />))
      const results = await axe(container)
      expect(results).toHaveNoViolations()
    })
  })

  describe('Sidebar', () => {
    it('has no accessibility violations when open', async () => {
      const { Sidebar } = await import('@/layouts/Sidebar')
      const { container } = render(withRouter(<Sidebar />))
      const results = await axe(container)
      expect(results).toHaveNoViolations()
    })
  })
})
