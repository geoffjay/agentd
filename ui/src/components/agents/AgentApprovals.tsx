/**
 * AgentApprovals — list of pending tool approval requests for this agent.
 *
 * Shows:
 * - Tool name, request time, expiry
 * - Tool input (collapsed by default)
 * - Approve / Deny buttons per request
 * - Count badge in section header
 * - Loading / empty / error states
 */

import { Check, ChevronDown, ChevronRight, X } from "lucide-react";
import { useState } from "react";
import { ListItemSkeleton } from "@/components/common/LoadingSkeleton";
import type { PendingApproval } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// ApprovalRow
// ---------------------------------------------------------------------------

interface ApprovalRowProps {
	approval: PendingApproval;
	onApprove: (id: string) => Promise<void>;
	onDeny: (id: string) => Promise<void>;
}

function ApprovalRow({ approval, onApprove, onDeny }: ApprovalRowProps) {
	const [expanded, setExpanded] = useState(false);
	const [approving, setApproving] = useState(false);
	const [denying, setDenying] = useState(false);
	const [error, setError] = useState<string | undefined>();

	const busy = approving || denying;

	const requestedAt = new Date(approval.created_at).toLocaleString();
	const expiresAt = new Date(approval.expires_at).toLocaleString();

	async function handleApprove() {
		setError(undefined);
		setApproving(true);
		try {
			await onApprove(approval.id);
		} catch (err) {
			setError(err instanceof Error ? err.message : "Approve failed");
		} finally {
			setApproving(false);
		}
	}

	async function handleDeny() {
		setError(undefined);
		setDenying(true);
		try {
			await onDeny(approval.id);
		} catch (err) {
			setError(err instanceof Error ? err.message : "Deny failed");
		} finally {
			setDenying(false);
		}
	}

	return (
		<li className="flex flex-col gap-2 rounded-lg border border-th-border p-3">
			{/* Header row */}
			<div className="flex items-start justify-between gap-3">
				<div className="flex flex-col gap-0.5">
					<span className="text-sm font-medium text-th-text">
						{approval.tool_name}
					</span>
					<span className="text-xs text-th-text-muted">
						Requested: {requestedAt}
					</span>
					<span className="text-xs text-th-text-muted">
						Expires: {expiresAt}
					</span>
				</div>

				{/* Actions */}
				<div className="flex flex-shrink-0 items-center gap-2">
					<button
						type="button"
						aria-label={`Approve ${approval.tool_name}`}
						onClick={handleApprove}
						disabled={busy}
						className="flex items-center gap-1 rounded-md bg-th-status-success-dot px-2.5 py-1 text-xs font-medium text-th-accent-text hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-th-focus-ring disabled:opacity-50"
					>
						<Check size={12} aria-hidden="true" />
						{approving ? "Approving…" : "Approve"}
					</button>
					<button
						type="button"
						aria-label={`Deny ${approval.tool_name}`}
						onClick={handleDeny}
						disabled={busy}
						className="flex items-center gap-1 rounded-md bg-th-status-error-dot px-2.5 py-1 text-xs font-medium text-th-accent-text hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-th-focus-ring disabled:opacity-50"
					>
						<X size={12} aria-hidden="true" />
						{denying ? "Denying…" : "Deny"}
					</button>
				</div>
			</div>

			{/* Tool input toggle */}
			{approval.tool_input !== undefined && (
				<div>
					<button
						type="button"
						aria-expanded={expanded}
						onClick={() => setExpanded((e) => !e)}
						className="flex items-center gap-1 text-xs text-th-text-muted hover:text-th-text-secondary"
					>
						{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
						Tool input
					</button>
					{expanded && (
						<pre className="mt-1 overflow-x-auto rounded bg-th-surface-sunken p-2 text-xs text-th-text-secondary">
							{JSON.stringify(approval.tool_input, null, 2)}
						</pre>
					)}
				</div>
			)}

			{/* Error */}
			{error && (
				<p role="alert" className="text-xs text-th-status-error-text">
					{error}
				</p>
			)}
		</li>
	);
}

// ---------------------------------------------------------------------------
// AgentApprovals
// ---------------------------------------------------------------------------

export interface AgentApprovalsProps {
	approvals: PendingApproval[];
	loading: boolean;
	error?: string;
	onApprove: (id: string) => Promise<void>;
	onDeny: (id: string) => Promise<void>;
}

export function AgentApprovals({
	approvals,
	loading,
	error,
	onApprove,
	onDeny,
}: AgentApprovalsProps) {
	return (
		<section aria-label="Pending approvals">
			<div className="mb-3 flex items-center gap-2">
				<h3 className="text-sm font-medium text-th-text">Pending Approvals</h3>
				{approvals.length > 0 && (
					<span className="rounded-full bg-th-status-warning-bg px-2 py-0.5 text-xs font-medium text-th-status-warning-text">
						{approvals.length}
					</span>
				)}
			</div>

			{loading ? (
				<ListItemSkeleton rows={2} />
			) : error ? (
				<p role="alert" className="text-sm text-th-status-error-text">
					{error}
				</p>
			) : approvals.length === 0 ? (
				<p className="text-sm text-th-text-muted">No pending approvals.</p>
			) : (
				<ul className="flex flex-col gap-2" aria-label="Approval requests">
					{approvals.map((a) => (
						<ApprovalRow
							key={a.id}
							approval={a}
							onApprove={onApprove}
							onDeny={onDeny}
						/>
					))}
				</ul>
			)}
		</section>
	);
}

export default AgentApprovals;
