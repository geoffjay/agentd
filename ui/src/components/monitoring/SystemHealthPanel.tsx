/**
 * SystemHealthPanel — grid of service health indicators with response times.
 *
 * Shows per-service:
 * - Status dot + name
 * - Port
 * - Response time (measured client-side from /health check)
 * - Uptime calculation from repeated successful checks
 */

import { Activity, CheckCircle2, Clock, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ServiceMetricsData } from "@/hooks/useMetrics";
import { askClient } from "@/services/ask";
import { notifyClient } from "@/services/notify";
import { orchestratorClient } from "@/services/orchestrator";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SystemHealthPanelProps {
	serviceMetrics: ServiceMetricsData[];
	loading?: boolean;
}

interface ResponseTimeStat {
	key: string;
	latestMs?: number;
	successCount: number;
	totalChecks: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const HEALTH_FETCHERS = {
	orchestrator: () => orchestratorClient.getHealth(),
	notify: () => notifyClient.getHealth(),
	ask: () => askClient.getHealth(),
};

function uptimePct(stat: ResponseTimeStat): number {
	if (stat.totalChecks === 0) return 0;
	return Math.round((stat.successCount / stat.totalChecks) * 100);
}

function responseClass(ms?: number): string {
	if (ms === undefined) return "text-th-text-faint";
	if (ms < 100) return "text-th-status-success-text";
	if (ms < 500) return "text-th-status-warning-text";
	return "text-th-status-error-text";
}

// ---------------------------------------------------------------------------
// SystemHealthPanel
// ---------------------------------------------------------------------------

export function SystemHealthPanel({
	serviceMetrics,
	loading,
}: SystemHealthPanelProps) {
	const [responseStats, setResponseStats] = useState<
		Record<string, ResponseTimeStat>
	>({});
	const statsRef = useRef<Record<string, ResponseTimeStat>>({});

	const measureResponseTimes = useCallback(async () => {
		const entries = Object.entries(HEALTH_FETCHERS);
		await Promise.allSettled(
			entries.map(async ([key, fetcher]) => {
				const start = performance.now();
				let success = false;
				try {
					await fetcher();
					success = true;
				} catch {
					// reachability failure
				}
				const ms = success ? Math.round(performance.now() - start) : undefined;

				const prev = statsRef.current[key] ?? {
					key,
					successCount: 0,
					totalChecks: 0,
				};
				const next: ResponseTimeStat = {
					key,
					latestMs: success ? ms : undefined,
					successCount: prev.successCount + (success ? 1 : 0),
					totalChecks: prev.totalChecks + 1,
				};
				statsRef.current = { ...statsRef.current, [key]: next };
			}),
		);
		setResponseStats({ ...statsRef.current });
	}, []);

	useEffect(() => {
		void measureResponseTimes();
		const timer = setInterval(() => void measureResponseTimes(), 30_000);
		return () => clearInterval(timer);
	}, [measureResponseTimes]);

	return (
		<div className="rounded-lg border border-th-border bg-th-surface p-5">
			<div className="flex items-center gap-2 mb-4">
				<Activity size={16} className="text-th-text-link" />
				<h3 className="text-sm font-semibold text-th-text">System Health</h3>
			</div>

			{loading ? (
				<div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
					{[1, 2, 3].map((i) => (
						<div
							key={i}
							className="h-20 rounded-lg bg-th-surface-sunken animate-pulse"
						/>
					))}
				</div>
			) : (
				<div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
					{serviceMetrics.map((svc) => {
						const stat = responseStats[svc.key];
						const uptime = stat ? uptimePct(stat) : null;

						return (
							<div
								key={svc.key}
								className={[
									"rounded-lg border p-3 space-y-2",
									svc.reachable
										? "border-th-border-subtle bg-th-surface-sunken"
										: "border-th-status-error-border bg-th-status-error-bg",
								].join(" ")}
							>
								{/* Service name + status */}
								<div className="flex items-center justify-between">
									<span className="text-sm font-medium text-th-text">
										{svc.name}
									</span>
									{svc.reachable ? (
										<CheckCircle2
											size={15}
											className="text-th-status-success-text"
										/>
									) : (
										<XCircle size={15} className="text-th-status-error-text" />
									)}
								</div>

								{/* Port */}
								<p className="text-xs text-th-text-faint">Port {svc.port}</p>

								{/* Response time + uptime */}
								<div className="flex items-center justify-between">
									<div className="flex items-center gap-1">
										<Clock size={12} className="text-th-text-faint" />
										<span
											className={`text-xs tabular-nums font-medium ${responseClass(stat?.latestMs)}`}
										>
											{stat?.latestMs !== undefined
												? `${stat.latestMs}ms`
												: "—"}
										</span>
									</div>
									{uptime !== null && (
										<span
											className={`text-xs tabular-nums ${uptime === 100 ? "text-th-status-success-text" : uptime > 90 ? "text-th-status-warning-text" : "text-th-status-error-text"}`}
										>
											{uptime}% up
										</span>
									)}
								</div>
							</div>
						);
					})}
				</div>
			)}
		</div>
	);
}

export default SystemHealthPanel;
