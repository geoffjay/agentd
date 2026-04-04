/**
 * ApprovalDetail — drawer content for a single approval.
 *
 * Shows all approval details including the full tool input JSON,
 * urgency indicator, agent link, and approve/deny actions.
 */

import { Clock } from "lucide-react";
import { Link } from "react-router-dom";
import type { PendingApproval } from "@/types/orchestrator";
import { ApprovalActions } from "./ApprovalActions";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function minutesWaiting(createdAt: string): number {
	return Math.floor((Date.now() - new Date(createdAt).getTime()) / 60_000);
}

function urgencyLabel(minutes: number): string {
	if (minutes >= 10) return "High urgency";
	if (minutes >= 5) return "Medium urgency";
	return "Low urgency";
}

function urgencyColor(minutes: number): string {
	if (minutes >= 10) return "text-th-status-error-text";
	if (minutes >= 5) return "text-th-status-warning-text";
	return "text-th-status-success-text";
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface ApprovalDetailProps {
	approval: PendingApproval;
	agentName?: string;
	busy?: boolean;
	onApprove: (id: string) => void;
	onDeny: (id: string) => void;
}

export function ApprovalDetail({
	approval,
	agentName,
	busy = false,
	onApprove,
	onDeny,
}: ApprovalDetailProps) {
	const minutes = minutesWaiting(approval.created_at);

	return (
		<div className="space-y-5">
			{/* Tool name */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Tool
				</h3>
				<p className="mt-1 font-mono text-sm font-semibold text-th-text">
					{approval.tool_name}
				</p>
			</div>

			{/* Agent */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Agent
				</h3>
				<p className="mt-1">
					{agentName ? (
						<Link
							to={`/agents/${approval.agent_id}`}
							className="text-sm text-th-text-link hover:opacity-80"
						>
							{agentName}
						</Link>
					) : (
						<span className="text-sm text-th-text-muted">
							{approval.agent_id}
						</span>
					)}
				</p>
			</div>

			{/* Urgency & timing */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Wait Time
				</h3>
				<div className="mt-1 flex items-center gap-2">
					<Clock size={14} className={urgencyColor(minutes)} />
					<span
						className={["text-sm font-medium", urgencyColor(minutes)].join(" ")}
					>
						{minutes < 1 ? "Just now" : `${minutes}m ago`}
					</span>
					<span className="text-xs text-th-text-muted">
						({urgencyLabel(minutes)})
					</span>
				</div>
			</div>

			{/* Status */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Status
				</h3>
				<p className="mt-1 text-sm capitalize text-th-text-secondary">
					{approval.status}
				</p>
			</div>

			{/* Created / Expires */}
			<div className="grid grid-cols-2 gap-4">
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Created
					</h3>
					<p className="mt-1 text-sm text-th-text-secondary">
						{new Date(approval.created_at).toLocaleString()}
					</p>
				</div>
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Expires
					</h3>
					<p className="mt-1 text-sm text-th-text-secondary">
						{new Date(approval.expires_at).toLocaleString()}
					</p>
				</div>
			</div>

			{/* Tool input */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Tool Input
				</h3>
				<pre className="mt-2 max-h-72 overflow-auto rounded-lg bg-th-surface-sunken p-4 text-xs text-th-text-secondary">
					{JSON.stringify(approval.tool_input, null, 2)}
				</pre>
			</div>

			{/* Actions */}
			{approval.status === "pending" && (
				<div className="border-t border-th-border pt-4">
					<ApprovalActions
						approvalId={approval.id}
						busy={busy}
						onApprove={onApprove}
						onDeny={onDeny}
						size="md"
					/>
				</div>
			)}
		</div>
	);
}

export default ApprovalDetail;
