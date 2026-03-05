/**
 * Layout context — shared state between AppShell, Header, and Sidebar.
 */

import { createContext, useContext } from 'react'

export interface LayoutContextValue {
  /** Whether the sidebar is currently open/expanded */
  sidebarOpen: boolean
  /** Toggle the sidebar between open and closed */
  toggleSidebar: () => void
  /** Explicitly set sidebar open/closed */
  setSidebarOpen: (open: boolean) => void
  /** Whether the global search palette is open */
  searchOpen: boolean
  /** Open the global search palette */
  openSearch: () => void
  /** Close the global search palette */
  closeSearch: () => void
}

export const LayoutContext = createContext<LayoutContextValue | null>(null)

/** Hook to consume the layout context — throws if used outside AppShell */
export function useLayout(): LayoutContextValue {
  const ctx = useContext(LayoutContext)
  if (!ctx) {
    throw new Error('useLayout must be used within an AppShell')
  }
  return ctx
}
