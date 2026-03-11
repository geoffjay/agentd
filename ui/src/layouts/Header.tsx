/**
 * Header — fixed top bar with sidebar toggle, search, theme toggle,
 * connection status, notifications, and settings.
 *
 * Positioned to the right of the sidebar. Logo/branding lives in the Sidebar.
 * The search button opens the global SearchPalette (managed by AppShell).
 * Ctrl+K / Cmd+K is handled at the AppShell level.
 */

import { Link } from 'react-router-dom'
import { Bell, Menu, Search, Settings } from 'lucide-react'
import { useLayout } from './context'
import { ThemeToggle } from '@/components/common/ThemeToggle'
import { ConnectionStatus } from '@/components/common/ConnectionStatus'
import { useAllAgentsStream } from '@/hooks/useAllAgentsStream'
import { useNotificationCount } from '@/hooks/useNotificationCount'

// ---------------------------------------------------------------------------
// Notification badge
// ---------------------------------------------------------------------------

interface NotificationBadgeProps {
  count: number
}

function NotificationBadge({ count }: NotificationBadgeProps) {
  if (count === 0) return null
  return (
    <span
      aria-label={`${count} unread notifications`}
      className="absolute -right-1 -top-1 flex h-4 w-4 items-center justify-center rounded-full bg-red-500 text-[10px] font-bold text-white"
    >
      {count > 99 ? '99+' : count}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Search trigger button
// ---------------------------------------------------------------------------

function SearchTrigger() {
  const { openSearch } = useLayout()

  return (
    <button
      type="button"
      aria-label="Global search"
      aria-keyshortcuts="Control+k Meta+k"
      onClick={openSearch}
      className="flex items-center gap-2 rounded-md border border-gray-300 bg-white py-1.5 pl-3 pr-4 text-sm text-gray-500 transition-colors hover:border-gray-400 hover:text-gray-700 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-400 dark:hover:border-gray-600 dark:hover:text-gray-300"
    >
      <Search size={14} aria-hidden="true" />
      <span className="hidden md:inline">Search…</span>
      <kbd className="hidden rounded border border-gray-300 px-1 py-0.5 text-[10px] text-gray-400 dark:border-gray-600 dark:text-gray-500 md:inline">
        Ctrl+K
      </kbd>
    </button>
  )
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

export interface HeaderProps {
  /** Number of unread notifications to show in the badge; if omitted, fetched automatically */
  unreadCount?: number
}

export function Header({ unreadCount }: HeaderProps) {
  const { sidebarOpen, toggleSidebar } = useLayout()
  const { connectionState } = useAllAgentsStream()
  const { pending } = useNotificationCount({ refreshInterval: 15_000 })
  const displayCount = unreadCount ?? pending

  return (
    <header
      className={[
        'fixed top-0 right-0 z-30 flex h-16 items-center gap-3 bg-gray-50 px-4 transition-all duration-300 ease-in-out dark:border-gray-800 dark:bg-gray-950',
        // Offset left edge by sidebar width
        sidebarOpen ? 'lg:left-60' : 'lg:left-16',
        'left-0',
      ].join(' ')}
    >
      {/* Sidebar toggle */}
      <button
        type="button"
        aria-label="Toggle sidebar"
        onClick={toggleSidebar}
        className="rounded-md p-2 text-gray-500 transition-colors hover:bg-gray-200 hover:text-gray-900 dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-white"
      >
        <Menu size={20} />
      </button>

      {/* Search trigger */}
      <SearchTrigger />

      {/* Spacer */}
      <div className="flex-1" />

      {/* Global stream connection status (icon only on small screens) */}
      <ConnectionStatus connectionState={connectionState} iconOnly className="hidden sm:flex" />

      {/* Theme toggle */}
      <ThemeToggle />

      {/* Notification bell */}
      <Link
        to="/notifications"
        aria-label={displayCount > 0 ? `Notifications — ${displayCount} unread` : 'Notifications'}
        className="relative rounded-md p-2 text-gray-500 transition-colors hover:bg-gray-200 hover:text-gray-900 dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-white"
      >
        <Bell size={20} />
        <NotificationBadge count={displayCount} />
      </Link>

      {/* Settings */}
      <Link
        to="/settings"
        aria-label="Settings"
        className="rounded-md p-2 text-gray-500 transition-colors hover:bg-gray-200 hover:text-gray-900 dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-white"
      >
        <Settings size={20} />
      </Link>
    </header>
  )
}

export default Header
