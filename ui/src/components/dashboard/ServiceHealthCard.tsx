/**
 * ServiceHealthCard — shows a single service's health status, version, and port.
 */

import { Activity, Server } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { CardSkeleton } from "@/components/common/LoadingSkeleton";
import { StatusBadge } from "@/components/common/StatusBadge";
import type { ServiceHealth } from "@/hooks/useServiceHealth";

const SERVICE_ROUTES: Record<string, string> = {
	orchestrator: "/agents",
	notify: "/notifications",
	ask: "/questions",
};

interface ServiceHealthCardProps {
	service: ServiceHealth;
}

export function ServiceHealthCard({ service }: ServiceHealthCardProps) {
	const navigate = useNavigate();

	function handleClick() {
		const route = SERVICE_ROUTES[service.key];
		if (route) navigate(route);
	}

	function handleKeyDown(e: React.KeyboardEvent) {
		if (e.key === "Enter" || e.key === " ") {
			e.preventDefault();
			handleClick();
		}
	}

	return (
		<div
			role="button"
			tabIndex={0}
			aria-label={`${service.name} service — ${service.status}`}
			onClick={handleClick}
			onKeyDown={handleKeyDown}
			className="cursor-pointer rounded-lg border border-th-border bg-th-surface p-5 shadow-sm transition-shadow hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus:ring-th-focus-ring"
		>
			{/* Header row */}
			<div className="flex items-start justify-between">
				<div className="flex items-center gap-3">
					<div className="flex h-10 w-10 items-center justify-center rounded-full bg-th-accent-subtle">
						<Server
							size={20}
							className="text-th-text-link"
						/>
					</div>
					<div>
						<p className="font-semibold text-th-text">
							{service.name}
						</p>
						<p className="text-xs text-th-text-muted">
							Port {service.port}
						</p>
					</div>
				</div>
				<StatusBadge status={service.status} />
			</div>

			{/* Version + last checked */}
			<div className="mt-4 flex items-center justify-between text-xs text-th-text-muted">
				<span className="flex items-center gap-1">
					<Activity size={12} />
					{service.version ? `v${service.version}` : "—"}
				</span>
				{service.lastChecked && (
					<span>Checked {formatRelativeTime(service.lastChecked)}</span>
				)}
			</div>

			{/* Error message */}
			{service.error && (
				<p className="mt-2 text-xs text-th-status-error-text">
					{service.error}
				</p>
			)}
		</div>
	);
}

/** Loading placeholder matching the card's dimensions */
export function ServiceHealthCardSkeleton() {
	return <CardSkeleton />;
}

/** Format a Date as a short relative time string */
function formatRelativeTime(date: Date): string {
	const diffMs = Date.now() - date.getTime();
	const diffSec = Math.floor(diffMs / 1000);
	if (diffSec < 60) return "just now";
	const diffMin = Math.floor(diffSec / 60);
	if (diffMin < 60) return `${diffMin}m ago`;
	const diffHr = Math.floor(diffMin / 60);
	return `${diffHr}h ago`;
}

export default ServiceHealthCard;
