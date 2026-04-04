/**
 * ServiceMetricsCard — shows key metrics for a single service.
 *
 * Displays:
 * - Service name + port + reachability status
 * - Total requests
 * - Error rate (percentage)
 * - Response time (measured from health check)
 */

import { Activity, AlertCircle, CheckCircle2, XCircle } from "lucide-react";
import { CardSkeleton } from "@/components/common/LoadingSkeleton";
import type { ServiceMetricsData } from "@/hooks/useMetrics";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ServiceMetricsCardProps {
	data: ServiceMetricsData;
	responseTimeMs?: number;
	loading?: boolean;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatNumber(n: number): string {
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
	return String(Math.round(n));
}

function errorRateClass(rate: number): string {
	if (rate > 0.1) return "text-th-status-error-text";
	if (rate > 0.01) return "text-th-status-warning-text";
	return "text-th-status-success-text";
}

// ---------------------------------------------------------------------------
// Metric stat
// ---------------------------------------------------------------------------

function Stat({
	label,
	value,
	valueClass = "",
	sub,
}: {
	label: string;
	value: string;
	valueClass?: string;
	sub?: string;
}) {
	return (
		<div>
			<p className="text-xs text-th-text-faint">{label}</p>
			<p
				className={`text-lg font-semibold tabular-nums ${valueClass || "text-th-text"}`}
			>
				{value}
			</p>
			{sub && <p className="text-xs text-th-text-faint">{sub}</p>}
		</div>
	);
}

// ---------------------------------------------------------------------------
// ServiceMetricsCard
// ---------------------------------------------------------------------------

export function ServiceMetricsCard({
	data,
	responseTimeMs,
	loading,
}: ServiceMetricsCardProps) {
	if (loading) return <CardSkeleton />;

	const { name, port, reachable, http } = data;
	const errorPct = (http.errorRate * 100).toFixed(1);

	return (
		<div
			className={[
				"rounded-lg border bg-th-surface p-5 space-y-4",
				reachable
					? "border-th-border"
					: "border-th-status-error-border",
			].join(" ")}
		>
			{/* Header */}
			<div className="flex items-start justify-between">
				<div className="flex items-center gap-2.5">
					<div
						className={[
							"flex h-9 w-9 items-center justify-center rounded-full",
							reachable
								? "bg-th-status-success-bg"
								: "bg-th-status-error-bg",
						].join(" ")}
					>
						<Activity
							size={18}
							className={
								reachable
									? "text-th-status-success-text"
									: "text-th-status-error-text"
							}
						/>
					</div>
					<div>
						<p className="font-semibold text-th-text">
							{name}
						</p>
						<p className="text-xs text-th-text-faint">
							Port {port}
						</p>
					</div>
				</div>

				{reachable ? (
					<div className="flex items-center gap-1 text-xs text-th-status-success-text">
						<CheckCircle2 size={14} />
						<span>Up</span>
					</div>
				) : (
					<div className="flex items-center gap-1 text-xs text-th-status-error-text">
						<XCircle size={14} />
						<span>Down</span>
					</div>
				)}
			</div>

			{/* Metrics grid */}
			{reachable ? (
				<div className="grid grid-cols-3 gap-3">
					<Stat label="Requests" value={formatNumber(http.requestsTotal)} />
					<Stat
						label="Error rate"
						value={`${errorPct}%`}
						valueClass={errorRateClass(http.errorRate)}
					/>
					<Stat
						label="Response"
						value={responseTimeMs !== undefined ? `${responseTimeMs}ms` : "—"}
						sub={
							responseTimeMs !== undefined
								? responseTimeMs < 100
									? "Fast"
									: responseTimeMs < 500
										? "OK"
										: "Slow"
								: undefined
						}
					/>
				</div>
			) : (
				<div className="flex items-center gap-2 text-sm text-th-status-error-text">
					<AlertCircle size={14} />
					<span>Service unreachable</span>
				</div>
			)}

			{/* Error count detail */}
			{reachable && http.errorsTotal > 0 && (
				<p className="text-xs text-th-status-warning-text">
					{formatNumber(http.errorsTotal)} error
					{http.errorsTotal !== 1 ? "s" : ""} logged
				</p>
			)}
		</div>
	);
}

export default ServiceMetricsCard;
