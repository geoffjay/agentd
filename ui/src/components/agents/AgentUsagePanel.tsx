/**
 * AgentUsagePanel — displays token usage, cost, cache efficiency, and timing
 * for an agent's current session and cumulative lifetime stats.
 *
 * Layout:
 * 1. Current Session Summary — stat cards in a responsive grid
 * 2. Cache Efficiency Indicator — progress bar with color coding
 * 3. Cumulative Stats — collapsible section with lifetime totals
 * 4. Session Info — session number, start time, auto-clear threshold
 */

import { BarChart3, ChevronDown, ChevronRight, Zap } from "lucide-react";
import { useState } from "react";
import type { AgentUsageStats, SessionUsage } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

const tokenFmt = new Intl.NumberFormat("en-US", {
	notation: "compact",
	maximumFractionDigits: 1,
});
const costFmt = new Intl.NumberFormat("en-US", {
	style: "currency",
	currency: "USD",
	minimumFractionDigits: 4,
	maximumFractionDigits: 4,
});
const pctFmt = new Intl.NumberFormat("en-US", {
	style: "percent",
	minimumFractionDigits: 1,
	maximumFractionDigits: 1,
});

function formatDuration(ms: number): string {
	if (ms < 1_000) return `${ms}ms`;
	if (ms < 60_000) return `${(ms / 1_000).toFixed(1)}s`;
	const mins = Math.floor(ms / 60_000);
	const secs = Math.round((ms % 60_000) / 1_000);
	return `${mins}m ${secs}s`;
}

function relativeTime(dateStr: string): string {
	const diff = Date.now() - new Date(dateStr).getTime();
	if (diff < 60_000) return "just now";
	if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
	if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
	return `${Math.floor(diff / 86_400_000)}d ago`;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface StatCardProps {
	label: string;
	value: string;
	detail?: string;
	ariaLabel?: string;
}

function StatCard({ label, value, detail, ariaLabel }: StatCardProps) {
	return (
		<div
			className="flex flex-col gap-0.5 rounded-md border border-th-border bg-th-surface-sunken px-3 py-2"
			aria-label={ariaLabel ?? `${label}: ${value}`}
		>
			<span className="text-xs font-medium text-th-text-faint">
				{label}
			</span>
			<span className="text-sm font-semibold text-th-text">
				{value}
			</span>
			{detail && (
				<span className="text-xs text-th-text-faint">
					{detail}
				</span>
			)}
		</div>
	);
}

interface CacheEfficiencyBarProps {
	ratio: number; // 0–1
}

function CacheEfficiencyBar({ ratio }: CacheEfficiencyBarProps) {
	const color =
		ratio > 0.5
			? "bg-th-status-success-dot"
			: ratio > 0.2
				? "bg-th-status-warning-dot"
				: "bg-th-status-error-dot";

	const label = ratio > 0.5 ? "Excellent" : ratio > 0.2 ? "Moderate" : "Low";

	return (
		<div className="flex flex-col gap-1.5">
			<div className="flex items-center justify-between">
				<span className="text-xs font-medium text-th-text-faint">
					Cache Hit Ratio
				</span>
				<span
					className="text-xs font-semibold text-th-text-secondary"
					aria-label={`Cache hit ratio: ${pctFmt.format(ratio)}`}
				>
					{pctFmt.format(ratio)} — {label}
				</span>
			</div>
			<div
				className="h-2 w-full overflow-hidden rounded-full bg-th-surface-sunken"
				role="progressbar"
				aria-valuenow={Math.round(ratio * 100)}
				aria-valuemin={0}
				aria-valuemax={100}
				aria-label={`Cache efficiency: ${pctFmt.format(ratio)}`}
			>
				<div
					className={`h-full rounded-full transition-all duration-300 ${color}`}
					style={{ width: `${Math.min(ratio * 100, 100)}%` }}
				/>
			</div>
		</div>
	);
}

function SessionStats({
	session,
	label,
}: {
	session: SessionUsage;
	label: string;
}) {
	return (
		<div
			className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-4"
			aria-label={`${label} statistics`}
		>
			<StatCard
				label="Input Tokens"
				value={tokenFmt.format(session.input_tokens)}
				ariaLabel={`Input tokens: ${session.input_tokens.toLocaleString()}`}
			/>
			<StatCard
				label="Output Tokens"
				value={tokenFmt.format(session.output_tokens)}
				ariaLabel={`Output tokens: ${session.output_tokens.toLocaleString()}`}
			/>
			<StatCard
				label="Cache Read"
				value={tokenFmt.format(session.cache_read_input_tokens)}
				ariaLabel={`Cache read tokens: ${session.cache_read_input_tokens.toLocaleString()}`}
			/>
			<StatCard
				label="Cache Created"
				value={tokenFmt.format(session.cache_creation_input_tokens)}
				ariaLabel={`Cache creation tokens: ${session.cache_creation_input_tokens.toLocaleString()}`}
			/>
			<StatCard
				label="Total Cost"
				value={costFmt.format(session.total_cost_usd)}
				ariaLabel={`Total cost: ${costFmt.format(session.total_cost_usd)}`}
			/>
			<StatCard
				label="Turns"
				value={session.num_turns.toString()}
				ariaLabel={`Number of turns: ${session.num_turns}`}
			/>
			<StatCard
				label="Wall Clock"
				value={formatDuration(session.duration_ms)}
				ariaLabel={`Wall clock duration: ${formatDuration(session.duration_ms)}`}
			/>
			<StatCard
				label="API Duration"
				value={formatDuration(session.duration_api_ms)}
				detail={
					session.duration_ms > 0
						? `${Math.round((session.duration_api_ms / session.duration_ms) * 100)}% of wall clock`
						: undefined
				}
				ariaLabel={`API duration: ${formatDuration(session.duration_api_ms)}`}
			/>
		</div>
	);
}

// ---------------------------------------------------------------------------
// AgentUsagePanel
// ---------------------------------------------------------------------------

export interface AgentUsagePanelProps {
	usage: AgentUsageStats;
	/** auto_clear_threshold from agent config, if configured */
	autoClearThreshold?: number;
}

export function AgentUsagePanel({
	usage,
	autoClearThreshold,
}: AgentUsagePanelProps) {
	const [cumulativeOpen, setCumulativeOpen] = useState(false);

	const currentSession = usage.current_session;
	const cumulative = usage.cumulative;

	// Cache efficiency for current session
	const sessionCacheRatio = currentSession
		? computeCacheRatio(currentSession)
		: null;

	// Cache efficiency for cumulative
	const cumulativeCacheRatio = computeCacheRatio(cumulative);

	const avgCostPerSession =
		usage.session_count > 0
			? cumulative.total_cost_usd / usage.session_count
			: 0;

	return (
		<section
			aria-label="Agent usage statistics"
			className="rounded-lg border border-th-border bg-th-surface"
		>
			{/* Header */}
			<div className="flex items-center gap-2 px-4 py-3 text-sm font-medium text-th-text">
				<BarChart3 size={16} aria-hidden="true" className="text-th-text-muted" />
				<span>Usage</span>
				{usage.session_count > 0 && (
					<span className="ml-auto rounded-full bg-th-surface-sunken px-2 py-0.5 text-xs font-medium text-th-text-muted">
						Session {usage.session_count}
					</span>
				)}
			</div>

			<div className="flex flex-col gap-4 border-t border-th-border px-4 py-4">
				{/* ── Current Session ──────────────────────────────────────────── */}
				{currentSession ? (
					<>
						<div className="flex flex-col gap-2" aria-live="polite">
							<h3 className="text-xs font-semibold uppercase tracking-wide text-th-text-muted">
								Current Session
							</h3>
							<SessionStats session={currentSession} label="Current session" />
						</div>

						{/* Cache efficiency */}
						{sessionCacheRatio !== null && (
							<CacheEfficiencyBar ratio={sessionCacheRatio} />
						)}

						{/* Session info */}
						{currentSession.started_at && (
							<div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-th-text-muted">
								<span>
									Started{" "}
									<time
										dateTime={currentSession.started_at}
										title={new Date(currentSession.started_at).toLocaleString()}
									>
										{relativeTime(currentSession.started_at)}
									</time>
								</span>
								<span>{currentSession.result_count} result(s)</span>
							</div>
						)}
					</>
				) : (
					<p className="text-sm text-th-text-faint">
						No active session.
					</p>
				)}

				{/* ── Auto-clear threshold ─────────────────────────────────────── */}
				{autoClearThreshold != null &&
					autoClearThreshold > 0 &&
					currentSession && (
						<AutoClearProgress
							threshold={autoClearThreshold}
							currentCost={currentSession.total_cost_usd}
						/>
					)}

				{/* ── Cumulative Stats (collapsible) ───────────────────────────── */}
				<div>
					<button
						type="button"
						aria-expanded={cumulativeOpen}
						aria-controls="usage-cumulative-body"
						onClick={() => setCumulativeOpen((o) => !o)}
						className="flex w-full items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-th-text-muted hover:text-th-text-secondary"
					>
						{cumulativeOpen ? (
							<ChevronDown size={14} />
						) : (
							<ChevronRight size={14} />
						)}
						Cumulative ({usage.session_count} session
						{usage.session_count !== 1 ? "s" : ""})
					</button>

					{cumulativeOpen && (
						<div
							id="usage-cumulative-body"
							className="mt-3 flex flex-col gap-3"
							aria-live="polite"
						>
							<SessionStats session={cumulative} label="Cumulative" />
							<CacheEfficiencyBar ratio={cumulativeCacheRatio} />

							<div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-th-text-muted">
								<span>
									Avg cost / session:{" "}
									<span className="font-medium text-th-text-secondary">
										{costFmt.format(avgCostPerSession)}
									</span>
								</span>
							</div>
						</div>
					)}
				</div>
			</div>
		</section>
	);
}

// ---------------------------------------------------------------------------
// Auto-clear threshold progress
// ---------------------------------------------------------------------------

function AutoClearProgress({
	threshold,
	currentCost,
}: {
	threshold: number;
	currentCost: number;
}) {
	const progress = threshold > 0 ? Math.min(currentCost / threshold, 1) : 0;
	const isNear = progress > 0.8;

	return (
		<div className="flex flex-col gap-1.5">
			<div className="flex items-center justify-between">
				<span className="flex items-center gap-1 text-xs font-medium text-th-text-faint">
					<Zap size={12} aria-hidden="true" />
					Auto-clear Threshold
				</span>
				<span className="text-xs text-th-text-muted">
					{costFmt.format(currentCost)} / {costFmt.format(threshold)}
				</span>
			</div>
			<div
				className="h-1.5 w-full overflow-hidden rounded-full bg-th-surface-sunken"
				role="progressbar"
				aria-valuenow={Math.round(progress * 100)}
				aria-valuemin={0}
				aria-valuemax={100}
				aria-label={`Auto-clear threshold progress: ${Math.round(progress * 100)}%`}
			>
				<div
					className={`h-full rounded-full transition-all duration-300 ${
						isNear
							? "bg-th-status-warning-dot"
							: "bg-th-status-info-dot"
					}`}
					style={{ width: `${progress * 100}%` }}
				/>
			</div>
			{isNear && (
				<p className="text-xs text-th-status-warning-text">
					Approaching auto-clear threshold
				</p>
			)}
		</div>
	);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function computeCacheRatio(session: SessionUsage): number {
	const total =
		session.cache_read_input_tokens +
		session.cache_creation_input_tokens +
		session.input_tokens;
	if (total === 0) return 0;
	return session.cache_read_input_tokens / total;
}

export default AgentUsagePanel;
