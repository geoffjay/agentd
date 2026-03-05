/**
 * SearchPalette — full-screen command palette overlay.
 *
 * Features:
 * - Opens via Ctrl+K / Cmd+K or by clicking the search button in the header
 * - Debounced search-as-you-type across agents and notifications
 * - Keyboard navigation: ↑/↓ to move, Enter to navigate, Escape to close
 * - Recent searches shown when input is empty
 * - Backdrop click to close
 * - Portal to document.body to avoid z-index issues
 */

import { useEffect, useRef, useCallback, useState } from 'react'
import { createPortal } from 'react-dom'
import { useNavigate } from 'react-router-dom'
import { Search, X } from 'lucide-react'
import { useSearch } from '@/hooks/useSearch'
import { SearchResults, RecentSearches } from './SearchResults'
import type { SearchResult } from '@/hooks/useSearch'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Flatten all result groups into an ordered list for keyboard navigation */
function flattenResults(
  results: ReturnType<typeof useSearch>['results'],
): SearchResult[] {
  return [...results.actions, ...results.agents, ...results.notifications]
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface SearchPaletteProps {
  isOpen: boolean
  onClose: () => void
}

export function SearchPalette({ isOpen, onClose }: SearchPaletteProps) {
  const navigate = useNavigate()
  const { query, setQuery, results, loading, recentSearches, addRecentSearch, clearRecentSearches } =
    useSearch()
  const inputRef = useRef<HTMLInputElement>(null)
  const [activeIndex, setActiveIndex] = useState(-1)

  const allResults = flattenResults(results)
  const activeId = activeIndex >= 0 && activeIndex < allResults.length
    ? allResults[activeIndex].id
    : null

  // Focus input when palette opens; reset state when it closes
  useEffect(() => {
    if (isOpen) {
      setActiveIndex(-1)
      setTimeout(() => inputRef.current?.focus(), 0)
    } else {
      setQuery('')
      setActiveIndex(-1)
    }
  }, [isOpen, setQuery])

  // Navigate to a result
  const selectResult = useCallback(
    (result: SearchResult) => {
      if (query.trim()) addRecentSearch(query.trim())
      onClose()
      navigate(result.href)
    },
    [query, addRecentSearch, onClose, navigate],
  )

  // Keyboard navigation within palette
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown': {
          e.preventDefault()
          setActiveIndex((i) => Math.min(i + 1, allResults.length - 1))
          break
        }
        case 'ArrowUp': {
          e.preventDefault()
          setActiveIndex((i) => Math.max(i - 1, -1))
          break
        }
        case 'Enter': {
          e.preventDefault()
          if (activeIndex >= 0 && activeIndex < allResults.length) {
            selectResult(allResults[activeIndex])
          }
          break
        }
        case 'Escape': {
          e.preventDefault()
          onClose()
          break
        }
        default:
          break
      }
    },
    [activeIndex, allResults, selectResult, onClose],
  )

  // Reset active index when results change
  useEffect(() => {
    setActiveIndex(-1)
  }, [results])

  if (!isOpen) return null

  const showEmpty = !query.trim()

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Global search"
      className="fixed inset-0 z-[100] flex items-start justify-center pt-[10vh] px-4"
    >
      {/* Backdrop */}
      <div
        aria-hidden="true"
        onClick={onClose}
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
      />

      {/* Palette panel */}
      <div
        className="relative z-10 w-full max-w-xl overflow-hidden rounded-xl border border-gray-200 bg-white shadow-2xl dark:border-gray-700 dark:bg-gray-900"
        onKeyDown={handleKeyDown}
      >
        {/* Search input row */}
        <div className="flex items-center gap-3 border-b border-gray-200 px-4 py-3 dark:border-gray-700">
          <Search
            size={18}
            className="shrink-0 text-gray-400 dark:text-gray-500"
            aria-hidden="true"
          />
          <input
            ref={inputRef}
            type="search"
            role="combobox"
            aria-autocomplete="list"
            aria-expanded={!showEmpty}
            aria-controls="search-results"
            aria-activedescendant={activeId ?? undefined}
            placeholder="Search agents, notifications, pages…"
            aria-label="Search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="min-w-0 flex-1 bg-transparent text-sm text-gray-900 placeholder-gray-400 outline-none dark:text-white dark:placeholder-gray-500"
          />
          {query && (
            <button
              type="button"
              aria-label="Clear search"
              onClick={() => setQuery('')}
              className="shrink-0 rounded p-0.5 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300"
            >
              <X size={14} />
            </button>
          )}
          <kbd className="hidden shrink-0 rounded border border-gray-200 px-1.5 py-0.5 text-[10px] text-gray-400 dark:border-gray-600 dark:text-gray-500 sm:block">
            Esc
          </kbd>
        </div>

        {/* Results area */}
        <div id="search-results" className="max-h-[60vh] overflow-y-auto">
          {showEmpty ? (
            <RecentSearches
              searches={recentSearches}
              onSelect={(q) => setQuery(q)}
              onClear={clearRecentSearches}
            />
          ) : (
            <SearchResults
              query={query}
              results={results}
              loading={loading}
              activeId={activeId}
              onSelect={selectResult}
            />
          )}
        </div>

        {/* Footer hint */}
        <div className="flex items-center gap-4 border-t border-gray-100 px-4 py-2 dark:border-gray-800">
          <span className="flex items-center gap-1 text-[11px] text-gray-400 dark:text-gray-500">
            <kbd className="rounded border border-gray-200 px-1 py-0.5 text-[10px] dark:border-gray-600">
              ↑↓
            </kbd>
            navigate
          </span>
          <span className="flex items-center gap-1 text-[11px] text-gray-400 dark:text-gray-500">
            <kbd className="rounded border border-gray-200 px-1 py-0.5 text-[10px] dark:border-gray-600">
              ↵
            </kbd>
            open
          </span>
          <span className="flex items-center gap-1 text-[11px] text-gray-400 dark:text-gray-500">
            <kbd className="rounded border border-gray-200 px-1 py-0.5 text-[10px] dark:border-gray-600">
              Esc
            </kbd>
            close
          </span>
        </div>
      </div>
    </div>,
    document.body,
  )
}

export default SearchPalette
