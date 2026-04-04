/**
 * ServiceBanner — banner shown at the top of a page when a required service
 * is unavailable or degraded.
 *
 * Shows:
 * - Warning/error colour coded by severity
 * - Service name and status message
 * - "Retry" button to trigger an immediate health refresh
 * - "Stale data" indicator when data is being shown from cache
 */

import { AlertCircle, AlertTriangle, RefreshCw, WifiOff } from "lucide-react";
import type { ServiceHealth } from "@/hooks/useServiceHealth";

export interface ServiceBannerProps {
	service: ServiceHealth;
	onRetry?: () => void;
	showStaleIndicator?: boolean;
	className?: string;
}

export function ServiceBanner({
	service,
	onRetry,
	showStaleIndicator = false,
	className = "",
}: ServiceBannerProps) {
	if (service.status === "healthy") return null;

	const isDown = service.status === "down";

	return (
		<div
			role="alert"
			className={[
				"flex flex-wrap items-center gap-3 rounded-lg border px-4 py-3 text-sm",
				isDown
					? "border-th-status-error-border bg-th-status-error-bg text-th-status-error-text"
					: "border-th-status-warning-border bg-th-status-warning-bg text-th-status-warning-text",
				className,
			].join(" ")}
		>
			{/* Icon */}
			{isDown ? (
				<WifiOff size={16} aria-hidden="true" className="shrink-0" />
			) : (
				<AlertTriangle size={16} aria-hidden="true" className="shrink-0" />
			)}

			{/* Message */}
			<div className="flex-1 min-w-0">
				<span className="font-medium">{service.name} service </span>
				{isDown ? (
					<span>is unavailable — some features may not work</span>
				) : (
					<span>is degraded — performance may be impacted</span>
				)}
				{showStaleIndicator && (
					<span className="ml-2 inline-flex items-center gap-1 rounded-full bg-th-status-warning-bg px-2 py-0.5 text-[10px] font-medium text-th-status-warning-text">
						<AlertCircle size={10} aria-hidden="true" />
						Showing stale data
					</span>
				)}
			</div>

			{/* Retry */}
			{onRetry && (
				<button
					type="button"
					onClick={onRetry}
					className={[
						"flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium transition-colors",
						isDown
							? "bg-th-status-error-bg text-th-status-error-text hover:opacity-90"
							: "bg-th-status-warning-bg text-th-status-warning-text hover:opacity-90",
					].join(" ")}
				>
					<RefreshCw size={12} aria-hidden="true" />
					Retry
				</button>
			)}
		</div>
	);
}

/**
 * GlobalServiceBanner — shows banners for ALL degraded/down services.
 * Suitable for use at the top of a page that depends on multiple services.
 */
export interface GlobalServiceBannerProps {
	services: ServiceHealth[];
	onRetry?: () => void;
	showStaleIndicator?: boolean;
}

export function GlobalServiceBanner({
	services,
	onRetry,
	showStaleIndicator = false,
}: GlobalServiceBannerProps) {
	const degraded = services.filter((s) => s.status !== "healthy");
	if (degraded.length === 0) return null;

	return (
		<div className="flex flex-col gap-2">
			{degraded.map((s) => (
				<ServiceBanner
					key={s.key}
					service={s}
					onRetry={onRetry}
					showStaleIndicator={showStaleIndicator}
				/>
			))}
		</div>
	);
}

export default ServiceBanner;
