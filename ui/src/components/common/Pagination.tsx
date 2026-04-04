/**
 * Pagination — page navigation controls.
 *
 * Renders previous/next buttons and numbered page buttons.
 * Shows a window of up to 5 page numbers around the current page.
 */

import { ChevronLeft, ChevronRight } from "lucide-react";

export interface PaginationProps {
	/** Current page number (1-based) */
	page: number;
	/** Total number of pages */
	totalPages: number;
	/** Total number of items */
	totalItems: number;
	/** Items per page */
	pageSize: number;
	onPageChange: (page: number) => void;
}

export function Pagination({
	page,
	totalPages,
	totalItems,
	pageSize,
	onPageChange,
}: PaginationProps) {
	if (totalPages <= 1) return null;

	const startItem = (page - 1) * pageSize + 1;
	const endItem = Math.min(page * pageSize, totalItems);

	// Build a window of up to 5 visible page numbers
	const windowSize = 5;
	let windowStart = Math.max(1, page - Math.floor(windowSize / 2));
	const windowEnd = Math.min(totalPages, windowStart + windowSize - 1);
	windowStart = Math.max(1, windowEnd - windowSize + 1);

	const pageNumbers: number[] = [];
	for (let i = windowStart; i <= windowEnd; i++) {
		pageNumbers.push(i);
	}

	const btnBase =
		"inline-flex h-8 w-8 items-center justify-center rounded text-sm font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-1 disabled:pointer-events-none disabled:opacity-40";
	const btnActive = "bg-th-accent text-th-accent-text";
	const btnInactive =
		"border border-th-border-strong bg-th-surface text-th-text-secondary hover:bg-th-surface-hover";

	return (
		<nav
			aria-label="Pagination"
			className="flex flex-col items-center gap-3 sm:flex-row sm:justify-between"
		>
			{/* Item range */}
			<p className="text-sm text-th-text-muted">
				Showing{" "}
				<span className="font-medium text-th-text-secondary">
					{startItem}
				</span>
				–
				<span className="font-medium text-th-text-secondary">
					{endItem}
				</span>{" "}
				of{" "}
				<span className="font-medium text-th-text-secondary">
					{totalItems}
				</span>
			</p>

			{/* Page buttons */}
			<div className="flex items-center gap-1">
				{/* Previous */}
				<button
					type="button"
					aria-label="Previous page"
					onClick={() => onPageChange(page - 1)}
					disabled={page <= 1}
					className={[btnBase, btnInactive].join(" ")}
				>
					<ChevronLeft size={14} />
				</button>

				{/* First page + ellipsis */}
				{windowStart > 1 && (
					<>
						<button
							type="button"
							aria-label="Page 1"
							onClick={() => onPageChange(1)}
							className={[btnBase, btnInactive].join(" ")}
						>
							1
						</button>
						{windowStart > 2 && (
							<span className="px-1 text-th-text-muted">…</span>
						)}
					</>
				)}

				{/* Page window */}
				{pageNumbers.map((n) => (
					<button
						key={n}
						type="button"
						aria-label={`Page ${n}`}
						aria-current={n === page ? "page" : undefined}
						onClick={() => onPageChange(n)}
						className={[btnBase, n === page ? btnActive : btnInactive].join(
							" ",
						)}
					>
						{n}
					</button>
				))}

				{/* Last page + ellipsis */}
				{windowEnd < totalPages && (
					<>
						{windowEnd < totalPages - 1 && (
							<span className="px-1 text-th-text-muted">…</span>
						)}
						<button
							type="button"
							aria-label={`Page ${totalPages}`}
							onClick={() => onPageChange(totalPages)}
							className={[btnBase, btnInactive].join(" ")}
						>
							{totalPages}
						</button>
					</>
				)}

				{/* Next */}
				<button
					type="button"
					aria-label="Next page"
					onClick={() => onPageChange(page + 1)}
					disabled={page >= totalPages}
					className={[btnBase, btnInactive].join(" ")}
				>
					<ChevronRight size={14} />
				</button>
			</div>
		</nav>
	);
}

export default Pagination;
