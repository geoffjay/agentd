/**
 * DiskUsageCard — per-mount disk utilisation bars from the latest monitor
 * snapshot.
 *
 * Time series per mount point is noisy and rarely actionable; the current
 * fill level per mount is what operators check. Bars colour-shift as they
 * approach capacity.
 */

import { HardDrive } from "lucide-react";
import { ChartSkeleton } from "@/components/common/LoadingSkeleton";
import type { DiskMetrics } from "@/types/monitor";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface DiskUsageCardProps {
	disks: DiskMetrics[];
	/** False when the monitor service is unreachable */
	available?: boolean;
	loading?: boolean;
	/** Height of the content area in pixels (default 140) */
	height?: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatBytes(bytes: number): string {
	const gib = bytes / 1024 ** 3;
	if (gib >= 1024) return `${(gib / 1024).toFixed(1)} TiB`;
	return `${gib.toFixed(0)} GiB`;
}

/** Bar colour by fill level: calm → warning → critical. */
function barColor(usagePercent: number): string {
	if (usagePercent >= 90) return "var(--th-status-error-text, #ef4444)";
	if (usagePercent >= 75) return "var(--th-status-warning-text, #f59e0b)";
	return "#3b82f6";
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function DiskUsageCard({
	disks,
	available = true,
	loading = false,
	height = 140,
}: DiskUsageCardProps) {
	const hasData = available && disks.length > 0;

	return (
		<div className="rounded-lg border border-th-border bg-th-surface p-4">
			<h3 className="text-sm font-semibold text-th-text">Disk Usage</h3>
			<p className="mt-0.5 text-xs text-th-text-faint">
				Per-mount utilisation
			</p>

			{loading && (
				<div className="mt-3">
					<ChartSkeleton height={height} />
				</div>
			)}

			{!loading && !hasData && (
				<div
					className="mt-3 flex flex-col items-center justify-center gap-1.5 text-center"
					style={{ height }}
				>
					<HardDrive size={20} className="text-th-text-faint" />
					<p className="text-xs text-th-text-muted">
						{available
							? "No metrics collected yet."
							: "Monitor service unavailable"}
					</p>
				</div>
			)}

			{!loading && hasData && (
				<ul
					className="mt-3 space-y-3 overflow-y-auto"
					style={{ maxHeight: height }}
					data-testid="disk-usage-list"
				>
					{disks.map((disk) => (
						<li key={disk.mount_point}>
							<div className="flex items-baseline justify-between gap-2 text-xs">
								<span
									className="truncate font-medium text-th-text"
									title={`${disk.name} (${disk.mount_point})`}
								>
									{disk.mount_point}
								</span>
								<span className="shrink-0 text-th-text-muted">
									{formatBytes(disk.used_bytes)} /{" "}
									{formatBytes(disk.total_bytes)}
									<span className="ml-1.5 font-semibold text-th-text">
										{disk.usage_percent.toFixed(0)}%
									</span>
								</span>
							</div>
							<div
								className="mt-1 h-1.5 overflow-hidden rounded-full bg-th-surface-hover"
								role="progressbar"
								aria-label={`${disk.mount_point} usage`}
								aria-valuenow={Math.round(disk.usage_percent)}
								aria-valuemin={0}
								aria-valuemax={100}
							>
								<div
									className="h-full rounded-full transition-all"
									style={{
										width: `${Math.min(100, disk.usage_percent)}%`,
										background: barColor(disk.usage_percent),
									}}
								/>
							</div>
						</li>
					))}
				</ul>
			)}
		</div>
	);
}

export default DiskUsageCard;
