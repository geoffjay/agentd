/**
 * NotificationFilters — filter and sort controls for the notification list.
 *
 * Provides:
 * - Status filter: All, Pending, Viewed, Responded, Dismissed, Expired
 * - Priority filter: All, Low, Normal, High, Urgent
 * - Source filter: All, System, AskService, AgentHook, MonitorService
 * - Sort order: Newest first, Oldest first, Priority (high to low)
 */

import type { NotificationFilters as Filters, SortOrder } from '@/hooks/useNotifications'
import type { NotificationPriority, NotificationStatus } from '@/types/notify'

// ---------------------------------------------------------------------------
// Option lists
// ---------------------------------------------------------------------------

const STATUS_OPTIONS: Array<{ value: NotificationStatus | 'All'; label: string }> = [
  { value: 'All', label: 'All statuses' },
  { value: 'pending', label: 'Pending' },
  { value: 'viewed', label: 'Viewed' },
  { value: 'responded', label: 'Responded' },
  { value: 'dismissed', label: 'Dismissed' },
  { value: 'expired', label: 'Expired' },
]

const PRIORITY_OPTIONS: Array<{ value: NotificationPriority | 'All'; label: string }> = [
  { value: 'All', label: 'All priorities' },
  { value: 'urgent', label: 'Urgent' },
  { value: 'high', label: 'High' },
  { value: 'normal', label: 'Normal' },
  { value: 'low', label: 'Low' },
]

/** Source filter options use the API's type-discriminant strings. */
const SOURCE_OPTIONS: Array<{ value: string; label: string }> = [
  { value: 'All', label: 'All sources' },
  { value: 'system', label: 'System' },
  { value: 'ask_service', label: 'Ask Service' },
  { value: 'agent_hook', label: 'Agent Hook' },
  { value: 'monitor_service', label: 'Monitor' },
]

const SORT_OPTIONS: Array<{ value: SortOrder; label: string }> = [
  { value: 'newest', label: 'Newest first' },
  { value: 'oldest', label: 'Oldest first' },
  { value: 'priority', label: 'Priority (high to low)' },
]

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface NotificationFiltersProps {
  filters: Filters
  sort: SortOrder
  onFiltersChange: (filters: Filters) => void
  onSortChange: (sort: SortOrder) => void
}

const selectClass =
  'rounded-md border border-gray-600 bg-gray-800 px-3 py-1.5 text-sm text-gray-300 focus:outline-none focus:ring-2 focus:ring-primary-500'

export function NotificationFilters({
  filters,
  sort,
  onFiltersChange,
  onSortChange,
}: NotificationFiltersProps) {
  const setStatus = (value: string) =>
    onFiltersChange({ ...filters, status: value as NotificationStatus | 'All' })

  const setPriority = (value: string) =>
    onFiltersChange({ ...filters, priority: value as NotificationPriority | 'All' })

  const setSource = (value: string) =>
    onFiltersChange({ ...filters, source: value })

  const hasActiveFilters =
    (filters.status && filters.status !== 'All') ||
    (filters.priority && filters.priority !== 'All') ||
    (filters.source && filters.source !== 'All')

  const resetFilters = () =>
    onFiltersChange({ status: 'All', priority: 'All', source: 'All' })

  return (
    <div className="flex flex-wrap items-center gap-2">
      {/* Status */}
      <select
        aria-label="Filter by status"
        value={filters.status ?? 'All'}
        onChange={(e) => setStatus(e.target.value)}
        className={selectClass}
      >
        {STATUS_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>

      {/* Priority */}
      <select
        aria-label="Filter by priority"
        value={filters.priority ?? 'All'}
        onChange={(e) => setPriority(e.target.value)}
        className={selectClass}
      >
        {PRIORITY_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>

      {/* Source */}
      <select
        aria-label="Filter by source"
        value={filters.source ?? 'All'}
        onChange={(e) => setSource(e.target.value)}
        className={selectClass}
      >
        {SOURCE_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>

      {/* Sort */}
      <select
        aria-label="Sort order"
        value={sort}
        onChange={(e) => onSortChange(e.target.value as SortOrder)}
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
          onClick={resetFilters}
          className="rounded-md px-2.5 py-1.5 text-xs font-medium text-gray-400 hover:text-white hover:bg-gray-700 transition-colors"
        >
          Reset filters
        </button>
      )}
    </div>
  )
}

export default NotificationFilters
