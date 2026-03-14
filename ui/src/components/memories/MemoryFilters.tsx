/**
 * MemoryFilters — filter controls for the memory list.
 *
 * Provides:
 * - Memory type dropdown (All, Information, Question, Request)
 * - Visibility dropdown (All, Public, Shared, Private)
 * - Creator text filter
 * - Tag text filter
 * - Sort order dropdown
 * - Reset filters button
 */

import type { MemoryFilters as Filters, MemorySortField, MemorySortDir } from '@/hooks/useMemories'
import type { MemoryType, VisibilityLevel } from '@/types/memory'

// ---------------------------------------------------------------------------
// Option lists
// ---------------------------------------------------------------------------

const TYPE_OPTIONS: Array<{ value: MemoryType | 'All'; label: string }> = [
  { value: 'All', label: 'All types' },
  { value: 'information', label: 'Information' },
  { value: 'question', label: 'Question' },
  { value: 'request', label: 'Request' },
]

const VISIBILITY_OPTIONS: Array<{ value: VisibilityLevel | 'All'; label: string }> = [
  { value: 'All', label: 'All visibility' },
  { value: 'public', label: 'Public' },
  { value: 'shared', label: 'Shared' },
  { value: 'private', label: 'Private' },
]

const SORT_OPTIONS: Array<{ value: string; label: string }> = [
  { value: 'created_at:desc', label: 'Newest first' },
  { value: 'created_at:asc', label: 'Oldest first' },
  { value: 'updated_at:desc', label: 'Recently updated' },
  { value: 'type:asc', label: 'Type (A–Z)' },
]

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MemoryFiltersProps {
  filters: Filters
  sortBy: MemorySortField
  sortDir: MemorySortDir
  search: string
  onFiltersChange: (filters: Filters) => void
  onSortChange: (field: MemorySortField, dir: MemorySortDir) => void
  onSearchChange: (search: string) => void
}

const selectClass =
  'rounded-md border border-gray-600 bg-gray-800 px-3 py-1.5 text-sm text-gray-300 focus:outline-none focus:ring-2 focus:ring-primary-500'

const inputClass =
  'rounded-md border border-gray-600 bg-gray-800 px-3 py-1.5 text-sm text-gray-300 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-primary-500'

export function MemoryFilters({
  filters,
  sortBy,
  sortDir,
  search,
  onFiltersChange,
  onSortChange,
  onSearchChange,
}: MemoryFiltersProps) {
  const setType = (value: string) =>
    onFiltersChange({
      ...filters,
      type: value === 'All' ? undefined : (value as MemoryType),
    })

  const setVisibility = (value: string) =>
    onFiltersChange({
      ...filters,
      visibility: value === 'All' ? undefined : (value as VisibilityLevel),
    })

  const setCreatedBy = (value: string) =>
    onFiltersChange({ ...filters, created_by: value || undefined })

  const setTag = (value: string) =>
    onFiltersChange({ ...filters, tag: value || undefined })

  const handleSortChange = (value: string) => {
    const [field, dir] = value.split(':') as [MemorySortField, MemorySortDir]
    onSortChange(field, dir)
  }

  const hasActiveFilters =
    filters.type || filters.visibility || filters.created_by || filters.tag || search

  const resetAll = () => {
    onFiltersChange({})
    onSearchChange('')
  }

  return (
    <div className="flex flex-wrap items-center gap-2">
      {/* Content search */}
      <input
        type="text"
        aria-label="Search memory content"
        placeholder="Search content…"
        value={search}
        onChange={(e) => onSearchChange(e.target.value)}
        className={[inputClass, 'w-44'].join(' ')}
      />

      {/* Type */}
      <select
        aria-label="Filter by type"
        value={filters.type ?? 'All'}
        onChange={(e) => setType(e.target.value)}
        className={selectClass}
      >
        {TYPE_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>

      {/* Visibility */}
      <select
        aria-label="Filter by visibility"
        value={filters.visibility ?? 'All'}
        onChange={(e) => setVisibility(e.target.value)}
        className={selectClass}
      >
        {VISIBILITY_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>

      {/* Creator */}
      <input
        type="text"
        aria-label="Filter by creator"
        placeholder="Creator…"
        value={filters.created_by ?? ''}
        onChange={(e) => setCreatedBy(e.target.value)}
        className={[inputClass, 'w-32'].join(' ')}
      />

      {/* Tag */}
      <input
        type="text"
        aria-label="Filter by tag"
        placeholder="Tag…"
        value={filters.tag ?? ''}
        onChange={(e) => setTag(e.target.value)}
        className={[inputClass, 'w-28'].join(' ')}
      />

      {/* Sort */}
      <select
        aria-label="Sort order"
        value={`${sortBy}:${sortDir}`}
        onChange={(e) => handleSortChange(e.target.value)}
        className={selectClass}
      >
        {SORT_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>

      {/* Reset */}
      {hasActiveFilters && (
        <button
          type="button"
          onClick={resetAll}
          className="rounded-md px-2.5 py-1.5 text-xs font-medium text-gray-400 hover:text-white hover:bg-gray-700 transition-colors"
        >
          Reset filters
        </button>
      )}
    </div>
  )
}

export default MemoryFilters
