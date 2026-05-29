/**
 * NotificationCard — displays a single notification with:
 * - Priority-coded left border (blue/green/orange/red)
 * - Title and message with expand/collapse
 * - Source badge, status badge, timestamp, lifetime indicator
 * - Action buttons: View, Respond, Dismiss, Delete
 * - Selection checkbox for bulk operations
 */

import {
	ChevronDown,
	ChevronRight,
	Clock,
	Infinity as InfinityIcon,
	Timer,
} from "lucide-react";
import { useState } from "react";
import { StatusBadge } from "@/components/common/StatusBadge";
import type { Notification } from "@/types/notify";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const PRIORITY_BORDER: Record<string, string> = {
	low: "border-l-th-status-info-dot",
	normal: "border-l-th-status-success-dot",
	high: "border-l-th-status-warning-dot",
	urgent: "border-l-th-status-error-dot",
};

const SOURCE_LABELS: Record<string, string> = {
	system: "System",
	ask_service: "Ask",
	agent_hook: "Agent Hook",
	monitor_service: "Monitor",
};

const SOURCE_COLORS: Record<string, string> = {
	system: "bg-th-surface-sunken text-th-text-secondary",
	ask_service: "bg-th-status-info-bg text-th-status-info-text",
	agent_hook: "bg-th-status-info-bg text-th-status-info-text",
	monitor_service: "bg-th-status-info-bg text-th-status-info-text",
};

function formatRelativeTime(dateStr: string): string {
	const diffMs = Date.now() - new Date(dateStr).getTime();
	const diffSec = Math.floor(diffMs / 1000);
	if (diffSec < 60) return "just now";
	const diffMin = Math.floor(diffSec / 60);
	if (diffMin < 60) return `${diffMin} min ago`;
	const diffHour = Math.floor(diffMin / 60);
	if (diffHour < 24) return `${diffHour}h ago`;
	const diffDay = Math.floor(diffHour / 24);
	return `${diffDay}d ago`;
}

function formatCountdown(expiresAt: string): string {
	const diffMs = new Date(expiresAt).getTime() - Date.now();
	if (diffMs <= 0) return "Expired";
	const diffMin = Math.floor(diffMs / 60_000);
	if (diffMin < 60) return `${diffMin}m left`;
	const diffHour = Math.floor(diffMin / 60);
	if (diffHour < 24) return `${diffHour}h left`;
	return `${Math.floor(diffHour / 24)}d left`;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface NotificationCardProps {
	notification: Notification;
	busy?: boolean;
	selected?: boolean;
	onView: (id: string) => void;
	onRespond: (notification: Notification) => void;
	onDismiss: (id: string) => void;
	onDelete: (id: string) => void;
	onToggleSelect?: (id: string) => void;
}

export function NotificationCard({
	notification,
	busy = false,
	selected = false,
	onView,
	onRespond,
	onDismiss,
	onDelete,
	onToggleSelect,
}: NotificationCardProps) {
	const [expanded, setExpanded] = useState(false);
	const borderClass =
		PRIORITY_BORDER[notification.priority] ?? "border-l-th-border-strong";
	const isEphemeral = notification.lifetime.type === "ephemeral";
	const expiresAt = isEphemeral
		? (notification.lifetime as { type: "ephemeral"; expires_at: string })
				.expires_at
		: null;
	const isDone =
		notification.status === "dismissed" ||
		notification.status === "expired" ||
		notification.status === "responded";

	return (
		<article
			aria-label={`Notification: ${notification.title}`}
			className={[
				"rounded-lg border border-th-border bg-th-surface border-l-4 transition-opacity",
				borderClass,
				selected ? "ring-2 ring-th-focus-ring" : "",
				isDone ? "opacity-70" : "",
			]
				.filter(Boolean)
				.join(" ")}
		>
			<div className="flex items-start gap-3 p-4">
				{/* Selection checkbox */}
				{onToggleSelect && (
					<input
						type="checkbox"
						aria-label={`Select notification: ${notification.title}`}
						checked={selected}
						onChange={() => onToggleSelect(notification.id)}
						className="mt-0.5 h-4 w-4 rounded border-th-border bg-th-input text-th-accent focus:ring-th-focus-ring shrink-0"
					/>
				)}

				{/* Main content */}
				<div className="min-w-0 flex-1">
					{/* Top row: title + badges */}
					<div className="flex flex-wrap items-start gap-2">
						<span className="font-semibold text-th-text text-sm leading-tight flex-1 min-w-0">
							{notification.title}
						</span>

						{/* Source badge */}
						<span
							className={[
								"shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
								SOURCE_COLORS[notification.source.type] ??
									"bg-th-surface-sunken text-th-text-secondary",
							].join(" ")}
						>
							{SOURCE_LABELS[notification.source.type] ??
								notification.source.type}
						</span>

						{/* Status badge */}
						<StatusBadge status={notification.status} />
					</div>

					{/* Meta row: timestamp + lifetime */}
					<div className="mt-1 flex flex-wrap items-center gap-3 text-xs text-th-text-muted">
						<span className="flex items-center gap-1">
							<Clock size={11} aria-hidden="true" />
							{formatRelativeTime(notification.created_at)}
						</span>

						{isEphemeral && expiresAt ? (
							<span className="flex items-center gap-1 text-th-status-warning-text">
								<Timer size={11} aria-hidden="true" />
								{formatCountdown(expiresAt)}
							</span>
						) : (
							<span className="flex items-center gap-1 text-th-text-muted">
								<InfinityIcon size={11} aria-hidden="true" />
								Persistent
							</span>
						)}

						{/* Priority label */}
						<span className="capitalize text-th-text-muted">
							{notification.priority}
						</span>
					</div>

					{/* Expand toggle */}
					<button
						type="button"
						aria-expanded={expanded}
						aria-controls={`notif-msg-${notification.id}`}
						onClick={() => setExpanded((v) => !v)}
						className="mt-1 flex items-center gap-1 text-xs text-th-text-muted hover:text-th-text-secondary"
					>
						{expanded ? (
							<ChevronDown size={12} aria-hidden="true" />
						) : (
							<ChevronRight size={12} aria-hidden="true" />
						)}
						{expanded ? "Hide message" : "Show message"}
					</button>

					{/* Expandable message */}
					{expanded && (
						<p
							id={`notif-msg-${notification.id}`}
							className="mt-2 rounded bg-th-surface-sunken p-3 text-xs text-th-text-secondary whitespace-pre-wrap"
						>
							{notification.message}
						</p>
					)}

					{/* Response (if already responded) */}
					{notification.status === "responded" && notification.response && (
						<div className="mt-2 rounded bg-th-surface-sunken p-2 text-xs text-th-text-muted">
							<span className="font-semibold text-th-text-secondary">
								Response:
							</span>{" "}
							{notification.response}
						</div>
					)}

					{/* Action buttons */}
					<div className="mt-3 flex flex-wrap gap-2">
						{notification.status === "pending" && (
							<button
								type="button"
								disabled={busy}
								onClick={() => onView(notification.id)}
								className="rounded px-2.5 py-1 text-xs font-medium bg-th-surface-sunken text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50 transition-colors"
							>
								Mark Viewed
							</button>
						)}

						{notification.requires_response &&
							(notification.status === "pending" ||
								notification.status === "viewed") && (
								<button
									type="button"
									disabled={busy}
									onClick={() => onRespond(notification)}
									className="rounded px-2.5 py-1 text-xs font-medium bg-th-accent text-th-accent-text hover:bg-th-accent-hover disabled:opacity-50 transition-colors"
								>
									Respond
								</button>
							)}

						{(notification.status === "pending" ||
							notification.status === "viewed") && (
							<button
								type="button"
								disabled={busy}
								onClick={() => onDismiss(notification.id)}
								className="rounded px-2.5 py-1 text-xs font-medium bg-th-surface-sunken text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50 transition-colors"
							>
								Dismiss
							</button>
						)}

						<button
							type="button"
							disabled={busy}
							onClick={() => onDelete(notification.id)}
							className="rounded px-2.5 py-1 text-xs font-medium bg-th-status-error-bg text-th-status-error-text hover:opacity-90 disabled:opacity-50 transition-colors"
						>
							Delete
						</button>
					</div>
				</div>
			</div>
		</article>
	);
}

export default NotificationCard;
