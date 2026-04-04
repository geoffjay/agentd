/**
 * CheckControls — run environment checks manually or on a schedule.
 *
 * Shows:
 * - "Run Checks" button with loading state
 * - Last trigger timestamp
 * - Results after a successful trigger (checks run, notifications, tmux details)
 * - Auto-trigger toggle with interval picker
 */

import {
	Bell,
	Clock,
	Play,
	RefreshCw,
	ToggleLeft,
	ToggleRight,
} from "lucide-react";
import type { AutoTriggerInterval } from "@/hooks/useAskService";
import type { TriggerResponse } from "@/types/ask";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CheckControlsProps {
	triggering: boolean;
	lastTriggerResult?: TriggerResponse;
	lastTriggerAt?: Date;
	triggerError?: string;
	autoTrigger: boolean;
	autoTriggerInterval: AutoTriggerInterval;
	onRunTrigger: () => void;
	onSetAutoTrigger: (enabled: boolean) => void;
	onSetAutoTriggerInterval: (ms: AutoTriggerInterval) => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const INTERVAL_LABELS: Record<number, string> = {
	30000: "30s",
	60000: "1m",
	300000: "5m",
	600000: "10m",
};

function formatTimestamp(date: Date): string {
	return date.toLocaleTimeString([], {
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
	});
}

// ---------------------------------------------------------------------------
// CheckControls
// ---------------------------------------------------------------------------

export function CheckControls({
	triggering,
	lastTriggerResult,
	lastTriggerAt,
	triggerError,
	autoTrigger,
	autoTriggerInterval,
	onRunTrigger,
	onSetAutoTrigger,
	onSetAutoTriggerInterval,
}: CheckControlsProps) {
	const tmux = lastTriggerResult?.results?.tmux_sessions;

	return (
		<div className="rounded-lg border border-th-border bg-th-surface p-5 space-y-4">
			{/* Header */}
			<div className="flex items-center justify-between gap-3 flex-wrap">
				<div>
					<h3 className="text-sm font-semibold text-th-text">
						Environment Checks
					</h3>
					{lastTriggerAt && (
						<p className="mt-0.5 flex items-center gap-1 text-xs text-th-text-faint">
							<Clock size={11} />
							Last run at {formatTimestamp(lastTriggerAt)}
						</p>
					)}
				</div>

				{/* Run button */}
				<button
					type="button"
					onClick={onRunTrigger}
					disabled={triggering}
					aria-label="Run environment checks"
					className="flex items-center gap-2 rounded-md bg-th-accent hover:bg-th-accent-hover disabled:opacity-60 px-4 py-2 text-sm font-medium text-th-accent-text transition-colors"
				>
					{triggering ? (
						<RefreshCw size={14} className="animate-spin" />
					) : (
						<Play size={14} />
					)}
					{triggering ? "Running…" : "Run Checks"}
				</button>
			</div>

			{/* Error */}
			{triggerError && (
				<div className="rounded-md border border-th-status-error-border bg-th-status-error-bg px-3 py-2 text-sm text-th-status-error-text">
					{triggerError}
				</div>
			)}

			{/* Last trigger results */}
			{lastTriggerResult && (
				<div className="space-y-3">
					{/* Checks performed */}
					<div className="flex items-start gap-2">
						<RefreshCw
							size={13}
							className="mt-0.5 text-th-text-muted flex-shrink-0"
						/>
						<div>
							<p className="text-xs font-medium text-th-text-muted">
								Checks run
							</p>
							<div className="mt-1 flex flex-wrap gap-1">
								{lastTriggerResult.checks_run.map((check) => (
									<span
										key={check}
										className="rounded-full bg-th-status-info-bg px-2 py-0.5 text-xs text-th-status-info-text"
									>
										{check}
									</span>
								))}
							</div>
						</div>
					</div>

					{/* Notifications sent */}
					<div className="flex items-start gap-2">
						<Bell size={13} className="mt-0.5 text-th-text-muted flex-shrink-0" />
						<div>
							<p className="text-xs font-medium text-th-text-muted">
								Notifications sent
							</p>
							{lastTriggerResult.notifications_sent.length === 0 ? (
								<p className="mt-0.5 text-xs text-th-text-faint">
									None
								</p>
							) : (
								<ul className="mt-1 space-y-0.5">
									{lastTriggerResult.notifications_sent.map((id) => (
										<li
											key={id}
											className="font-mono text-xs text-th-text-muted"
										>
											{id}
										</li>
									))}
								</ul>
							)}
						</div>
					</div>

					{/* Tmux result detail */}
					{tmux && (
						<div className="rounded-md bg-th-surface-sunken border border-th-border px-3 py-2 space-y-1">
							<p className="text-xs font-medium text-th-text-muted">
								tmux_sessions result
							</p>
							<div className="flex items-center gap-2">
								<span
									className={[
										"h-2 w-2 rounded-full flex-shrink-0",
										tmux.running ? "bg-th-status-success-dot" : "bg-th-status-error-dot",
									].join(" ")}
								/>
								<span className="text-xs text-th-text-secondary">
									{tmux.running
										? `Running - ${tmux.session_count} session${tmux.session_count !== 1 ? "s" : ""}`
										: "Not running"}
								</span>
							</div>
							{tmux.sessions && tmux.sessions.length > 0 && (
								<p className="text-xs text-th-text-faint pl-4">
									{tmux.sessions.join(", ")}
								</p>
							)}
						</div>
					)}
				</div>
			)}

			{/* Auto-trigger controls */}
			<div className="border-t border-th-border pt-4 flex items-center justify-between gap-3 flex-wrap">
				<div className="flex items-center gap-2">
					<button
						type="button"
						role="switch"
						aria-checked={autoTrigger}
						onClick={() => onSetAutoTrigger(!autoTrigger)}
						className="flex items-center gap-2 text-sm text-th-text-secondary hover:text-th-text transition-colors"
					>
						{autoTrigger ? (
							<ToggleRight
								size={20}
								className="text-th-text-link"
							/>
						) : (
							<ToggleLeft size={20} className="text-th-text-muted" />
						)}
						Auto-trigger
					</button>
				</div>

				{autoTrigger && (
					<div
						className="flex items-center rounded-md border border-th-border overflow-hidden text-xs"
						role="group"
						aria-label="Auto-trigger interval"
					>
						{Object.entries(INTERVAL_LABELS).map(([ms, label]) => (
							<button
								key={ms}
								type="button"
								onClick={() =>
									onSetAutoTriggerInterval(Number(ms) as AutoTriggerInterval)
								}
								aria-pressed={autoTriggerInterval === Number(ms)}
								className={[
									"px-2.5 py-1 transition-colors",
									autoTriggerInterval === Number(ms)
										? "bg-th-accent/10 text-th-text-link font-medium"
										: "text-th-text-muted hover:text-th-text",
								].join(" ")}
							>
								{label}
							</button>
						))}
					</div>
				)}
			</div>
		</div>
	);
}

export default CheckControls;
