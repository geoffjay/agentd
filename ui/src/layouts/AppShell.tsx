/**
 * AppShell — root layout wrapper.
 *
 * Manages sidebar open/closed state (persisted to localStorage),
 * global search palette visibility, provides LayoutContext and
 * ThemeProvider to all children, and handles keyboard shortcuts:
 *   - Ctrl+B: toggle sidebar
 *   - Ctrl+K / Cmd+K: open search palette
 */

import { useCallback, useEffect, useState } from 'react'
import { Outlet } from 'react-router-dom'
import { LayoutContext } from './context'
import { Header } from './Header'
import { Sidebar } from './Sidebar'
import { ContentArea } from './ContentArea'
import { SearchPalette } from '@/components/search/SearchPalette'
import { ThemeProvider } from '@/hooks/useTheme'

const STORAGE_KEY = 'agentd:sidebar:open'

function readPersistedSidebarState(): boolean {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored === null) return true // default: expanded on desktop
    return stored === 'true'
  } catch {
    return true
  }
}

export function AppShell() {
  const [sidebarOpen, setSidebarOpenState] = useState<boolean>(readPersistedSidebarState)
  const [searchOpen, setSearchOpen] = useState(false)

  const setSidebarOpen = useCallback((open: boolean) => {
    setSidebarOpenState(open)
    try {
      localStorage.setItem(STORAGE_KEY, String(open))
    } catch {
      // ignore storage errors (e.g. private browsing quota)
    }
  }, [])

  const toggleSidebar = useCallback(() => {
    setSidebarOpen(!sidebarOpen)
  }, [sidebarOpen, setSidebarOpen])

  const openSearch = useCallback(() => setSearchOpen(true), [])
  const closeSearch = useCallback(() => setSearchOpen(false), [])

  // Ctrl+B to toggle sidebar; Ctrl+K / Cmd+K to open search
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key === 'b') {
        e.preventDefault()
        setSidebarOpen(!sidebarOpen)
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault()
        setSearchOpen(true)
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [sidebarOpen, setSidebarOpen])

  return (
    <ThemeProvider>
      <LayoutContext.Provider
        value={{ sidebarOpen, setSidebarOpen, toggleSidebar, searchOpen, openSearch, closeSearch }}
      >
        <div className="min-h-screen bg-gray-50 dark:bg-gray-950 transition-colors duration-150">
          <Header />
          <Sidebar />
          <ContentArea>
            <Outlet />
          </ContentArea>
          <SearchPalette isOpen={searchOpen} onClose={closeSearch} />
        </div>
      </LayoutContext.Provider>
    </ThemeProvider>
  )
}

export default AppShell
