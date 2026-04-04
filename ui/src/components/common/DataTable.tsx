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

import { ArrowUpDown, ChevronDown, ChevronUp } from "lucide-react";
import { ListItemSkeleton } from "./LoadingSkeleton";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ColumnDef<T> {
	/** Unique key for this column */
	key: string;
	/** Header label */
	header: string;
	/** Whether the column is sortable */
	sortable?: boolean;
	/** Sort field key (defaults to `key`) */
	sortField?: string;
	/** Additional header class names */
	headerClassName?: string;
	/** Additional cell class names */
	cellClassName?: string;
	/** Render the cell content for a given row */
	render: (row: T) => React.ReactNode;
}

export interface BulkAction {
	label: string;
	icon?: React.ReactNode;
	onClick: () => void;
	variant?: "default" | "danger" | "success";
}

export interface DataTableProps<T> {
	/** Column definitions */
	columns: ColumnDef<T>[];
	/** Row data */
	data: T[];
	/** Extract a unique key from a row */
	rowKey: (row: T) => string;
	/** Whether data is loading */
	loading?: boolean;
	/** Number of skeleton rows to show while loading */
	loadingRows?: number;
	/** Current sort field */
	sortBy?: string;
	/** Current sort direction */
	sortDir?: "asc" | "desc";
	/** Called when a sortable column header is clicked */
	onSort?: (field: string) => void;
	/** Called when a row is clicked */
	onRowClick?: (row: T) => void;
	/** Empty state message */
	emptyTitle?: string;
	/** Empty state description */
	emptyDescription?: string;
	/** Whether to show selection checkboxes */
	selectable?: boolean;
	/** Currently selected row IDs */
	selectedIds?: string[];
	/** Called when selection changes */
	onSelectChange?: (ids: string[]) => void;
	/** Bulk actions shown when items are selected */
	bulkActions?: BulkAction[];
	/** Label for the clear selection button */
	clearSelectionLabel?: string;
}

// ---------------------------------------------------------------------------
// Sort header
// ---------------------------------------------------------------------------

interface SortHeaderProps {
	field: string;
	label: string;
	currentSort?: string;
	currentDir?: "asc" | "desc";
	onSort?: (field: string) => void;
}

/** Return the aria-sort value for a given sort header. */
function getAriaSort(
	field: string,
	currentSort?: string,
	currentDir?: "asc" | "desc",
): "ascending" | "descending" | "none" {
	if (currentSort === field) {
		return currentDir === "asc" ? "ascending" : "descending";
	}
	return "none";
}

function SortHeader({
	field,
	label,
	currentSort,
	currentDir,
	onSort,
}: SortHeaderProps) {
	const isActive = currentSort === field;
	return (
		<button
			type="button"
			onClick={() => onSort?.(field)}
			className="flex items-center gap-1 font-medium hover:text-th-text"
		>
			{label}
			{isActive ? (
				currentDir === "asc" ? (
					<ChevronUp size={13} aria-hidden="true" />
				) : (
					<ChevronDown size={13} aria-hidden="true" />
				)
			) : (
				<ArrowUpDown size={13} aria-hidden="true" className="opacity-40" />
			)}
		</button>
	);
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
	emptyTitle = "No items found.",
	emptyDescription = "",
	selectable = false,
	selectedIds = [],
	onSelectChange,
	bulkActions = [],
	clearSelectionLabel = "Clear selection",
}: DataTableProps<T>) {
	const colCount = columns.length + (selectable ? 1 : 0);
	const allSelected =
		data.length > 0 && data.every((row) => selectedIds.includes(rowKey(row)));
	const someSelected = selectedIds.length > 0;

	function toggleAll(checked: boolean) {
		if (!onSelectChange) return;
		if (checked) {
			onSelectChange(data.map((row) => rowKey(row)));
		} else {
			onSelectChange([]);
		}
	}

	function toggleOne(id: string, checked: boolean) {
		if (!onSelectChange) return;
		if (checked) {
			onSelectChange([...selectedIds, id]);
		} else {
			onSelectChange(selectedIds.filter((s) => s !== id));
		}
	}

	const BULK_VARIANT_STYLES: Record<string, string> = {
		default: "bg-th-surface-hover text-th-text hover:opacity-90 focus:ring-th-focus-ring",
		danger: "bg-th-status-error-dot text-th-accent-text hover:opacity-90 focus:ring-th-focus-ring",
		success: "bg-th-status-success-dot text-th-accent-text hover:opacity-90 focus:ring-th-focus-ring",
	};

	return (
		<div className="overflow-hidden rounded-lg border border-th-border">
			{/* Bulk action toolbar */}
			{someSelected && bulkActions.length > 0 && (
				<div className="flex items-center gap-3 border-b border-th-border bg-th-accent-subtle px-4 py-2.5">
					<span className="text-sm font-medium text-th-text-link">
						{selectedIds.length} selected
					</span>
					{bulkActions.map((action) => (
						<button
							key={action.label}
							type="button"
							onClick={action.onClick}
							className={[
								"flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium focus:outline-none focus:ring-2 focus:ring-offset-1",
								BULK_VARIANT_STYLES[action.variant ?? "default"],
							].join(" ")}
						>
							{action.icon}
							{action.label}
						</button>
					))}
					<button
						type="button"
						onClick={() => onSelectChange?.([])}
						className="text-xs text-th-text-muted hover:text-th-text"
					>
						{clearSelectionLabel}
					</button>
				</div>
			)}

			<div className="overflow-x-auto">
				<table className="min-w-full divide-y divide-th-border">
					<thead className="bg-th-surface-sunken">
						<tr>
							{/* Select all checkbox */}
							{selectable && (
								<th className="w-10 px-4 py-3">
									<input
										type="checkbox"
										aria-label="Select all"
										checked={allSelected}
										onChange={(e) => toggleAll(e.target.checked)}
										className="h-4 w-4 rounded border-th-border-strong text-th-accent focus:ring-th-focus-ring"
									/>
								</th>
							)}

							{columns.map((col) => (
								<th
									key={col.key}
									className={[
										"px-4 py-3 text-left text-xs text-th-text-muted",
										col.headerClassName ?? "",
									]
										.filter(Boolean)
										.join(" ")}
									aria-sort={
										col.sortable && onSort
											? getAriaSort(col.sortField ?? col.key, sortBy, sortDir)
											: undefined
									}
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

					<tbody className="divide-y divide-th-border-subtle bg-th-surface">
						{loading ? (
							<tr>
								<td colSpan={colCount} className="p-4">
									<ListItemSkeleton rows={loadingRows} />
								</td>
							</tr>
						) : data.length === 0 ? (
							<tr>
								<td colSpan={colCount} className="py-12 text-center">
									<p className="text-sm text-th-text-muted">
										{emptyTitle}
									</p>
									{emptyDescription && (
										<p className="mt-1 text-xs text-th-text-muted">
											{emptyDescription}
										</p>
									)}
								</td>
							</tr>
						) : (
							data.map((row) => {
								const id = rowKey(row);
								const isSelected = selectedIds.includes(id);
								return (
									<tr
										key={id}
										className={[
											"border-b border-th-border-subtle hover:bg-th-surface-hover",
											onRowClick ? "cursor-pointer" : "",
										].join(" ")}
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
													className="h-4 w-4 rounded border-th-border-strong text-th-accent focus:ring-th-focus-ring"
												/>
											</td>
										)}

										{columns.map((col) => (
											<td
												key={col.key}
												className={[
													"px-4 py-3 text-sm",
													col.cellClassName ??
														"text-th-text-muted",
												].join(" ")}
											>
												{col.render(row)}
											</td>
										))}
									</tr>
								);
							})
						)}
					</tbody>
				</table>
			</div>
		</div>
	);
}

export default DataTable;
