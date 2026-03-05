/**
 * SearchResults — grouped result sections displayed inside the search palette.
 *
 * Renders three sections: Quick Actions, Agents, Notifications.
 * Each section shows a heading, up to 5 items, and a "View all" link.
 */

import { Search } from 'lucide-react'
import { SearchResultItem } from './SearchResultItem'
import type { GroupedSearchResults, SearchResult } from '@/hooks/useSearch'

// ---------------------------------------------------------------------------
// Section
// ---------------------------------------------------------------------------

interface ResultSectionProps {
  heading: string
  items: SearchResult[]
  activeId: string | null
  onSelect: (result: SearchResult) => void
  viewAllHref?: string
  viewAllLabel?: string
}

function ResultSection({
  heading,
  items,
  activeId,
  onSelect,
  viewAllHref,
  viewAllLabel,
}: ResultSectionProps) {
  if (items.length === 0) return null

  return (
    <div>
      <div className="px-4 py-1.5">
        <span className="text-[11px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
          {heading}
        </span>
      </div>
      <ul role="group" aria-label={heading}>
        {items.map((result) => (
          <SearchResultItem
            key={result.id}
            result={result}
            isActive={result.id === activeId}
            onClick={onSelect}
          />
        ))}
      </ul>
      {viewAllHref && (
        <div className="px-4 py-1">
          <a
            href={viewAllHref}
            className="text-xs text-primary-600 hover:text-primary-500 dark:text-primary-400"
          >
            {viewAllLabel ?? `View all results →`}
          </a>
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Empty / no-results state
// ---------------------------------------------------------------------------

function NoResults({ query }: { query: string }) {
  return (
    <div className="flex flex-col items-center gap-3 px-6 py-10 text-center">
      <Search size={28} className="text-gray-300 dark:text-gray-600" />
      <p className="text-sm font-medium text-gray-700 dark:text-gray-300">
        No results for <span className="font-semibold">"{query}"</span>
      </p>
      <p className="text-xs text-gray-500 dark:text-gray-400">
        Try searching for an agent name, notification title, or a page name.
      </p>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Recent searches (shown when query is empty)
// ---------------------------------------------------------------------------

interface RecentSearchesProps {
  searches: string[]
  onSelect: (q: string) => void
  onClear: () => void
}

export function RecentSearches({ searches, onSelect, onClear }: RecentSearchesProps) {
  if (searches.length === 0) {
    return (
      <div className="px-6 py-8 text-center text-sm text-gray-500 dark:text-gray-400">
        Start typing to search agents, notifications, and more.
      </div>
    )
  }

  return (
    <div>
      <div className="flex items-center justify-between px-4 py-1.5">
        <span className="text-[11px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
          Recent Searches
        </span>
        <button
          type="button"
          onClick={onClear}
          className="text-[11px] text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300"
        >
          Clear
        </button>
      </div>
      <ul role="list" aria-label="Recent searches">
        {searches.map((q) => (
          <li key={q}>
            <button
              type="button"
              onClick={() => onSelect(q)}
              className="flex w-full items-center gap-3 px-4 py-2 text-left text-sm text-gray-700 hover:bg-gray-50 dark:text-gray-300 dark:hover:bg-gray-800"
            >
              <Search size={14} className="shrink-0 text-gray-400" aria-hidden="true" />
              {q}
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}

// ---------------------------------------------------------------------------
// SearchResults
// ---------------------------------------------------------------------------

export interface SearchResultsProps {
  query: string
  results: GroupedSearchResults
  loading: boolean
  activeId: string | null
  onSelect: (result: SearchResult) => void
}

export function SearchResults({ query, results, loading, activeId, onSelect }: SearchResultsProps) {
  if (loading) {
    return (
      <div className="flex items-center justify-center py-10">
        <span
          className="h-5 w-5 animate-spin rounded-full border-2 border-primary-500 border-t-transparent"
          role="status"
          aria-label="Searching…"
        />
      </div>
    )
  }

  const hasResults =
    results.actions.length > 0 || results.agents.length > 0 || results.notifications.length > 0

  if (!hasResults) {
    return <NoResults query={query} />
  }

  return (
    <div
      role="listbox"
      aria-label="Search results"
      className="divide-y divide-gray-100 dark:divide-gray-800"
    >
      <ResultSection
        heading="Quick Actions"
        items={results.actions}
        activeId={activeId}
        onSelect={onSelect}
      />
      <ResultSection
        heading={`Agents${results.agents.length === 5 ? ' (showing top 5)' : ''}`}
        items={results.agents}
        activeId={activeId}
        onSelect={onSelect}
        viewAllHref={results.agents.length === 5 ? `/agents?search=${encodeURIComponent(query)}` : undefined}
        viewAllLabel={`View all agent results for "${query}" →`}
      />
      <ResultSection
        heading={`Notifications${results.notifications.length === 5 ? ' (showing top 5)' : ''}`}
        items={results.notifications}
        activeId={activeId}
        onSelect={onSelect}
        viewAllHref={
          results.notifications.length === 5
            ? `/notifications?search=${encodeURIComponent(query)}`
            : undefined
        }
        viewAllLabel={`View all notification results for "${query}" →`}
      />
    </div>
  )
}

export default SearchResults
