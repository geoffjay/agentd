/**
 * ApprovalQueue — global tool-approval queue page.
 *
 * Keyboard shortcuts (when no input focused):
 *   A  →  Approve selected
 *   D  →  Deny selected
 */

import { Check, RefreshCw, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { ApprovalBadge } from "@/components/approvals/ApprovalBadge";
import { ApprovalDetail } from "@/components/approvals/ApprovalDetail";
import type { BulkAction, ColumnDef } from "@/components/common";
import { DataTable, DrawerProvider, useDrawer } from "@/components/common";
import { useApprovals } from "@/hooks/useApprovals";
import type { PendingApproval } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function minutesWaiting(createdAt: string): number {
	return Math.floor((Date.now() - new Date(createdAt).getTime()) / 60_000);
}

function urgencyColor(minutes: number): string {
	if (minutes >= 10) return "text-th-status-error-text";
	if (minutes >= 5) return "text-th-status-warning-text";
	return "text-th-status-success-text";
}

// ---------------------------------------------------------------------------
// Inner page (needs drawer context)
// ---------------------------------------------------------------------------

function ApprovalQueueInner() {
	const {
		approvals,
		totalPendingCount,
		loading,
		error,
		agentMap,
		busyIds,
		refetch,
		approve,
		deny,
		bulkApprove,
		bulkDeny,
	} = useApprovals({ browserNotifications: true });

	const { openDrawer, closeDrawer } = useDrawer();

	const [selectedIds, setSelectedIds] = useState<string[]>([]);
	const [filterAgentId, setFilterAgentId] = useState<string>("");

	// Keep selectedIds clean when approvals list changes
	useEffect(() => {
		const ids = new Set(approvals.map((a) => a.id));
		setSelectedIds((prev) => prev.filter((id) => ids.has(id)));
	}, [approvals]);

	// Filtered approvals
	const visible = filterAgentId
		? approvals.filter((a) => a.agent_id === filterAgentId)
		: approvals;

	const someSelected = selectedIds.length > 0;

	// Keyboard shortcuts
	useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			const tag = (e.target as HTMLElement).tagName;
			if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
			if (e.metaKey || e.ctrlKey || e.altKey) return;

			if (e.key === "a" || e.key === "A") {
				e.preventDefault();
				if (someSelected) bulkApprove(selectedIds);
			}
			if (e.key === "d" || e.key === "D") {
				e.preventDefault();
				if (someSelected) bulkDeny(selectedIds);
			}
		};
		document.addEventListener("keydown", handler);
		return () => document.removeEventListener("keydown", handler);
	}, [someSelected, selectedIds, bulkApprove, bulkDeny]);

	// Row click → open drawer
	const handleRowClick = useCallback(
		(approval: PendingApproval) => {
			openDrawer(
				approval.tool_name,
				<ApprovalDetail
					approval={approval}
					agentName={agentMap.get(approval.agent_id)?.name}
					busy={busyIds.has(approval.id)}
					onApprove={(id) => {
						approve(id);
						closeDrawer();
					}}
					onDeny={(id) => {
						deny(id);
						closeDrawer();
					}}
				/>,
			);
		},
		[agentMap, busyIds, approve, deny, openDrawer, closeDrawer],
	);

	// Unique agents for filter dropdown
	const agentOptions = [
		...new Map(
			approvals.map((a) => [a.agent_id, agentMap.get(a.agent_id)]),
		).entries(),
	];

	// Bulk actions
	const bulkActions: BulkAction[] = [
		{
			label: "Approve selected (A)",
			icon: <Check size={12} />,
			onClick: () => bulkApprove(selectedIds),
			variant: "success",
		},
		{
			label: "Deny selected (D)",
			icon: <X size={12} />,
			onClick: () => bulkDeny(selectedIds),
			variant: "danger",
		},
	];

	// Column definitions
	const columns: ColumnDef<PendingApproval>[] = [
		{
			key: "tool_name",
			header: "Tool",
			render: (a) => (
				<span className="font-mono text-sm font-semibold text-th-text">
					{a.tool_name}
				</span>
			),
		},
		{
			key: "agent",
			header: "Agent",
			render: (a) => {
				const agent = agentMap.get(a.agent_id);
				return (
					<span className="text-sm text-th-text-secondary">
						{agent?.name ?? a.agent_id}
					</span>
				);
			},
		},
		{
			key: "urgency",
			header: "Wait Time",
			render: (a) => {
				const mins = minutesWaiting(a.created_at);
				return (
					<span
						className={["text-sm font-medium", urgencyColor(mins)].join(" ")}
					>
						{mins < 1 ? "Just now" : `${mins}m ago`}
					</span>
				);
			},
		},
		{
			key: "status",
			header: "Status",
			render: (a) => (
				<span className="text-sm capitalize text-th-text-muted">
					{a.status}
				</span>
			),
		},
		{
			key: "created_at",
			header: "Created",
			render: (a) => (
				<span className="text-sm text-th-text-muted whitespace-nowrap">
					{new Date(a.created_at).toLocaleString()}
				</span>
			),
		},
		{
			key: "actions",
			header: "",
			render: (a) => (
				<div
					className="flex items-center gap-1"
					onClick={(e) => e.stopPropagation()}
				>
					<button
						type="button"
						disabled={busyIds.has(a.id)}
						onClick={() => approve(a.id)}
						className="rounded-md px-2.5 py-1 text-xs font-medium bg-th-status-success-dot text-th-accent-text hover:opacity-90 disabled:opacity-50 transition-colors"
					>
						Approve
					</button>
					<button
						type="button"
						disabled={busyIds.has(a.id)}
						onClick={() => deny(a.id)}
						className="rounded-md px-2.5 py-1 text-xs font-medium bg-th-status-error-dot text-th-accent-text hover:opacity-90 disabled:opacity-50 transition-colors"
					>
						Deny
					</button>
				</div>
			),
		},
	];

	return (
		<div className="space-y-5">
			{/* Page header */}
			<div className="flex items-center justify-between">
				<div className="flex items-center gap-3">
					<h1 className="text-2xl font-semibold text-th-text">
						Approval Queue
					</h1>
					<ApprovalBadge count={totalPendingCount} showZero />
				</div>

				<div className="flex items-center gap-2">
					{/* Filter by agent */}
					{agentOptions.length > 1 && (
						<select
							aria-label="Filter by agent"
							value={filterAgentId}
							onChange={(e) => setFilterAgentId(e.target.value)}
							className="rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-sm text-th-text focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
						>
							<option value="">All agents</option>
							{agentOptions.map(([id, agent]) => (
								<option key={id} value={id}>
									{agent?.name ?? id}
								</option>
							))}
						</select>
					)}

					{/* Refresh */}
					<button
						type="button"
						onClick={refetch}
						aria-label="Refresh approvals"
						className="rounded-md border border-th-border-strong bg-th-surface p-2 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text-secondary transition-colors"
					>
						<RefreshCw size={16} />
					</button>
				</div>
			</div>

			{/* Error banner */}
			{error && (
				<div
					role="alert"
					className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text"
				>
					{error}
				</div>
			)}

			{/* Table */}
			<DataTable
				columns={columns}
				data={visible}
				rowKey={(a) => a.id}
				loading={loading}
				onRowClick={handleRowClick}
				emptyTitle="No pending approvals"
				emptyDescription="All caught up!"
				selectable
				selectedIds={selectedIds}
				onSelectChange={setSelectedIds}
				bulkActions={bulkActions}
			/>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Exported page (wraps with DrawerProvider)
// ---------------------------------------------------------------------------

export function ApprovalQueuePage() {
	return (
		<DrawerProvider>
			<ApprovalQueueInner />
		</DrawerProvider>
	);
}

export default ApprovalQueuePage;
