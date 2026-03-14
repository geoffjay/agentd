/**
 * Sidebar — collapsible left navigation with icon + label items.
 *
 * Collapse state is persisted to localStorage under 'agentd:sidebar:open'.
 * On desktop (≥1024px) it pushes the content area.
 * On tablet / mobile it overlays the content with a backdrop.
 */

import { useEffect } from 'react'
import { Link, useLocation } from 'react-router-dom'
import {
  BarChart2,
  Bell,
  Bot,
  Brain,
  CheckSquare,
  GitBranch,
  HelpCircle,
  Home,
  Webhook,
  X,
  Hexagon,
} from 'lucide-react'
import { useLayout } from './context'
import { useApprovals } from '@/hooks/useApprovals'
import { useNotificationCount } from '@/hooks/useNotificationCount'
import { ApprovalBadge } from '@/components/approvals/ApprovalBadge'
import { NotificationBadge } from '@/components/notifications/NotificationBadge'

// ---------------------------------------------------------------------------
// Nav item definition
// ---------------------------------------------------------------------------

interface NavItem {
  label: string
  path: string
  icon: React.ReactNode
  /** Whether to render an ApprovalBadge for this item */
  showApprovalBadge?: boolean
  /** Whether to render a NotificationBadge for this item */
  showNotificationBadge?: boolean
}

const NAV_ITEMS: NavItem[] = [
  { label: 'Dashboard', path: '/', icon: <Home size={20} /> },
  { label: 'Agents', path: '/agents', icon: <Bot size={20} /> },
  {
    label: 'Approvals',
    path: '/approvals',
    icon: <CheckSquare size={20} />,
    showApprovalBadge: true,
  },
  {
    label: 'Notifications',
    path: '/notifications',
    icon: <Bell size={20} />,
    showNotificationBadge: true,
  },
  { label: 'Questions', path: '/questions', icon: <HelpCircle size={20} /> },
  { label: 'Workflows', path: '/workflows', icon: <GitBranch size={20} /> },
  { label: 'Memories', path: '/memories', icon: <Brain size={20} /> },
  { label: 'Monitoring', path: '/monitoring', icon: <BarChart2 size={20} /> },
  { label: 'Hooks', path: '/hooks', icon: <Webhook size={20} /> },
]

const APP_VERSION = import.meta.env.VITE_APP_VERSION ?? '0.2.0'

// ---------------------------------------------------------------------------
// Single nav link
// ---------------------------------------------------------------------------

interface NavLinkProps {
  item: NavItem
  collapsed: boolean
  approvalCount?: number
  notificationCount?: number
  onClick?: () => void
}

function NavLink({ item, collapsed, approvalCount = 0, notificationCount = 0, onClick }: NavLinkProps) {
  const location = useLocation()
  const isActive =
    item.path === '/' ? location.pathname === '/' : location.pathname.startsWith(item.path)

  return (
    <Link
      to={item.path}
      onClick={onClick}
      aria-label={collapsed ? item.label : undefined}
      aria-current={isActive ? 'page' : undefined}
      className={[
        'flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors',
        isActive ? 'bg-primary-700 text-white' : 'text-gray-400 hover:bg-gray-700 hover:text-white',
        collapsed ? 'justify-center px-2' : '',
      ]
        .filter(Boolean)
        .join(' ')}
      title={collapsed ? item.label : undefined}
    >
      <span className="relative shrink-0">
        {item.icon}
        {item.showApprovalBadge && collapsed && approvalCount > 0 && (
          <span className="absolute -right-1 -top-1">
            <ApprovalBadge count={approvalCount} />
          </span>
        )}
        {item.showNotificationBadge && collapsed && notificationCount > 0 && (
          <span className="absolute -right-1 -top-1">
            <NotificationBadge count={notificationCount} />
          </span>
        )}
      </span>
      {!collapsed && <span className="truncate">{item.label}</span>}
      {!collapsed && item.showApprovalBadge && <ApprovalBadge count={approvalCount} />}
      {!collapsed && item.showNotificationBadge && <NotificationBadge count={notificationCount} />}
    </Link>
  )
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

export function Sidebar() {
  const { sidebarOpen, setSidebarOpen, toggleSidebar } = useLayout()
  const { totalPendingCount } = useApprovals({ refreshInterval: 30_000 })
  const { pending: notificationPending } = useNotificationCount({ refreshInterval: 15_000 })

  // Close sidebar on Escape key (mobile overlay behaviour)
  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      if (e.key === 'Escape') setSidebarOpen(false)
    }
    document.addEventListener('keydown', handleKey)
    return () => document.removeEventListener('keydown', handleKey)
  }, [setSidebarOpen])

  return (
    <>
      {/* Mobile backdrop — only visible when open on small screens */}
      {sidebarOpen && (
        <div
          aria-hidden="true"
          className="fixed inset-0 z-30 bg-black/50 lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Sidebar panel */}
      <aside
        aria-label="Sidebar navigation"
        className={[
          'fixed inset-y-0 left-0 z-40 flex flex-col border-r border-gray-700 bg-gray-900 transition-all duration-300 ease-in-out',
          // Width: collapsed = 64px, expanded = 240px
          sidebarOpen ? 'w-60' : 'w-16',
          // On mobile: slide in/out from left
          sidebarOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0',
        ]
          .filter(Boolean)
          .join(' ')}
      >
        {/* Logo / branding */}
        <div className="flex h-16 shrink-0 items-center gap-2 px-4">
          <Link to="/" className="flex items-center gap-2 text-lg m-auto font-semibold text-white hover:text-primary-400" aria-label="agentd home">
            <Hexagon size={20} className="shrink-0 text-primary-400" />
            {sidebarOpen && <span>agentd</span>}
          </Link>
          {/* Mobile close button */}
          <div className="ml-auto lg:hidden">
            <button
              type="button"
              aria-label="Close sidebar"
              onClick={() => setSidebarOpen(false)}
              className="rounded-md p-1 text-gray-400 hover:bg-gray-700 hover:text-white"
            >
              <X size={18} />
            </button>
          </div>
        </div>

        {/* Nav items */}
        <nav className="flex-1 overflow-y-auto px-2 py-2">
          <ul role="list" className="space-y-1">
            {NAV_ITEMS.map((item) => (
              <li key={item.path}>
                <NavLink
                  item={item}
                  collapsed={!sidebarOpen}
                  approvalCount={totalPendingCount}
                  notificationCount={notificationPending}
                  // On mobile/tablet, close sidebar after navigating
                  onClick={() => {
                    if (window.innerWidth < 1024) setSidebarOpen(false)
                  }}
                />
              </li>
            ))}
          </ul>
        </nav>

        {/* Bottom section */}
        <div className="border-t border-gray-700 px-3 py-3">
          {sidebarOpen ? (
            <div className="space-y-1 text-xs text-gray-500">
              <p>v{APP_VERSION}</p>
              <div className="flex gap-2">
                <a
                  href="https://github.com/geoffjay/agentd"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="hover:text-gray-300"
                >
                  Docs
                </a>
                <span>·</span>
                <a
                  href="/api/orchestrator/health"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="hover:text-gray-300"
                >
                  Health
                </a>
              </div>
            </div>
          ) : (
            /* Collapsed: just show a toggle hint */
            <button
              type="button"
              aria-label="Expand sidebar"
              onClick={toggleSidebar}
              className="flex w-full justify-center rounded-md p-1 text-gray-500 hover:bg-gray-700 hover:text-white"
            >
              <span className="text-xs">›</span>
            </button>
          )}
        </div>
      </aside>
    </>
  )
}

export default Sidebar
