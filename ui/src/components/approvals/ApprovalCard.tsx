/**
 * ApprovalCard — displays a single pending approval with urgency indicator,
 * expandable tool input, approve/deny actions, and selection checkbox.
 */

import { ChevronDown, ChevronRight, Clock } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import type { PendingApproval } from "@/types/orchestrator";
import { ApprovalActions } from "./ApprovalActions";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function minutesWaiting(createdAt: string): number {
	return Math.floor((Date.now() - new Date(createdAt).getTime()) / 60_000);
}

function urgencyClass(minutes: number): string {
	if (minutes >= 10) return "border-l-th-status-error-dot";
	if (minutes >= 5) return "border-l-th-status-warning-dot";
	return "border-l-th-status-success-dot";
}

function urgencyLabel(minutes: number): string {
	if (minutes >= 10) return "High urgency";
	if (minutes >= 5) return "Medium urgency";
	return "Low urgency";
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface ApprovalCardProps {
	approval: PendingApproval;
	agentName?: string;
	busy?: boolean;
	selected?: boolean;
	onApprove: (id: string) => void;
	onDeny: (id: string) => void;
	onToggleSelect?: (id: string) => void;
}

export function ApprovalCard({
	approval,
	agentName,
	busy = false,
	selected = false,
	onApprove,
	onDeny,
	onToggleSelect,
}: ApprovalCardProps) {
	const [expanded, setExpanded] = useState(false);
	const minutes = minutesWaiting(approval.created_at);
	const urgency = urgencyClass(minutes);

	return (
		<article
			aria-label={`Approval request for ${approval.tool_name}`}
			className={[
				"rounded-lg border border-th-border bg-th-surface border-l-4",
				urgency,
				selected ? "ring-2 ring-th-focus-ring" : "",
			]
				.filter(Boolean)
				.join(" ")}
		>
			{/* Header row */}
			<div className="flex items-start gap-3 p-4">
				{/* Selection checkbox */}
				{onToggleSelect && (
					<input
						type="checkbox"
						aria-label={`Select approval for ${approval.tool_name}`}
						checked={selected}
						onChange={() => onToggleSelect(approval.id)}
						className="mt-0.5 h-4 w-4 rounded border-th-border bg-th-input text-th-accent focus:ring-th-focus-ring"
					/>
				)}

				{/* Main content */}
				<div className="min-w-0 flex-1">
					<div className="flex flex-wrap items-center gap-2">
						{/* Tool name */}
						<span className="font-mono text-sm font-semibold text-th-text">
							{approval.tool_name}
						</span>

						{/* Agent link */}
						{agentName && (
							<Link
								to={`/agents/${approval.agent_id}`}
								className="text-xs text-th-text-link hover:opacity-90"
							>
								{agentName}
							</Link>
						)}

						{/* Urgency + wait time */}
						<span
							aria-label={urgencyLabel(minutes)}
							className="ml-auto flex items-center gap-1 text-xs text-th-text-muted"
						>
							<Clock size={12} aria-hidden="true" />
							{minutes < 1 ? "just now" : `${minutes}m ago`}
						</span>
					</div>

					{/* Expand toggle */}
					<button
						type="button"
						aria-expanded={expanded}
						aria-controls={`approval-details-${approval.id}`}
						onClick={() => setExpanded((v) => !v)}
						className="mt-1 flex items-center gap-1 text-xs text-th-text-muted hover:text-th-text-secondary"
					>
						{expanded ? (
							<ChevronDown size={12} aria-hidden="true" />
						) : (
							<ChevronRight size={12} aria-hidden="true" />
						)}
						{expanded ? "Hide details" : "Show details"}
					</button>

					{/* Expandable tool input */}
					{expanded && (
						<pre
							id={`approval-details-${approval.id}`}
							className="mt-2 max-h-48 overflow-auto rounded bg-th-surface-sunken p-3 text-xs text-th-text-secondary"
						>
							{JSON.stringify(approval.tool_input, null, 2)}
						</pre>
					)}
				</div>

				{/* Actions */}
				<ApprovalActions
					approvalId={approval.id}
					busy={busy}
					onApprove={onApprove}
					onDeny={onDeny}
					size="sm"
				/>
			</div>
		</article>
	);
}

export default ApprovalCard;
