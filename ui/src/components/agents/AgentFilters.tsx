/**
 * AgentFilters — filter toolbar for the agents list.
 *
 * Controls:
 * - Status dropdown (All / Running / Pending / Stopped / Failed)
 * - Text search by agent name
 * - Results count ("Showing X of Y agents")
 */

import { Search, X } from "lucide-react";
import type { AgentStatus } from "@/types/orchestrator";

export interface AgentFiltersProps {
	/** Currently selected status filter; empty string means "All" */
	status: AgentStatus | "";
	onStatusChange: (status: AgentStatus | "") => void;
	/** Search query string */
	search: string;
	onSearchChange: (search: string) => void;
	/** Number of agents currently displayed */
	displayCount: number;
	/** Total agents matching the filter */
	totalCount: number;
}

const STATUS_OPTIONS: Array<{ label: string; value: AgentStatus | "" }> = [
	{ label: "All statuses", value: "" },
	{ label: "Running", value: "running" },
	{ label: "Pending", value: "pending" },
	{ label: "Stopped", value: "stopped" },
	{ label: "Failed", value: "failed" },
];

export function AgentFilters({
	status,
	onStatusChange,
	search,
	onSearchChange,
	displayCount,
	totalCount,
}: AgentFiltersProps) {
	return (
		<div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
			{/* Left: filter controls */}
			<div className="flex flex-col gap-2 sm:flex-row sm:items-center">
				{/* Status dropdown */}
				<select
					aria-label="Filter by status"
					value={status}
					onChange={(e) => onStatusChange(e.target.value as AgentStatus | "")}
					className="rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text shadow-sm focus:border-th-border-focus focus:outline-none focus:ring-1 focus:ring-th-focus-ring"
				>
					{STATUS_OPTIONS.map((opt) => (
						<option key={opt.value} value={opt.value}>
							{opt.label}
						</option>
					))}
				</select>

				{/* Name search */}
				<div className="relative">
					<Search
						size={14}
						aria-hidden="true"
						className="absolute left-3 top-1/2 -translate-y-1/2 text-th-text-muted"
					/>
					<input
						type="search"
						aria-label="Search agents by name"
						placeholder="Search by name…"
						value={search}
						onChange={(e) => onSearchChange(e.target.value)}
						className="w-full rounded-md border border-th-border-input bg-th-input py-2 pl-9 pr-8 text-sm text-th-text shadow-sm placeholder:text-th-text-muted focus:border-th-border-focus focus:outline-none focus:ring-1 focus:ring-th-focus-ring sm:w-60"
					/>
					{search && (
						<button
							type="button"
							aria-label="Clear search"
							onClick={() => onSearchChange("")}
							className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-0.5 text-th-text-muted hover:text-th-text-secondary"
						>
							<X size={12} />
						</button>
					)}
				</div>
			</div>

			{/* Right: results count */}
			<p className="text-sm text-th-text-muted">
				Showing{" "}
				<span className="font-medium text-th-text-secondary">
					{displayCount}
				</span>{" "}
				of{" "}
				<span className="font-medium text-th-text-secondary">
					{totalCount}
				</span>{" "}
				{totalCount === 1 ? "agent" : "agents"}
			</p>
		</div>
	);
}

export default AgentFilters;
