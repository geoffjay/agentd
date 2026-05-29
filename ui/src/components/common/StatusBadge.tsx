/**
 * StatusBadge — coloured pill/dot for entity status values.
 */

import type { NotificationStatus } from "@/types/notify";
import type { AgentStatus } from "@/types/orchestrator";
import type { QuestionStatus } from "@/types/ask";

export type ServiceStatus = "healthy" | "degraded" | "down" | "unknown";

type KnownStatus =
	| AgentStatus
	| NotificationStatus
	| ServiceStatus
	| QuestionStatus;

interface StatusBadgeProps {
	status: KnownStatus;
	/** 'badge' renders a pill with text; 'dot' renders a coloured circle only */
	variant?: "badge" | "dot";
	className?: string;
}

const STATUS_STYLES: Record<string, string> = {
	// Agent statuses
	running: "bg-th-status-success-bg text-th-status-success-text",
	pending: "bg-th-status-warning-bg text-th-status-warning-text",
	stopped: "bg-th-surface-sunken text-th-text-muted",
	failed: "bg-th-status-error-bg text-th-status-error-text",
	// Notification statuses
	viewed: "bg-th-status-info-bg text-th-status-info-text",
	responded: "bg-th-status-success-bg text-th-status-success-text",
	dismissed: "bg-th-surface-sunken text-th-text-muted",
	expired: "bg-th-surface-sunken text-th-text-muted",
	// Service health
	healthy: "bg-th-status-success-bg text-th-status-success-text",
	degraded: "bg-th-status-warning-bg text-th-status-warning-text",
	down: "bg-th-status-error-bg text-th-status-error-text",
	unknown: "bg-th-surface-sunken text-th-text-muted",
};

const DOT_STYLES: Record<string, string> = {
	running: "bg-th-status-success-dot",
	pending: "bg-th-status-warning-dot",
	stopped: "bg-th-text-muted",
	failed: "bg-th-status-error-dot",
	viewed: "bg-th-status-info-dot",
	responded: "bg-th-status-success-dot",
	dismissed: "bg-th-text-muted",
	expired: "bg-th-text-muted",
	healthy: "bg-th-status-success-dot",
	degraded: "bg-th-status-warning-dot",
	down: "bg-th-status-error-dot",
	unknown: "bg-th-text-muted",
};

export function StatusBadge({
	status,
	variant = "badge",
	className = "",
}: StatusBadgeProps) {
	if (variant === "dot") {
		return (
			<span
				role="status"
				aria-label={status}
				className={[
					"inline-block h-2.5 w-2.5 rounded-full",
					DOT_STYLES[status] ?? "bg-th-text-muted",
					className,
				].join(" ")}
			/>
		);
	}

	return (
		<span
			role="status"
			className={[
				"inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium",
				STATUS_STYLES[status] ?? "bg-th-surface-sunken text-th-text-muted",
				className,
			].join(" ")}
		>
			{status}
		</span>
	);
}

export default StatusBadge;
