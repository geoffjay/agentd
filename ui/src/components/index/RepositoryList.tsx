/**
 * RepositoryList — table of indexed repositories with status badges and
 * per-row actions (reindex, delete).
 *
 * Columns: Name, Path, Status, Last Indexed, Actions
 * - Status badge uses color-coded pill matching RepoStatus variants
 * - Reindex and Delete buttons are disabled while the row is busy
 * - Empty state rendered via DataTable
 */

import { RefreshCw, Trash2 } from "lucide-react";
import type { ColumnDef } from "@/components/common";
import { DataTable } from "@/components/common";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { useState } from "react";
import type { RepoRecord, RepoStatus } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Status badge
// ---------------------------------------------------------------------------

const STATUS_STYLES: Record<RepoStatus, string> = {
	pending: "bg-th-status-warning-bg text-th-status-warning-text",
	indexing: "bg-th-status-info-bg text-th-status-info-text",
	ready: "bg-th-status-success-bg text-th-status-success-text",
	error: "bg-th-status-error-bg text-th-status-error-text",
};

function StatusBadge({ status }: { status: RepoStatus }) {
	const label = status.charAt(0).toUpperCase() + status.slice(1);
	return (
		<span
			className={[
				"inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium",
				STATUS_STYLES[status] ?? "bg-th-surface-sunken text-th-text-muted",
			].join(" ")}
		>
			{label}
		</span>
	);
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface RepositoryListProps {
	repositories: RepoRecord[];
	loading: boolean;
	busyIds: Set<string>;
	onReindex: (id: string) => Promise<boolean>;
	onDelete: (id: string) => Promise<boolean>;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function RepositoryList({
	repositories,
	loading,
	busyIds,
	onReindex,
	onDelete,
}: RepositoryListProps) {
	const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
	const [deleteLoading, setDeleteLoading] = useState(false);

	const handleDeleteConfirm = async () => {
		if (!deleteTarget) return;
		setDeleteLoading(true);
		try {
			await onDelete(deleteTarget);
		} finally {
			setDeleteLoading(false);
			setDeleteTarget(null);
		}
	};

	const columns: ColumnDef<RepoRecord>[] = [
		{
			key: "name",
			header: "Name",
			render: (r) => (
				<span className="text-sm font-medium text-th-text">{r.name}</span>
			),
		},
		{
			key: "path",
			header: "Path",
			render: (r) => (
				<span className="truncate max-w-xs text-sm text-th-text-muted font-mono">
					{r.path}
				</span>
			),
		},
		{
			key: "status",
			header: "Status",
			render: (r) => <StatusBadge status={r.status} />,
		},
		{
			key: "last_indexed",
			header: "Last Indexed",
			render: (r) =>
				r.last_indexed ? (
					<span className="text-sm text-th-text-muted whitespace-nowrap">
						{new Date(r.last_indexed).toLocaleString()}
					</span>
				) : (
					<span className="text-sm text-th-text-muted">Never</span>
				),
		},
		{
			key: "actions",
			header: "",
			render: (r) => {
				const busy = busyIds.has(r.id);
				return (
					<div className="flex items-center gap-1 justify-end">
						<button
							type="button"
							onClick={(e) => {
								e.stopPropagation();
								void onReindex(r.id);
							}}
							disabled={busy}
							aria-label={`Reindex ${r.name}`}
							title="Reindex"
							className="rounded p-1.5 text-th-text-muted hover:text-th-text hover:bg-th-surface-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
						>
							<RefreshCw size={15} className={busy ? "animate-spin" : ""} />
						</button>
						<button
							type="button"
							onClick={(e) => {
								e.stopPropagation();
								setDeleteTarget(r.id);
							}}
							disabled={busy}
							aria-label={`Delete ${r.name}`}
							title="Delete"
							className="rounded p-1.5 text-th-text-muted hover:text-th-status-error-text hover:bg-th-status-error-bg transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
						>
							<Trash2 size={15} />
						</button>
					</div>
				);
			},
		},
	];

	const targetName =
		repositories.find((r) => r.id === deleteTarget)?.name ?? "this repository";

	return (
		<>
			<DataTable
				columns={columns}
				data={repositories}
				rowKey={(r) => r.id}
				loading={loading}
				emptyTitle="No repositories indexed"
				emptyDescription="Add a repository to start indexing its source code."
			/>

			<ConfirmDialog
				open={deleteTarget !== null}
				title="Delete repository"
				description={`Remove "${targetName}" from the index? Indexed data will be deleted and cannot be recovered.`}
				confirmLabel="Delete"
				variant="danger"
				loading={deleteLoading}
				onConfirm={handleDeleteConfirm}
				onCancel={() => setDeleteTarget(null)}
			/>
		</>
	);
}

export default RepositoryList;
