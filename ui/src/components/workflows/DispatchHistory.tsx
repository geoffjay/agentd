/**
 * DispatchHistory — paginated table of workflow dispatch records.
 *
 * Shows source ID, prompt excerpt, status badge, and timestamps.
 * Supports filtering by status and pagination.
 */

import { ChevronDown, ChevronUp } from "lucide-react";
import { useState } from "react";
import { ListItemSkeleton } from "@/components/common/LoadingSkeleton";
import { Pagination } from "@/components/common/Pagination";
import { useDispatchHistory } from "@/hooks/useWorkflows";
import type { DispatchRecord, DispatchStatus } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface DispatchHistoryProps {
	workflowId: string;
	pageSize?: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const STATUS_STYLES: Record<DispatchStatus, string> = {
	pending: "bg-th-status-warning-bg text-th-status-warning-text",
	dispatched: "bg-th-status-info-bg text-th-status-info-text",
	completed: "bg-th-status-success-bg text-th-status-success-text",
	failed: "bg-th-status-error-bg text-th-status-error-text",
	skipped: "bg-th-surface-sunken text-th-text-muted",
};

function formatDate(iso: string): string {
	return new Date(iso).toLocaleString(undefined, {
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	});
}

function truncate(text: string, maxLen = 80): string {
	return text.length > maxLen ? text.slice(0, maxLen) + "…" : text;
}

// ---------------------------------------------------------------------------
// Status filter
// ---------------------------------------------------------------------------

const STATUS_OPTIONS: Array<{ value: DispatchStatus | "all"; label: string }> =
	[
		{ value: "all", label: "All" },
		{ value: "dispatched", label: "Dispatched" },
		{ value: "completed", label: "Completed" },
		{ value: "failed", label: "Failed" },
		{ value: "skipped", label: "Skipped" },
		{ value: "pending", label: "Pending" },
	];

// ---------------------------------------------------------------------------
// Row component
// ---------------------------------------------------------------------------

function DispatchRow({ record }: { record: DispatchRecord }) {
	const [expanded, setExpanded] = useState(false);

	return (
		<>
			<tr className="border-t border-th-border-subtle hover:bg-th-surface-hover">
				<td className="py-2 px-4 font-mono text-xs text-th-text-muted">
					{record.source_id}
				</td>
				<td className="py-2 px-4 text-sm">
					<div className="flex items-start gap-1">
						<span className="text-th-text-secondary leading-relaxed">
							{truncate(record.prompt_sent)}
						</span>
						{record.prompt_sent.length > 80 && (
							<button
								type="button"
								onClick={() => setExpanded((v) => !v)}
								className="flex-shrink-0 text-th-text-link hover:text-th-text-link focus-visible:outline-none"
								aria-label={expanded ? "Collapse prompt" : "Expand prompt"}
							>
								{expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
							</button>
						)}
					</div>
				</td>
				<td className="py-2 px-4">
					<span
						className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[record.status]}`}
					>
						{record.status}
					</span>
				</td>
				<td className="py-2 px-4 text-xs text-th-text-muted whitespace-nowrap">
					{formatDate(record.dispatched_at)}
				</td>
				<td className="py-2 px-4 text-xs text-th-text-faint whitespace-nowrap">
					{record.completed_at ? formatDate(record.completed_at) : "—"}
				</td>
			</tr>

			{expanded && (
				<tr className="bg-th-surface-sunken">
					<td colSpan={5} className="px-4 py-2">
						<pre className="text-xs font-mono text-th-text-secondary whitespace-pre-wrap">
							{record.prompt_sent}
						</pre>
					</td>
				</tr>
			)}
		</>
	);
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function DispatchHistory({
	workflowId,
	pageSize = 20,
}: DispatchHistoryProps) {
	const [page, setPage] = useState(1);
	const [statusFilter, setStatusFilter] = useState<DispatchStatus | "all">(
		"all",
	);

	const { dispatches, total, loading, error } = useDispatchHistory({
		workflowId,
		page,
		pageSize,
		status: statusFilter === "all" ? undefined : statusFilter,
	});

	return (
		<div className="space-y-3">
			{/* Filter bar */}
			<div className="flex items-center gap-2">
				<span className="text-xs text-th-text-muted">
					Filter:
				</span>
				<div className="flex gap-1">
					{STATUS_OPTIONS.map((opt) => (
						<button
							key={opt.value}
							type="button"
							onClick={() => {
								setStatusFilter(opt.value);
								setPage(1);
							}}
							className={[
								"rounded px-2 py-0.5 text-xs transition-colors",
								statusFilter === opt.value
									? "bg-th-accent-subtle text-th-text-link font-medium"
									: "text-th-text-muted hover:text-th-text",
							].join(" ")}
						>
							{opt.label}
						</button>
					))}
				</div>
			</div>

			{/* Table */}
			<div className="overflow-x-auto rounded-lg border border-th-border">
				<table className="min-w-full text-sm">
					<thead className="bg-th-surface-sunken">
						<tr>
							<th className="py-2 px-4 text-left text-xs font-medium text-th-text-muted uppercase tracking-wide">
								Source ID
							</th>
							<th className="py-2 px-4 text-left text-xs font-medium text-th-text-muted uppercase tracking-wide">
								Prompt sent
							</th>
							<th className="py-2 px-4 text-left text-xs font-medium text-th-text-muted uppercase tracking-wide">
								Status
							</th>
							<th className="py-2 px-4 text-left text-xs font-medium text-th-text-muted uppercase tracking-wide">
								Dispatched
							</th>
							<th className="py-2 px-4 text-left text-xs font-medium text-th-text-muted uppercase tracking-wide">
								Completed
							</th>
						</tr>
					</thead>
					<tbody>
						{loading ? (
							Array.from({ length: 3 }).map((_, i) => (
								<tr
									key={i}
									className="border-t border-th-border-subtle"
								>
									<td colSpan={5} className="p-2">
										<ListItemSkeleton />
									</td>
								</tr>
							))
						) : error ? (
							<tr>
								<td
									colSpan={5}
									className="py-8 text-center text-sm text-th-status-error-text"
								>
									{error}
								</td>
							</tr>
						) : dispatches.length === 0 ? (
							<tr>
								<td
									colSpan={5}
									className="py-8 text-center text-sm text-th-text-faint"
								>
									No dispatches found.
								</td>
							</tr>
						) : (
							dispatches.map((record) => (
								<DispatchRow key={record.id} record={record} />
							))
						)}
					</tbody>
				</table>
			</div>

			{/* Pagination */}
			{total > pageSize && (
				<Pagination
					page={page}
					totalPages={Math.ceil(total / pageSize)}
					totalItems={total}
					pageSize={pageSize}
					onPageChange={setPage}
				/>
			)}
		</div>
	);
}

export default DispatchHistory;
