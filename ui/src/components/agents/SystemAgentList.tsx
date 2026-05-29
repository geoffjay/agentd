/**
 * SystemAgentList — display of built-in system agents.
 *
 * Renders above the main agent list on the AgentsPage. Shows name, status,
 * model, and rooms. No create/delete actions are available — system agents
 * are managed automatically by the orchestrator.
 *
 * Features:
 * - Auto-refresh every 10 seconds (shared with the main agent list cadence)
 * - Visual distinction via a "System" badge on each row
 * - Clicking a row navigates to the agent detail page (same as user agents)
 */

import { AlertCircle, Bot, RefreshCw } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { AgentStatusBadge } from "@/components/agents/AgentStatusBadge";
import { ListItemSkeleton } from "@/components/common/LoadingSkeleton";
import { useSystemAgents } from "@/hooks/useSystemAgents";
import type { Agent } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// SystemAgentList
// ---------------------------------------------------------------------------

export function SystemAgentList() {
	const { agents, loading, refreshing, error, refetch } = useSystemAgents();
	const navigate = useNavigate();

	return (
		<div className="space-y-5 mb-8">
			{/* Header */}
			<div className="flex items-center justify-between">
				<div>
					<h1 className="text-2xl font-semibold text-th-text">System Agents</h1>
					<p className="mt-1 text-sm text-th-text-muted">
						Built-in agents managed by the orchestrator.
					</p>
				</div>

				<div className="flex items-center gap-2">
					{refreshing && (
						<RefreshCw
							size={14}
							aria-label="Refreshing..."
							className="animate-spin text-th-text-muted"
						/>
					)}
					<button
						type="button"
						aria-label="Refresh system agents"
						onClick={refetch}
						disabled={loading}
						className="rounded-md border border-th-border-strong bg-th-surface p-2 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text-secondary focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-1 disabled:opacity-50"
					>
						<RefreshCw size={15} />
					</button>
				</div>
			</div>

			{/* Error banner */}
			{error && (
				<div
					role="alert"
					className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text"
				>
					<div className="flex items-center gap-2">
						<AlertCircle className="h-4 w-4 flex-shrink-0" />
						{error}
					</div>
				</div>
			)}

			{/* Table */}
			<div className="overflow-hidden rounded-lg border border-th-border">
				<div className="overflow-x-auto">
					<table className="min-w-full divide-y divide-th-border">
						<thead className="bg-th-surface-sunken">
							<tr>
								<th className="px-4 py-3 text-left text-xs font-medium text-th-text-muted">
									Name
								</th>
								<th className="px-4 py-3 text-left text-xs font-medium text-th-text-muted">
									Status
								</th>
								<th className="px-4 py-3 text-left text-xs font-medium text-th-text-muted">
									Model
								</th>
								<th className="px-4 py-3 text-left text-xs font-medium text-th-text-muted">
									Rooms
								</th>
							</tr>
						</thead>
						<tbody className="divide-y divide-th-border bg-th-surface">
							{loading ? (
								<tr>
									<td colSpan={4} className="p-4">
										<ListItemSkeleton rows={2} />
									</td>
								</tr>
							) : agents.length === 0 && !error ? (
								<tr>
									<td colSpan={4} className="py-12 text-center">
										<p className="text-sm text-th-text-muted">
											No system agents found.
										</p>
										<p className="mt-1 text-xs text-th-text-faint">
											The orchestrator may still be starting up.
										</p>
									</td>
								</tr>
							) : (
								agents.map((agent) => (
									<SystemAgentRow
										key={agent.id}
										agent={agent}
										onClick={() => navigate(`/agents/${agent.id}`)}
									/>
								))
							)}
						</tbody>
					</table>
				</div>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// SystemAgentRow
// ---------------------------------------------------------------------------

interface SystemAgentRowProps {
	agent: Agent;
	onClick: () => void;
}

function SystemAgentRow({ agent, onClick }: SystemAgentRowProps) {
	const rooms = agent.config.rooms ?? [];

	return (
		<tr
			className="cursor-pointer border-b border-th-border hover:bg-th-surface-hover"
			onClick={onClick}
		>
			{/* Name + system badge */}
			<td className="px-4 py-3">
				<div className="flex items-center gap-2">
					<Bot className="h-3.5 w-3.5 flex-shrink-0 text-th-text-muted" />
					<span className="text-sm font-medium text-th-text">{agent.name}</span>
					<span className="rounded-full bg-th-accent/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-th-accent">
						system
					</span>
				</div>
			</td>

			{/* Status */}
			<td className="px-4 py-3">
				<AgentStatusBadge status={agent.status} />
			</td>

			{/* Model */}
			<td className="px-4 py-3 text-sm text-th-text-muted">
				{agent.config.model ?? (
					<span className="italic opacity-50">default</span>
				)}
			</td>

			{/* Rooms */}
			<td className="px-4 py-3 text-sm text-th-text-muted">
				{rooms.length > 0 ? (
					<div className="flex flex-wrap gap-1">
						{rooms.map((r: string) => (
							<span
								key={r}
								className="rounded bg-th-surface-sunken px-1.5 py-0.5 text-[10px] text-th-text-muted"
							>
								{r}
							</span>
						))}
					</div>
				) : (
					<span className="text-th-text-faint">-</span>
				)}
			</td>
		</tr>
	);
}

export default SystemAgentList;
