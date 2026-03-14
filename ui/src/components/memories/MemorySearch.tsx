/**
 * MemorySearch — semantic similarity search panel for memories.
 *
 * Features:
 * - Search input with submit button (Enter key or click)
 * - Expandable advanced filters (type, tag, date range, limit)
 * - Results displayed as MemoryCard components
 * - Loading spinner during search
 * - Empty state when no results found
 * - Clear/reset button to return to default list view
 */

import { useState } from 'react'
import { ChevronDown, ChevronRight, Loader2, Search, X } from 'lucide-react'
import { useMemorySearch } from '@/hooks/useMemorySearch'
import { MemoryCard } from '@/components/memories/MemoryCard'
import type { Memory, MemoryType, SearchRequest } from '@/types/memory'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface MemorySearchProps {
  /** Called when the user wants to switch back to list view. */
  onSwitchToList: () => void
  /** Called when user clicks edit visibility on a search result. */
  onEditVisibility: (memory: Memory) => void
  /** Called when user clicks delete on a search result. */
  onDelete: (id: string) => void
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TYPE_OPTIONS: Array<{ value: MemoryType | ''; label: string }> = [
  { value: '', label: 'Any type' },
  { value: 'information', label: 'Information' },
  { value: 'question', label: 'Question' },
  { value: 'request', label: 'Request' },
]

const LIMIT_OPTIONS = [5, 10, 20, 50]

const selectClass =
  'rounded-md border border-gray-600 bg-gray-800 px-3 py-1.5 text-sm text-gray-300 focus:outline-none focus:ring-2 focus:ring-primary-500'

const inputClass =
  'rounded-md border border-gray-600 bg-gray-800 px-3 py-1.5 text-sm text-gray-300 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-primary-500'

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MemorySearch({
  onSwitchToList,
  onEditVisibility,
  onDelete,
}: MemorySearchProps) {
  const { results, total, searching, error, search, clear } = useMemorySearch()

  // Search form state
  const [query, setQuery] = useState('')
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [filterType, setFilterType] = useState<MemoryType | ''>('')
  const [filterTag, setFilterTag] = useState('')
  const [filterFrom, setFilterFrom] = useState('')
  const [filterTo, setFilterTo] = useState('')
  const [limit, setLimit] = useState(10)
  const [hasSearched, setHasSearched] = useState(false)

  const handleSearch = () => {
    if (!query.trim()) return
    const request: SearchRequest = {
      query: query.trim(),
      limit,
      ...(filterType ? { type: filterType } : {}),
      ...(filterTag ? { tags: filterTag.split(',').map((t) => t.trim()).filter(Boolean) } : {}),
      ...(filterFrom ? { from: filterFrom } : {}),
      ...(filterTo ? { to: filterTo } : {}),
    }
    search(request)
    setHasSearched(true)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleSearch()
  }

  const handleClear = () => {
    setQuery('')
    setFilterType('')
    setFilterTag('')
    setFilterFrom('')
    setFilterTo('')
    setLimit(10)
    setHasSearched(false)
    clear()
  }

  const handleBackToList = () => {
    handleClear()
    onSwitchToList()
  }

  return (
    <div>
      {/* Search input row */}
      <div className="flex items-center gap-2 mb-3">
        <div className="relative flex-1">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" aria-hidden="true" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Semantic search across memories…"
            aria-label="Semantic search query"
            className="w-full rounded-md border border-gray-600 bg-gray-800 pl-9 pr-3 py-2 text-sm text-gray-300 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-primary-500"
          />
        </div>

        <button
          type="button"
          onClick={handleSearch}
          disabled={!query.trim() || searching}
          className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-500 disabled:opacity-50 transition-colors"
        >
          {searching ? (
            <Loader2 size={16} className="animate-spin" />
          ) : (
            'Search'
          )}
        </button>

        {hasSearched && (
          <button
            type="button"
            onClick={handleClear}
            aria-label="Clear search"
            className="rounded-md p-2 text-gray-400 hover:bg-gray-700 hover:text-white transition-colors"
          >
            <X size={16} />
          </button>
        )}
      </div>

      {/* Advanced filters toggle */}
      <button
        type="button"
        onClick={() => setShowAdvanced((v) => !v)}
        aria-expanded={showAdvanced}
        className="mb-3 flex items-center gap-1 text-xs text-gray-400 hover:text-gray-300"
      >
        {showAdvanced ? (
          <ChevronDown size={12} aria-hidden="true" />
        ) : (
          <ChevronRight size={12} aria-hidden="true" />
        )}
        Advanced filters
      </button>

      {/* Advanced filters panel */}
      {showAdvanced && (
        <div className="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-gray-700 bg-gray-800/50 p-3">
          {/* Type filter */}
          <select
            aria-label="Filter by type"
            value={filterType}
            onChange={(e) => setFilterType(e.target.value as MemoryType | '')}
            className={selectClass}
          >
            {TYPE_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>

          {/* Tag filter */}
          <input
            type="text"
            aria-label="Filter by tags"
            placeholder="Tags…"
            value={filterTag}
            onChange={(e) => setFilterTag(e.target.value)}
            className={[inputClass, 'w-28'].join(' ')}
          />

          {/* From date */}
          <input
            type="date"
            aria-label="From date"
            value={filterFrom}
            onChange={(e) => setFilterFrom(e.target.value)}
            className={[inputClass, 'w-36'].join(' ')}
          />

          {/* To date */}
          <input
            type="date"
            aria-label="To date"
            value={filterTo}
            onChange={(e) => setFilterTo(e.target.value)}
            className={[inputClass, 'w-36'].join(' ')}
          />

          {/* Limit */}
          <select
            aria-label="Result limit"
            value={limit}
            onChange={(e) => setLimit(Number(e.target.value))}
            className={selectClass}
          >
            {LIMIT_OPTIONS.map((n) => (
              <option key={n} value={n}>
                Max {n}
              </option>
            ))}
          </select>
        </div>
      )}

      {/* Back to list link */}
      <div className="mb-4">
        <button
          type="button"
          onClick={handleBackToList}
          className="text-xs text-primary-400 hover:text-primary-300 transition-colors"
        >
          ← Back to memory list
        </button>
      </div>

      {/* Search loading */}
      {searching && (
        <div className="flex items-center justify-center gap-2 py-12 text-sm text-gray-400">
          <Loader2 size={16} className="animate-spin" />
          Searching memories…
        </div>
      )}

      {/* Search error */}
      {!searching && error && (
        <div className="rounded-lg border border-red-800 bg-red-900/20 px-4 py-3 text-sm text-red-400">
          {error}
        </div>
      )}

      {/* No results */}
      {!searching && !error && hasSearched && results.length === 0 && (
        <div className="py-12 text-center">
          <Search size={32} className="mx-auto mb-3 text-gray-600" aria-hidden="true" />
          <p className="text-gray-400">No matching memories found</p>
          <p className="mt-1 text-xs text-gray-600">
            Try a different query or adjust the advanced filters.
          </p>
        </div>
      )}

      {/* Search results */}
      {!searching && results.length > 0 && (
        <>
          <div className="mb-3 text-xs text-gray-500">
            {total} result{total !== 1 ? 's' : ''} for &ldquo;{query}&rdquo;
          </div>
          <ul className="space-y-3" aria-label="Search results">
            {results.map((m) => (
              <li key={m.id}>
                <MemoryCard
                  memory={m}
                  onEditVisibility={onEditVisibility}
                  onDelete={onDelete}
                />
              </li>
            ))}
          </ul>
        </>
      )}

      {/* Initial state (before any search) */}
      {!searching && !error && !hasSearched && (
        <div className="py-12 text-center">
          <Search size={32} className="mx-auto mb-3 text-gray-600" aria-hidden="true" />
          <p className="text-gray-400">Enter a query to search memories</p>
          <p className="mt-1 text-xs text-gray-600">
            Uses semantic similarity to find the most relevant memories.
          </p>
        </div>
      )}
    </div>
  )
}

export default MemorySearch
