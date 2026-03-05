/**
 * SearchResultItem — a single row in the search palette results list.
 *
 * Shows: icon, title, subtitle, and a category badge.
 * Supports keyboard focus highlighting.
 */

import { Bot, Bell, Zap, ChevronRight } from 'lucide-react'
import type { SearchResult } from '@/hooks/useSearch'

// ---------------------------------------------------------------------------
// Category icon + badge colour
// ---------------------------------------------------------------------------

const CATEGORY_META: Record<
  SearchResult['category'],
  { label: string; iconEl: React.ReactNode; badgeClass: string }
> = {
  agent: {
    label: 'Agent',
    iconEl: <Bot size={16} className="text-primary-400" />,
    badgeClass: 'bg-primary-100 text-primary-700 dark:bg-primary-900/40 dark:text-primary-300',
  },
  notification: {
    label: 'Notification',
    iconEl: <Bell size={16} className="text-yellow-400" />,
    badgeClass: 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/40 dark:text-yellow-300',
  },
  action: {
    label: 'Action',
    iconEl: <Zap size={16} className="text-green-400" />,
    badgeClass: 'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300',
  },
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface SearchResultItemProps {
  result: SearchResult
  isActive: boolean
  onClick: (result: SearchResult) => void
}

export function SearchResultItem({ result, isActive, onClick }: SearchResultItemProps) {
  const meta = CATEGORY_META[result.category]

  function handleClick() {
    onClick(result)
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      onClick(result)
    }
  }

  return (
    <li role="option" aria-selected={isActive}>
      <div
        role="button"
        tabIndex={-1}
        onClick={handleClick}
        onKeyDown={handleKeyDown}
        data-active={isActive}
        className={[
          'flex cursor-pointer items-center gap-3 px-4 py-2.5 transition-colors',
          isActive
            ? 'bg-primary-50 dark:bg-primary-900/20'
            : 'hover:bg-gray-50 dark:hover:bg-gray-800',
        ].join(' ')}
      >
        {/* Category icon */}
        <span
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-gray-100 dark:bg-gray-700"
          aria-hidden="true"
        >
          {meta.iconEl}
        </span>

        {/* Text */}
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium text-gray-900 dark:text-white">
            {result.title}
          </span>
          <span className="block truncate text-xs text-gray-500 dark:text-gray-400">
            {result.subtitle}
          </span>
        </span>

        {/* Category badge */}
        <span
          className={[
            'shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium',
            meta.badgeClass,
          ].join(' ')}
        >
          {meta.label}
        </span>

        {/* Arrow hint */}
        <ChevronRight
          size={14}
          className="shrink-0 text-gray-300 dark:text-gray-600"
          aria-hidden="true"
        />
      </div>
    </li>
  )
}

export default SearchResultItem
