/**
 * NotificationDetail — drawer content for a single notification.
 *
 * Shows all notification details including the full message,
 * source, priority, lifetime, response, and action buttons.
 */

import { Clock, Infinity as InfinityIcon, Timer } from "lucide-react";
import { StatusBadge } from "@/components/common/StatusBadge";
import type { Notification } from "@/types/notify";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SOURCE_LABELS: Record<string, string> = {
	system: "System",
	ask_service: "Ask",
	agent_hook: "Agent Hook",
	monitor_service: "Monitor",
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

export interface NotificationDetailProps {
	notification: Notification;
	busy?: boolean;
	onView: (id: string) => void;
	onRespond: (notification: Notification) => void;
	onDismiss: (id: string) => void;
	onDelete: (id: string) => void;
}

export function NotificationDetail({
	notification,
	busy = false,
	onView,
	onRespond,
	onDismiss,
	onDelete,
}: NotificationDetailProps) {
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
		<div className="space-y-5">
			{/* Title */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Title
				</h3>
				<p className="mt-1 text-sm font-semibold text-th-text">
					{notification.title}
				</p>
			</div>

			{/* Status & Priority */}
			<div className="flex items-center gap-3">
				<StatusBadge status={notification.status} />
				<span className="rounded-full bg-th-surface-sunken px-2.5 py-0.5 text-xs font-medium capitalize text-th-text-muted">
					{notification.priority}
				</span>
			</div>

			{/* Source */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Source
				</h3>
				<p className="mt-1 text-sm text-th-text-secondary">
					{SOURCE_LABELS[notification.source.type] ?? notification.source.type}
				</p>
			</div>

			{/* Timing */}
			<div className="grid grid-cols-2 gap-4">
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Created
					</h3>
					<div className="mt-1 flex items-center gap-1.5 text-sm text-th-text-secondary">
						<Clock size={13} className="text-th-text-muted" />
						{formatRelativeTime(notification.created_at)}
					</div>
				</div>
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Lifetime
					</h3>
					<div className="mt-1 flex items-center gap-1.5 text-sm">
						{isEphemeral && expiresAt ? (
							<span className="flex items-center gap-1 text-th-status-warning-text">
								<Timer size={13} />
								{formatCountdown(expiresAt)}
							</span>
						) : (
							<span className="flex items-center gap-1 text-th-text-muted">
								<InfinityIcon size={13} />
								Persistent
							</span>
						)}
					</div>
				</div>
			</div>

			{/* Message */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Message
				</h3>
				<div className="mt-2 rounded-lg bg-th-surface-sunken p-4 text-sm text-th-text-secondary whitespace-pre-wrap">
					{notification.message}
				</div>
			</div>

			{/* Response (if responded) */}
			{notification.status === "responded" && notification.response && (
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Response
					</h3>
					<div className="mt-2 rounded-lg bg-th-status-success-bg p-4 text-sm text-th-status-success-text">
						{notification.response}
					</div>
				</div>
			)}

			{/* Actions */}
			{!isDone && (
				<div className="border-t border-th-border pt-4">
					<div className="flex flex-wrap gap-2">
						{notification.status === "pending" && (
							<button
								type="button"
								disabled={busy}
								onClick={() => onView(notification.id)}
								className="rounded-md px-3 py-1.5 text-xs font-medium bg-th-surface-sunken text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50 transition-colors"
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
									className="rounded-md px-3 py-1.5 text-xs font-medium bg-th-accent text-th-accent-text hover:bg-th-accent-hover disabled:opacity-50 transition-colors"
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
								className="rounded-md px-3 py-1.5 text-xs font-medium bg-th-surface-sunken text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50 transition-colors"
							>
								Dismiss
							</button>
						)}

						<button
							type="button"
							disabled={busy}
							onClick={() => onDelete(notification.id)}
							className="rounded-md px-3 py-1.5 text-xs font-medium bg-th-status-error-bg text-th-status-error-text hover:opacity-90 disabled:opacity-50 transition-colors"
						>
							Delete
						</button>
					</div>
				</div>
			)}
		</div>
	);
}

export default NotificationDetail;
