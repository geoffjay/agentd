/**
 * AppShell — root layout wrapper.
 *
 * Manages sidebar open/closed state (persisted to localStorage),
 * provides LayoutContext to all children, and handles Ctrl+B
 * keyboard shortcut to toggle the sidebar.
 */

import { useCallback, useEffect, useState } from 'react'
import { Outlet } from 'react-router-dom'
import { LayoutContext } from './context'
import { Header } from './Header'
import { Sidebar } from './Sidebar'
import { ContentArea } from './ContentArea'

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

  // Ctrl+B to toggle sidebar
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key === 'b') {
        e.preventDefault()
        setSidebarOpen(!sidebarOpen)
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [sidebarOpen, setSidebarOpen])

  return (
    <LayoutContext.Provider value={{ sidebarOpen, setSidebarOpen, toggleSidebar }}>
      <div className="min-h-screen bg-gray-50 dark:bg-gray-950">
        <Header />
        <Sidebar />
        <ContentArea>
          <Outlet />
        </ContentArea>
      </div>
    </LayoutContext.Provider>
  )
}

export default AppShell
