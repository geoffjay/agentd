/**
 * DataTable — reusable, sortable table component with bulk selection.
 *
 * Provides the consistent table styling used across all list pages
 * (agents, workflows, approvals, notifications, memories).
 *
 * Features:
 * - Sortable column headers
 * - Optional bulk selection with "select all" checkbox
 * - Row click handler
 * - Loading, empty, and error states
 * - Bulk action toolbar
 * - Responsive overflow handling
 */

import { ArrowUpDown, ChevronDown, ChevronUp } from 'lucide-react'
import { ListItemSkeleton } from './LoadingSkeleton'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ColumnDef<T> {
  /** Unique key for this column */
  key: string
  /** Header label */
  header: string
  /** Whether the column is sortable */
  sortable?: boolean
  /** Sort field key (defaults to `key`) */
  sortField?: string
  /** Additional header class names */
  headerClassName?: string
  /** Additional cell class names */
  cellClassName?: string
  /** Render the cell content for a given row */
  render: (row: T) => React.ReactNode
}

export interface BulkAction {
  label: string
  icon?: React.ReactNode
  onClick: () => void
  variant?: 'default' | 'danger' | 'success'
}

export interface DataTableProps<T> {
  /** Column definitions */
  columns: ColumnDef<T>[]
  /** Row data */
  data: T[]
  /** Extract a unique key from a row */
  rowKey: (row: T) => string
  /** Whether data is loading */
  loading?: boolean
  /** Number of skeleton rows to show while loading */
  loadingRows?: number
  /** Current sort field */
  sortBy?: string
  /** Current sort direction */
  sortDir?: 'asc' | 'desc'
  /** Called when a sortable column header is clicked */
  onSort?: (field: string) => void
  /** Called when a row is clicked */
  onRowClick?: (row: T) => void
  /** Empty state message */
  emptyTitle?: string
  /** Empty state description */
  emptyDescription?: string
  /** Whether to show selection checkboxes */
  selectable?: boolean
  /** Currently selected row IDs */
  selectedIds?: string[]
  /** Called when selection changes */
  onSelectChange?: (ids: string[]) => void
  /** Bulk actions shown when items are selected */
  bulkActions?: BulkAction[]
  /** Label for the clear selection button */
  clearSelectionLabel?: string
}

// ---------------------------------------------------------------------------
// Sort header
// ---------------------------------------------------------------------------

interface SortHeaderProps {
  field: string
  label: string
  currentSort?: string
  currentDir?: 'asc' | 'desc'
  onSort?: (field: string) => void
}

function SortHeader({ field, label, currentSort, currentDir, onSort }: SortHeaderProps) {
  const isActive = currentSort === field
  return (
    <button
      type="button"
      onClick={() => onSort?.(field)}
      className="flex items-center gap-1 font-medium hover:text-gray-900 dark:hover:text-white"
      aria-sort={isActive ? (currentDir === 'asc' ? 'ascending' : 'descending') : 'none'}
    >
      {label}
      {isActive ? (
        currentDir === 'asc' ? (
          <ChevronUp size={13} aria-hidden="true" />
        ) : (
          <ChevronDown size={13} aria-hidden="true" />
        )
      ) : (
        <ArrowUpDown size={13} aria-hidden="true" className="opacity-40" />
      )}
    </button>
  )
}

// ---------------------------------------------------------------------------
// DataTable
// ---------------------------------------------------------------------------

export function DataTable<T>({
  columns,
  data,
  rowKey,
  loading = false,
  loadingRows = 5,
  sortBy,
  sortDir,
  onSort,
  onRowClick,
  emptyTitle = 'No items found.',
  emptyDescription = '',
  selectable = false,
  selectedIds = [],
  onSelectChange,
  bulkActions = [],
  clearSelectionLabel = 'Clear selection',
}: DataTableProps<T>) {
  const colCount = columns.length + (selectable ? 1 : 0)
  const allSelected = data.length > 0 && data.every((row) => selectedIds.includes(rowKey(row)))
  const someSelected = selectedIds.length > 0

  function toggleAll(checked: boolean) {
    if (!onSelectChange) return
    if (checked) {
      onSelectChange(data.map((row) => rowKey(row)))
    } else {
      onSelectChange([])
    }
  }

  function toggleOne(id: string, checked: boolean) {
    if (!onSelectChange) return
    if (checked) {
      onSelectChange([...selectedIds, id])
    } else {
      onSelectChange(selectedIds.filter((s) => s !== id))
    }
  }

  const BULK_VARIANT_STYLES: Record<string, string> = {
    default:
      'bg-gray-600 text-white hover:bg-gray-500 focus:ring-gray-500',
    danger:
      'bg-red-600 text-white hover:bg-red-700 focus:ring-red-500',
    success:
      'bg-green-600 text-white hover:bg-green-700 focus:ring-green-500',
  }

  return (
    <div className="overflow-hidden rounded-lg border border-gray-200 dark:border-gray-700">
      {/* Bulk action toolbar */}
      {someSelected && bulkActions.length > 0 && (
        <div className="flex items-center gap-3 border-b border-gray-200 bg-primary-50 px-4 py-2.5 dark:border-gray-700 dark:bg-primary-900/20">
          <span className="text-sm font-medium text-primary-700 dark:text-primary-300">
            {selectedIds.length} selected
          </span>
          {bulkActions.map((action) => (
            <button
              key={action.label}
              type="button"
              onClick={action.onClick}
              className={[
                'flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium focus:outline-none focus:ring-2 focus:ring-offset-1',
                BULK_VARIANT_STYLES[action.variant ?? 'default'],
              ].join(' ')}
            >
              {action.icon}
              {action.label}
            </button>
          ))}
          <button
            type="button"
            onClick={() => onSelectChange?.([])}
            className="text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
          >
            {clearSelectionLabel}
          </button>
        </div>
      )}

      <div className="overflow-x-auto">
        <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
          <thead className="bg-gray-50 dark:bg-gray-800/50">
            <tr>
              {/* Select all checkbox */}
              {selectable && (
                <th className="w-10 px-4 py-3">
                  <input
                    type="checkbox"
                    aria-label="Select all"
                    checked={allSelected}
                    onChange={(e) => toggleAll(e.target.checked)}
                    className="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-700"
                  />
                </th>
              )}

              {columns.map((col) => (
                <th
                  key={col.key}
                  className={[
                    'px-4 py-3 text-left text-xs text-gray-500 dark:text-gray-400',
                    col.headerClassName ?? '',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                >
                  {col.sortable && onSort ? (
                    <SortHeader
                      field={col.sortField ?? col.key}
                      label={col.header}
                      currentSort={sortBy}
                      currentDir={sortDir}
                      onSort={onSort}
                    />
                  ) : (
                    <span className="font-medium">{col.header}</span>
                  )}
                </th>
              ))}
            </tr>
          </thead>

          <tbody className="divide-y divide-gray-100 bg-white dark:divide-gray-700 dark:bg-gray-900">
            {loading ? (
              <tr>
                <td colSpan={colCount} className="p-4">
                  <ListItemSkeleton rows={loadingRows} />
                </td>
              </tr>
            ) : data.length === 0 ? (
              <tr>
                <td colSpan={colCount} className="py-12 text-center">
                  <p className="text-sm text-gray-500 dark:text-gray-400">{emptyTitle}</p>
                  {emptyDescription && (
                    <p className="mt-1 text-xs text-gray-400 dark:text-gray-500">
                      {emptyDescription}
                    </p>
                  )}
                </td>
              </tr>
            ) : (
              data.map((row) => {
                const id = rowKey(row)
                const isSelected = selectedIds.includes(id)
                return (
                  <tr
                    key={id}
                    className={[
                      'border-b border-gray-100 hover:bg-gray-50 dark:border-gray-700 dark:hover:bg-gray-800/50',
                      onRowClick ? 'cursor-pointer' : '',
                    ].join(' ')}
                    onClick={() => onRowClick?.(row)}
                  >
                    {selectable && (
                      <td
                        className="w-10 px-4 py-3"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <input
                          type="checkbox"
                          aria-label={`Select row`}
                          checked={isSelected}
                          onChange={(e) => toggleOne(id, e.target.checked)}
                          className="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-700"
                        />
                      </td>
                    )}

                    {columns.map((col) => (
                      <td
                        key={col.key}
                        className={[
                          'px-4 py-3 text-sm',
                          col.cellClassName ?? 'text-gray-500 dark:text-gray-400',
                        ].join(' ')}
                      >
                        {col.render(row)}
                      </td>
                    ))}
                  </tr>
                )
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}

export default DataTable
