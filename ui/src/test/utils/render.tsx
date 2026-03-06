/**
 * Custom render utility wrapping @testing-library/react.
 *
 * Wraps the component under test with all required providers:
 *   - React Router (MemoryRouter) for navigation
 *   - LayoutContext for sidebar / search state
 *
 * Usage:
 *   import { renderWithProviders } from '@/test/utils/render'
 *
 *   renderWithProviders(<MyComponent />)
 *   renderWithProviders(<MyComponent />, { initialPath: '/agents' })
 *   renderWithProviders(<MyComponent />, { contextOverrides: { sidebarOpen: false } })
 */

import { vi } from 'vitest'
import { render } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { LayoutContext } from '@/layouts/context'
import type { LayoutContextValue } from '@/layouts/context'
import type { RenderOptions } from '@testing-library/react'

// ---------------------------------------------------------------------------
// Default context value
// ---------------------------------------------------------------------------

export function makeLayoutContext(overrides?: Partial<LayoutContextValue>): LayoutContextValue {
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

// ---------------------------------------------------------------------------
// renderWithProviders
// ---------------------------------------------------------------------------

export interface RenderWithProvidersOptions extends Omit<RenderOptions, 'wrapper'> {
  /** Initial URL path for the router (default: '/') */
  initialPath?: string
  /** Override layout context values */
  contextOverrides?: Partial<LayoutContextValue>
}

export function renderWithProviders(
  ui: React.ReactElement,
  options: RenderWithProvidersOptions = {},
) {
  const { initialPath = '/', contextOverrides, ...renderOptions } = options
  const ctx = makeLayoutContext(contextOverrides)

  function Wrapper({ children }: { children: React.ReactNode }) {
    return (
      <MemoryRouter initialEntries={[initialPath]}>
        <LayoutContext.Provider value={ctx}>
          <Routes>
            <Route path="*" element={children} />
          </Routes>
        </LayoutContext.Provider>
      </MemoryRouter>
    )
  }

  return {
    ctx,
    ...render(ui, { wrapper: Wrapper, ...renderOptions }),
  }
}

// Re-export everything from @testing-library/react so tests can import
// from a single location.
// eslint-disable-next-line react-refresh/only-export-components
export * from '@testing-library/react'
export { renderWithProviders as render }
