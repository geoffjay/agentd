/**
 * Header — fixed top bar with sidebar toggle, logo, search, theme toggle,
 * connection status, notifications, and settings.
 *
 * The search button opens the global SearchPalette (managed by AppShell).
 * Ctrl+K / Cmd+K is handled at the AppShell level.
 */

import { Link } from 'react-router-dom'
import { Bell, Menu, Search, Settings } from 'lucide-react'
import { useLayout } from './context'
import { ThemeToggle } from '@/components/common/ThemeToggle'
import { ConnectionStatus } from '@/components/common/ConnectionStatus'
import { useAllAgentsStream } from '@/hooks/useAllAgentsStream'

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
      className="flex items-center gap-2 rounded-md border border-gray-700 bg-gray-800 py-1.5 pl-3 pr-4 text-sm text-gray-400 transition-colors hover:border-gray-600 hover:text-gray-300"
    >
      <Search size={14} aria-hidden="true" />
      <span className="hidden md:inline">Search…</span>
      <kbd className="hidden rounded border border-gray-600 px-1 py-0.5 text-[10px] text-gray-500 md:inline">
        Ctrl+K
      </kbd>
    </button>
  )
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

export interface HeaderProps {
  /** Number of unread notifications to show in the badge */
  unreadCount?: number
}

export function Header({ unreadCount = 0 }: HeaderProps) {
  const { toggleSidebar } = useLayout()
  const { connectionState } = useAllAgentsStream()

  return (
    <header className="fixed inset-x-0 top-0 z-50 flex h-16 items-center gap-3 border-b border-gray-700 bg-gray-900 px-4 transition-colors duration-150">
      {/* Sidebar toggle */}
      <button
        type="button"
        aria-label="Toggle sidebar"
        onClick={toggleSidebar}
        className="rounded-md p-2 text-gray-400 transition-colors hover:bg-gray-700 hover:text-white"
      >
        <Menu size={20} />
      </button>

      {/* Logo / title */}
      <Link
        to="/"
        className="flex items-center gap-2 text-lg font-semibold text-white hover:text-primary-400"
        aria-label="agentd home"
      >
        <span className="text-primary-400">⬡</span>
        <span>agentd</span>
      </Link>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Search trigger */}
      <SearchTrigger />

      {/* Global stream connection status (icon only on small screens) */}
      <ConnectionStatus
        connectionState={connectionState}
        iconOnly
        className="hidden sm:flex"
      />

      {/* Theme toggle */}
      <ThemeToggle />

      {/* Notification bell */}
      <Link
        to="/notifications"
        aria-label={
          unreadCount > 0 ? `Notifications — ${unreadCount} unread` : 'Notifications'
        }
        className="relative rounded-md p-2 text-gray-400 transition-colors hover:bg-gray-700 hover:text-white"
      >
        <Bell size={20} />
        <NotificationBadge count={unreadCount} />
      </Link>

      {/* Settings */}
      <Link
        to="/settings"
        aria-label="Settings"
        className="rounded-md p-2 text-gray-400 transition-colors hover:bg-gray-700 hover:text-white"
      >
        <Settings size={20} />
      </Link>
    </header>
  )
}

export default Header
