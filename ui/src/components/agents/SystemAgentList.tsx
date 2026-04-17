/**
 * SystemAgentList — compact display of built-in system agents.
 *
 * Renders above the main agent list on the AgentsPage. Shows name, status,
 * model, and activity state. No create/delete actions are available — system
 * agents are managed automatically by the orchestrator.
 *
 * Features:
 * - Auto-refresh every 10 seconds (shared with the main agent list cadence)
 * - Collapsible panel (expanded by default)
 * - Visual distinction via a "System" badge on each row
 * - Clicking a row navigates to the agent detail page (same as user agents)
 */

import {
	AlertCircle,
	Bot,
	ChevronDown,
	ChevronRight,
	RefreshCw,
} from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { AgentStatusBadge } from "@/components/agents/AgentStatusBadge";
import { useSystemAgents } from "@/hooks/useSystemAgents";
import type { Agent } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// SystemAgentList
// ---------------------------------------------------------------------------

export function SystemAgentList() {
	const { agents, loading, refreshing, error, refetch } = useSystemAgents();
	const [collapsed, setCollapsed] = useState(false);
	const navigate = useNavigate();

	return (
		<div className="mb-6 rounded-lg border border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950/30">
			{/* Header */}
			<div className="flex items-center justify-between px-4 py-3">
				<button
					type="button"
					className="flex items-center gap-2 text-sm font-semibold text-blue-800 dark:text-blue-300"
					onClick={() => setCollapsed((c) => !c)}
				>
					{collapsed ? (
						<ChevronRight className="h-4 w-4" />
					) : (
						<ChevronDown className="h-4 w-4" />
					)}
					<Bot className="h-4 w-4" />
					System Agents
					{!loading && (
						<span className="ml-1 rounded-full bg-blue-200 px-2 py-0.5 text-xs font-medium text-blue-800 dark:bg-blue-800 dark:text-blue-200">
							{agents.length}
						</span>
					)}
				</button>

				<div className="flex items-center gap-2">
					{refreshing && (
						<RefreshCw className="h-3 w-3 animate-spin text-blue-600 dark:text-blue-400" />
					)}
					<button
						type="button"
						title="Refresh system agents"
						onClick={refetch}
						className="rounded p-1 text-blue-600 hover:bg-blue-100 dark:text-blue-400 dark:hover:bg-blue-900"
					>
						<RefreshCw className="h-3.5 w-3.5" />
					</button>
				</div>
			</div>

			{/* Body */}
			{!collapsed && (
				<div className="border-t border-blue-200 dark:border-blue-800">
					{/* Error state */}
					{error && (
						<div className="flex items-center gap-2 px-4 py-3 text-sm text-red-600 dark:text-red-400">
							<AlertCircle className="h-4 w-4 flex-shrink-0" />
							{error}
						</div>
					)}

					{/* Loading state */}
					{loading && !error && (
						<div className="px-4 py-4 text-sm text-blue-600 dark:text-blue-400">
							Loading system agents…
						</div>
					)}

					{/* Empty state */}
					{!loading && !error && agents.length === 0 && (
						<div className="px-4 py-4 text-sm text-blue-600/70 dark:text-blue-400/70">
							No system agents found. The orchestrator may still be starting up.
						</div>
					)}

					{/* Agent rows */}
					{!loading && !error && agents.length > 0 && (
						<table className="w-full text-sm">
							<thead>
								<tr className="border-b border-blue-200 bg-blue-100/50 text-left text-xs font-medium text-blue-700 dark:border-blue-800 dark:bg-blue-900/30 dark:text-blue-400">
									<th className="px-4 py-2">Name</th>
									<th className="px-4 py-2">Status</th>
									<th className="px-4 py-2">Model</th>
									<th className="px-4 py-2">Rooms</th>
								</tr>
							</thead>
							<tbody>
								{agents.map((agent) => (
									<SystemAgentRow
										key={agent.id}
										agent={agent}
										onClick={() =>
											navigate(`/agents/${agent.id}`)
										}
									/>
								))}
							</tbody>
						</table>
					)}
				</div>
			)}
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
			className="cursor-pointer border-b border-blue-100 last:border-0 hover:bg-blue-100/60 dark:border-blue-900 dark:hover:bg-blue-900/40"
			onClick={onClick}
		>
			{/* Name + system badge */}
			<td className="px-4 py-3">
				<div className="flex items-center gap-2">
					<Bot className="h-3.5 w-3.5 flex-shrink-0 text-blue-500 dark:text-blue-400" />
					<span className="font-medium text-gray-900 dark:text-gray-100">
						{agent.name}
					</span>
					<span className="rounded-full bg-blue-200 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-blue-800 dark:bg-blue-800 dark:text-blue-200">
						system
					</span>
				</div>
			</td>

			{/* Status */}
			<td className="px-4 py-3">
				<AgentStatusBadge status={agent.status} />
			</td>

			{/* Model */}
			<td className="px-4 py-3 text-gray-600 dark:text-gray-400">
				{agent.config.model ?? "default"}
			</td>

			{/* Rooms */}
			<td className="px-4 py-3 text-gray-600 dark:text-gray-400">
				{rooms.length > 0 ? (
					<div className="flex flex-wrap gap-1">
						{rooms.map((r: string) => (
							<span
								key={r}
								className="rounded bg-blue-100 px-1.5 py-0.5 text-[10px] text-blue-800 dark:bg-blue-900 dark:text-blue-300"
							>
								{r}
							</span>
						))}
					</div>
				) : (
					<span className="text-xs text-gray-400">—</span>
				)}
			</td>
		</tr>
	);
}

export default SystemAgentList;
