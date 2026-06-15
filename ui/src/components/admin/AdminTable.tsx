/**
 * AdminTable — reusable read-only table for the product-admin entity views.
 *
 * Renders a titled, refreshable table with loading/error/empty states and
 * simple offset-based prev/next pagination. Columns are described declaratively
 * so each admin page only defines its headers and cell renderers.
 */

import { AlertCircle, RefreshCw } from "lucide-react";
import { ListItemSkeleton } from "@/components/common/LoadingSkeleton";

export interface AdminColumn<T> {
	header: string;
	render: (row: T) => React.ReactNode;
}

interface AdminTableProps<T> {
	title: string;
	description?: string;
	columns: AdminColumn<T>[];
	rows: T[];
	rowKey: (row: T) => string;
	loading: boolean;
	error?: string;
	total: number;
	offset: number;
	limit: number;
	onPage: (offset: number) => void;
	onRefresh: () => void;
}

export function AdminTable<T>({
	title,
	description,
	columns,
	rows,
	rowKey,
	loading,
	error,
	total,
	offset,
	limit,
	onPage,
	onRefresh,
}: AdminTableProps<T>) {
	const start = total === 0 ? 0 : offset + 1;
	const end = Math.min(offset + limit, total);
	const canPrev = offset > 0;
	const canNext = offset + limit < total;

	return (
		<div className="space-y-4">
			<div className="flex items-center justify-between">
				<div>
					<h2 className="text-lg font-semibold text-th-text">{title}</h2>
					{description && (
						<p className="mt-1 text-sm text-th-text-muted">{description}</p>
					)}
				</div>
				<button
					type="button"
					aria-label={`Refresh ${title}`}
					onClick={onRefresh}
					disabled={loading}
					className="rounded-md border border-th-border-strong bg-th-surface p-2 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text-secondary focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-1 disabled:opacity-50"
				>
					<RefreshCw
						size={15}
						className={loading ? "animate-spin" : undefined}
					/>
				</button>
			</div>

			{error && (
				<div
					role="alert"
					className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text"
				>
					<div className="flex items-center gap-2">
						<AlertCircle className="h-4 w-4 flex-shrink-0" />
						{error}
					</div>
				</div>
			)}

			<div className="overflow-hidden rounded-lg border border-th-border">
				<div className="overflow-x-auto">
					<table className="min-w-full divide-y divide-th-border">
						<thead className="bg-th-surface-sunken">
							<tr>
								{columns.map((col) => (
									<th
										key={col.header}
										className="px-4 py-3 text-left text-xs font-medium text-th-text-muted"
									>
										{col.header}
									</th>
								))}
							</tr>
						</thead>
						<tbody className="divide-y divide-th-border bg-th-surface">
							{loading ? (
								<tr>
									<td colSpan={columns.length} className="p-4">
										<ListItemSkeleton rows={3} />
									</td>
								</tr>
							) : rows.length === 0 && !error ? (
								<tr>
									<td
										colSpan={columns.length}
										className="py-12 text-center text-sm text-th-text-muted"
									>
										No records found.
									</td>
								</tr>
							) : (
								rows.map((row) => (
									<tr
										key={rowKey(row)}
										className="border-b border-th-border hover:bg-th-surface-hover"
									>
										{columns.map((col) => (
											<td
												key={col.header}
												className="px-4 py-3 text-sm text-th-text"
											>
												{col.render(row)}
											</td>
										))}
									</tr>
								))
							)}
						</tbody>
					</table>
				</div>
			</div>

			{/* Pagination footer */}
			<div className="flex items-center justify-between text-sm text-th-text-muted">
				<span>
					{total === 0 ? "0 records" : `Showing ${start}–${end} of ${total}`}
				</span>
				<div className="flex gap-2">
					<button
						type="button"
						onClick={() => onPage(Math.max(0, offset - limit))}
						disabled={!canPrev || loading}
						className="rounded-md border border-th-border-strong bg-th-surface px-3 py-1 hover:bg-th-surface-hover disabled:opacity-40"
					>
						Previous
					</button>
					<button
						type="button"
						onClick={() => onPage(offset + limit)}
						disabled={!canNext || loading}
						className="rounded-md border border-th-border-strong bg-th-surface px-3 py-1 hover:bg-th-surface-hover disabled:opacity-40"
					>
						Next
					</button>
				</div>
			</div>
		</div>
	);
}

export default AdminTable;
