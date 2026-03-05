/**
 * Header — fixed top bar with sidebar toggle, logo, search, notifications, and settings.
 */

import { useEffect, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { Bell, Menu, Search, Settings, X } from 'lucide-react'
import { useLayout } from './context'

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
// Search bar
// ---------------------------------------------------------------------------

interface SearchBarProps {
  /** Whether the search bar is in collapsed (icon-only) mode */
  collapsed?: boolean
}

function SearchBar({ collapsed = false }: SearchBarProps) {
  const inputRef = useRef<HTMLInputElement>(null)
  const [expanded, setExpanded] = useState(false)

  // Ctrl+K shortcut
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault()
        inputRef.current?.focus()
        setExpanded(true)
      }
      if (e.key === 'Escape') {
        inputRef.current?.blur()
        setExpanded(false)
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [])

  if (collapsed && !expanded) {
    return (
      <button
        type="button"
        aria-label="Open search"
        onClick={() => setExpanded(true)}
        className="rounded-md p-2 text-gray-400 transition-colors hover:bg-gray-700 hover:text-white"
      >
        <Search size={20} />
      </button>
    )
  }

  return (
    <div className="relative flex items-center">
      <Search size={16} className="absolute left-3 text-gray-400" aria-hidden="true" />
      <input
        ref={inputRef}
        type="search"
        placeholder="Search… (Ctrl+K)"
        aria-label="Global search"
        className="w-48 rounded-md border border-gray-700 bg-gray-800 py-1.5 pl-9 pr-3 text-sm text-gray-200 placeholder-gray-500 transition-all focus:w-72 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 md:w-64"
        onBlur={() => setExpanded(false)}
        autoFocus={expanded}
      />
      {expanded && (
        <button
          type="button"
          aria-label="Close search"
          onClick={() => setExpanded(false)}
          className="absolute right-2 text-gray-400 hover:text-white"
        >
          <X size={14} />
        </button>
      )}
    </div>
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

  return (
    <header className="fixed inset-x-0 top-0 z-50 flex h-16 items-center gap-3 border-b border-gray-700 bg-gray-900 px-4">
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

      {/* Search — collapses to icon on small screens */}
      <div className="hidden sm:block">
        <SearchBar />
      </div>
      <div className="sm:hidden">
        <SearchBar collapsed />
      </div>

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
