/**
 * EnvironmentStatus — displays the current tmux environment status.
 *
 * Shows:
 * - Running indicator (green/red dot)
 * - Session count and names
 * - Last checked timestamp
 * - Expandable raw JSON view
 */

import { ChevronDown, ChevronRight, Clock, Terminal } from "lucide-react";
import { useState } from "react";
import type { TmuxCheckResult } from "@/types/ask";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface EnvironmentStatusProps {
	tmux?: TmuxCheckResult;
	lastCheckedAt?: Date;
	loading?: boolean;
}

// ---------------------------------------------------------------------------
// EnvironmentStatus
// ---------------------------------------------------------------------------

export function EnvironmentStatus({
	tmux,
	lastCheckedAt,
	loading,
}: EnvironmentStatusProps) {
	const [expanded, setExpanded] = useState(false);

	return (
		<div className="rounded-lg border border-th-border bg-th-surface p-5 space-y-3">
			{/* Header */}
			<div className="flex items-center gap-2">
				<Terminal size={15} className="text-th-text-muted" />
				<h3 className="text-sm font-semibold text-th-text">
					Environment Status
				</h3>
			</div>

			{loading ? (
				<div className="space-y-2">
					<div className="h-4 w-2/3 rounded bg-th-surface-sunken animate-pulse" />
					<div className="h-4 w-1/2 rounded bg-th-surface-sunken animate-pulse" />
				</div>
			) : tmux ? (
				<div className="space-y-3">
					{/* Tmux status card */}
					<div
						className={[
							"rounded-md border p-3 space-y-2",
							tmux.running
								? "border-th-status-success-border bg-th-status-success-bg"
								: "border-th-status-error-border bg-th-status-error-bg",
						].join(" ")}
					>
						{/* Running indicator */}
						<div className="flex items-center justify-between">
							<div className="flex items-center gap-2">
								<span
									className={[
										"h-2.5 w-2.5 rounded-full flex-shrink-0",
										tmux.running
											? "bg-th-status-success-dot"
											: "bg-th-status-error-dot",
									].join(" ")}
									role="status"
									aria-label={
										tmux.running ? "tmux running" : "tmux not running"
									}
								/>
								<span className="text-sm font-medium text-th-text">tmux</span>
							</div>
							<span
								className={[
									"text-xs font-medium",
									tmux.running
										? "text-th-status-success-text"
										: "text-th-status-error-text",
								].join(" ")}
							>
								{tmux.running ? "Running" : "Not running"}
							</span>
						</div>

						{/* Session count */}
						<p className="text-xs text-th-text-muted">
							{tmux.session_count === 0
								? "No active sessions"
								: `${tmux.session_count} active session${tmux.session_count !== 1 ? "s" : ""}`}
						</p>

						{/* Session names */}
						{tmux.sessions && tmux.sessions.length > 0 && (
							<div className="flex flex-wrap gap-1">
								{tmux.sessions.map((session) => (
									<span
										key={session}
										className="rounded bg-th-surface-sunken px-1.5 py-0.5 font-mono text-xs text-th-text-secondary"
									>
										{session}
									</span>
								))}
							</div>
						)}
					</div>

					{/* Last checked timestamp */}
					{lastCheckedAt && (
						<p className="flex items-center gap-1 text-xs text-th-text-faint">
							<Clock size={11} />
							Checked at{" "}
							{lastCheckedAt.toLocaleTimeString([], {
								hour: "2-digit",
								minute: "2-digit",
								second: "2-digit",
							})}
						</p>
					)}

					{/* Expandable raw JSON */}
					<div>
						<button
							type="button"
							onClick={() => setExpanded((e) => !e)}
							className="flex items-center gap-1 text-xs text-th-text-faint hover:text-th-text-secondary transition-colors"
							aria-expanded={expanded}
						>
							{expanded ? (
								<ChevronDown size={12} />
							) : (
								<ChevronRight size={12} />
							)}
							{expanded ? "Hide" : "Show"} raw result
						</button>
						{expanded && (
							<pre className="mt-2 overflow-auto rounded-md bg-th-surface-sunken border border-th-border p-2 text-xs text-th-text-secondary">
								{JSON.stringify(tmux, null, 2)}
							</pre>
						)}
					</div>
				</div>
			) : (
				<p className="text-sm text-th-text-faint">
					No check results yet. Run checks to see environment status.
				</p>
			)}
		</div>
	);
}

export default EnvironmentStatus;
