/**
 * ActivityTimeline — recent activity feed combining agent state changes,
 * notifications, and questions.
 *
 * For the initial release this component accepts pre-fetched data as props
 * (passed down from the Dashboard page) rather than fetching independently.
 */

import { Bell, Bot, ExternalLink, HelpCircle } from "lucide-react";
import { Link } from "react-router-dom";
import { ListItemSkeleton } from "@/components/common/LoadingSkeleton";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ActivityEventType = "agent" | "notification" | "question";

export interface ActivityEvent {
	id: string;
	type: ActivityEventType;
	description: string;
	timestamp: Date;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatRelativeTime(date: Date): string {
	const diffMs = Date.now() - date.getTime();
	const diffSec = Math.floor(diffMs / 1000);
	if (diffSec < 60) return "just now";
	const diffMin = Math.floor(diffSec / 60);
	if (diffMin < 60) return `${diffMin} min ago`;
	const diffHr = Math.floor(diffMin / 60);
	if (diffHr < 24) return `${diffHr}h ago`;
	return `${Math.floor(diffHr / 24)}d ago`;
}

const EVENT_ICONS: Record<ActivityEventType, React.ReactNode> = {
	agent: <Bot size={16} className="text-th-text-link" />,
	notification: <Bell size={16} className="text-th-status-warning-text" />,
	question: <HelpCircle size={16} className="text-th-status-info-text" />,
};

const EVENT_BG: Record<ActivityEventType, string> = {
	agent: "bg-th-accent-subtle",
	notification: "bg-th-status-warning-bg",
	question: "bg-th-status-info-bg",
};

// ---------------------------------------------------------------------------
// ActivityTimeline
// ---------------------------------------------------------------------------

interface ActivityTimelineProps {
	events: ActivityEvent[];
	loading?: boolean;
	error?: string;
}

export function ActivityTimeline({
	events,
	loading = false,
	error,
}: ActivityTimelineProps) {
	return (
		<section
			aria-labelledby="activity-timeline-heading"
			className="rounded-lg border border-th-border bg-th-surface p-5"
		>
			{/* Header */}
			<div className="flex items-center justify-between">
				<h2
					id="activity-timeline-heading"
					className="text-base font-semibold text-th-text"
				>
					Recent Activity
				</h2>
				<Link
					to="/notifications"
					className="flex items-center gap-1 text-xs font-medium text-th-text-link hover:opacity-80"
				>
					View All <ExternalLink size={12} />
				</Link>
			</div>

			{/* Error */}
			{error && (
				<p className="mt-3 text-sm text-th-status-error-text">{error}</p>
			)}

			{/* Loading */}
			{loading && !error && (
				<div className="mt-4">
					<ListItemSkeleton rows={5} />
				</div>
			)}

			{/* Empty state */}
			{!loading && !error && events.length === 0 && (
				<p className="mt-4 text-sm text-th-text-muted">No recent activity.</p>
			)}

			{/* Event list */}
			{!loading && !error && events.length > 0 && (
				<ol role="list" aria-label="Activity feed" className="mt-4 space-y-3">
					{events.slice(0, 10).map((event) => (
						<li key={event.id} className="flex items-start gap-3">
							{/* Icon bubble */}
							<div
								aria-hidden="true"
								className={[
									"mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full",
									EVENT_BG[event.type],
								].join(" ")}
							>
								{EVENT_ICONS[event.type]}
							</div>
							{/* Text */}
							<div className="min-w-0 flex-1">
								<p className="text-sm text-th-text-secondary">
									{event.description}
								</p>
								<time
									dateTime={event.timestamp.toISOString()}
									className="text-xs text-th-text-faint"
								>
									{formatRelativeTime(event.timestamp)}
								</time>
							</div>
						</li>
					))}
				</ol>
			)}
		</section>
	);
}

export default ActivityTimeline;
