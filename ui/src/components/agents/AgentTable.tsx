/**
 * AgentTable — paginated, sortable table of agents with bulk selection.
 *
 * Features:
 * - Column sort: name, status, created_at
 * - Row click → navigate to /agents/{id}
 * - Per-row actions: View, Terminate (with confirmation)
 * - Bulk select checkboxes + bulk terminate
 * - Empty and loading states
 */

import { ArrowUpDown, ChevronDown, ChevronUp, Eye, Trash2 } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { ListItemSkeleton } from "@/components/common/LoadingSkeleton";
import type { SortDir, SortField } from "@/hooks/useAgents";
import type { Agent, AgentUsageStats } from "@/types/orchestrator";
import { AgentStatusBadge } from "./AgentStatusBadge";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AgentTableProps {
	agents: Agent[];
	loading: boolean;
	sortBy: SortField;
	sortDir: SortDir;
	onSort: (field: SortField) => void;
	onDelete: (id: string) => Promise<void>;
	onBulkDelete: (ids: string[]) => Promise<void>;
	selectedIds: string[];
	onSelectChange: (ids: string[]) => void;
	/** Per-agent usage stats keyed by agent ID */
	usageMap?: Map<string, AgentUsageStats>;
}

// ---------------------------------------------------------------------------
// Sort header cell
// ---------------------------------------------------------------------------

interface SortHeaderProps {
	field: SortField;
	label: string;
	currentSort: SortField;
	currentDir: SortDir;
	onSort: (field: SortField) => void;
}

function SortHeader({
	field,
	label,
	currentSort,
	currentDir,
	onSort,
}: SortHeaderProps) {
	const isActive = currentSort === field;
	return (
		<button
			type="button"
			onClick={() => onSort(field)}
			className="flex items-center gap-1 font-medium hover:text-th-text"
			aria-sort={
				isActive ? (currentDir === "asc" ? "ascending" : "descending") : "none"
			}
		>
			{label}
			{isActive ? (
				currentDir === "asc" ? (
					<ChevronUp size={13} aria-hidden="true" />
				) : (
					<ChevronDown size={13} aria-hidden="true" />
				)
			) : (
				<ArrowUpDown size={13} aria-hidden="true" className="opacity-40" />
			)}
		</button>
	);
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

function EmptyState() {
	return (
		<tr>
			<td colSpan={10} className="py-12 text-center">
				<p className="text-sm text-th-text-muted">
					No agents found.
				</p>
				<p className="mt-1 text-xs text-th-text-faint">
					Create your first agent using the button above.
				</p>
			</td>
		</tr>
	);
}

// ---------------------------------------------------------------------------
// Row
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/** Format a USD cost value for display */
function formatCost(usd: number): string {
	if (usd < 0.01) return "<$0.01";
	return `$${usd.toFixed(2)}`;
}

/** Format a token count compactly (e.g. 1.2k, 3.4M) */
function formatTokens(count: number): string {
	if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
	if (count >= 1_000) return `${(count / 1_000).toFixed(1)}k`;
	return String(count);
}

/** Compute cache hit ratio as a percentage string */
function formatCacheHit(stats: AgentUsageStats): string {
	const c = stats.cumulative;
	const total =
		c.cache_read_input_tokens + c.cache_creation_input_tokens + c.input_tokens;
	if (total === 0) return "—";
	const ratio = c.cache_read_input_tokens / total;
	return `${(ratio * 100).toFixed(0)}%`;
}

// ---------------------------------------------------------------------------
// Row
// ---------------------------------------------------------------------------

interface AgentRowProps {
	agent: Agent;
	selected: boolean;
	onSelect: (id: string, checked: boolean) => void;
	onDelete: (id: string) => void;
	usage?: AgentUsageStats;
}

function AgentRow({
	agent,
	selected,
	onSelect,
	onDelete,
	usage,
}: AgentRowProps) {
	const navigate = useNavigate();

	const formattedDate = new Date(agent.created_at).toLocaleDateString(
		undefined,
		{
			year: "numeric",
			month: "short",
			day: "numeric",
		},
	);

	const workingDir = agent.config.working_dir;
	const displayDir =
		workingDir.length > 30 ? `…${workingDir.slice(-29)}` : workingDir;

	const dash = <span className="text-th-text-faint">—</span>;

	return (
		<tr
			className="cursor-pointer border-b border-th-border hover:bg-th-surface-hover"
			onClick={() => navigate(`/agents/${agent.id}`)}
		>
			{/* Checkbox */}
			<td
				className="w-10 px-4 py-3"
				onClick={(e) => {
					e.stopPropagation();
				}}
			>
				<input
					type="checkbox"
					aria-label={`Select agent ${agent.name}`}
					checked={selected}
					onChange={(e) => {
						onSelect(agent.id, e.target.checked);
					}}
					className="h-4 w-4 rounded border-th-border-input text-th-accent focus:ring-th-focus-ring bg-th-input"
				/>
			</td>

			{/* Name */}
			<td className="px-4 py-3 text-sm font-medium text-th-text">
				{agent.name}
			</td>

			{/* Status */}
			<td className="px-4 py-3">
				<AgentStatusBadge status={agent.status} />
			</td>

			{/* Model */}
			<td className="text-th-text-muted px-4 py-3 text-sm">
				{agent.config.model ?? (
					<span className="italic opacity-50">default</span>
				)}
			</td>

			{/* Cost */}
			<td className="hidden px-4 py-3 text-sm text-th-text-muted whitespace-nowrap lg:table-cell">
				{usage ? formatCost(usage.cumulative.total_cost_usd) : dash}
			</td>

			{/* Tokens */}
			<td className="hidden px-4 py-3 text-sm text-th-text-muted whitespace-nowrap lg:table-cell">
				{usage
					? formatTokens(
							usage.cumulative.input_tokens + usage.cumulative.output_tokens,
						)
					: dash}
			</td>

			{/* Cache Hit */}
			<td className="hidden px-4 py-3 text-sm text-th-text-muted whitespace-nowrap xl:table-cell">
				{usage ? formatCacheHit(usage) : dash}
			</td>

			{/* Working Directory */}
			<td
				className="px-4 py-3 text-sm text-th-text-muted"
				title={workingDir}
			>
				<span className="font-mono text-xs">{displayDir}</span>
			</td>

			{/* Created */}
			<td className="px-4 py-3 text-sm text-th-text-muted whitespace-nowrap">
				{formattedDate}
			</td>

			{/* Actions */}
			<td className="px-4 py-3" onClick={(e) => e.stopPropagation()}>
				<div className="flex items-center gap-1">
					<button
						type="button"
						aria-label={`View agent ${agent.name}`}
						onClick={() => navigate(`/agents/${agent.id}`)}
						className="rounded p-1.5 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text-link"
					>
						<Eye size={15} />
					</button>
					<button
						type="button"
						aria-label={`Terminate agent ${agent.name}`}
						onClick={() => onDelete(agent.id)}
						className="rounded p-1.5 text-th-text-muted hover:bg-th-status-error-bg hover:text-th-status-error-text"
					>
						<Trash2 size={15} />
					</button>
				</div>
			</td>
		</tr>
	);
}

// ---------------------------------------------------------------------------
// AgentTable
// ---------------------------------------------------------------------------

export function AgentTable({
	agents,
	loading,
	sortBy,
	sortDir,
	onSort,
	onDelete,
	onBulkDelete,
	selectedIds,
	onSelectChange,
	usageMap,
}: AgentTableProps) {
	const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
	const [bulkDeletePending, setBulkDeletePending] = useState(false);
	const [deleteLoading, setDeleteLoading] = useState(false);

	// Select / deselect helpers
	const allSelected =
		agents.length > 0 && agents.every((a) => selectedIds.includes(a.id));
	const someSelected = selectedIds.length > 0;

	function toggleAll(checked: boolean) {
		if (checked) {
			onSelectChange(agents.map((a) => a.id));
		} else {
			onSelectChange([]);
		}
	}

	function toggleOne(id: string, checked: boolean) {
		if (checked) {
			onSelectChange([...selectedIds, id]);
		} else {
			onSelectChange(selectedIds.filter((s) => s !== id));
		}
	}

	async function handleConfirmDelete() {
		setDeleteLoading(true);
		try {
			if (bulkDeletePending) {
				await onBulkDelete(selectedIds);
				onSelectChange([]);
				setBulkDeletePending(false);
			} else if (deleteTarget) {
				await onDelete(deleteTarget);
				setDeleteTarget(null);
			}
		} finally {
			setDeleteLoading(false);
		}
	}

	const confirmTitle = bulkDeletePending
		? `Terminate ${selectedIds.length} agent${selectedIds.length !== 1 ? "s" : ""}?`
		: "Terminate agent?";
	const confirmDesc = bulkDeletePending
		? `This will permanently terminate ${selectedIds.length} selected agent${selectedIds.length !== 1 ? "s" : ""}. This action cannot be undone.`
		: "This will permanently terminate this agent and all associated resources. This action cannot be undone.";

	return (
		<div className="overflow-hidden rounded-lg border border-th-border">
			{/* Bulk action toolbar */}
			{someSelected && (
				<div className="flex items-center gap-3 border-b border-th-border bg-th-accent/10 px-4 py-2.5">
					<span className="text-sm font-medium text-th-text-link">
						{selectedIds.length} selected
					</span>
					<button
						type="button"
						onClick={() => setBulkDeletePending(true)}
						className="flex items-center gap-1.5 rounded-md bg-th-status-error-dot px-3 py-1.5 text-xs font-medium text-th-accent-text hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-1"
					>
						<Trash2 size={12} />
						Terminate selected
					</button>
					<button
						type="button"
						onClick={() => onSelectChange([])}
						className="text-xs text-th-text-muted hover:text-th-text-secondary"
					>
						Clear selection
					</button>
				</div>
			)}

			<div className="overflow-x-auto">
				<table className="min-w-full divide-y divide-th-border">
					<thead className="bg-th-surface-sunken">
						<tr>
							{/* Select all */}
							<th className="w-10 px-4 py-3">
								<input
									type="checkbox"
									aria-label="Select all agents"
									checked={allSelected}
									onChange={(e) => toggleAll(e.target.checked)}
									className="h-4 w-4 rounded border-th-border-input text-th-accent focus:ring-th-focus-ring bg-th-input"
								/>
							</th>
							<th className="px-4 py-3 text-left text-xs text-th-text-muted">
								<SortHeader
									field="name"
									label="Name"
									currentSort={sortBy}
									currentDir={sortDir}
									onSort={onSort}
								/>
							</th>
							<th className="px-4 py-3 text-left text-xs text-th-text-muted">
								<SortHeader
									field="status"
									label="Status"
									currentSort={sortBy}
									currentDir={sortDir}
									onSort={onSort}
								/>
							</th>
							<th className="px-4 py-3 text-left text-xs font-medium text-th-text-muted">
								Model
							</th>
							<th className="hidden px-4 py-3 text-left text-xs text-th-text-muted lg:table-cell">
								<SortHeader
									field="cost"
									label="Cost"
									currentSort={sortBy}
									currentDir={sortDir}
									onSort={onSort}
								/>
							</th>
							<th className="hidden px-4 py-3 text-left text-xs text-th-text-muted lg:table-cell">
								<SortHeader
									field="tokens"
									label="Tokens"
									currentSort={sortBy}
									currentDir={sortDir}
									onSort={onSort}
								/>
							</th>
							<th className="hidden px-4 py-3 text-left text-xs text-th-text-muted xl:table-cell">
								<SortHeader
									field="cache"
									label="Cache Hit"
									currentSort={sortBy}
									currentDir={sortDir}
									onSort={onSort}
								/>
							</th>
							<th className="px-4 py-3 text-left text-xs font-medium text-th-text-muted">
								Working Directory
							</th>
							<th className="px-4 py-3 text-left text-xs text-th-text-muted">
								<SortHeader
									field="created_at"
									label="Created"
									currentSort={sortBy}
									currentDir={sortDir}
									onSort={onSort}
								/>
							</th>
							<th className="px-4 py-3 text-left text-xs font-medium text-th-text-muted">
								Actions
							</th>
						</tr>
					</thead>

					<tbody className="divide-y divide-th-border bg-th-surface">
						{loading ? (
							<tr>
								<td colSpan={10} className="p-4">
									<ListItemSkeleton rows={5} />
								</td>
							</tr>
						) : agents.length === 0 ? (
							<EmptyState />
						) : (
							agents.map((agent) => (
								<AgentRow
									key={agent.id}
									agent={agent}
									selected={selectedIds.includes(agent.id)}
									onSelect={toggleOne}
									onDelete={(id) => setDeleteTarget(id)}
									usage={usageMap?.get(agent.id)}
								/>
							))
						)}
					</tbody>
				</table>
			</div>

			{/* Confirmation dialog */}
			<ConfirmDialog
				open={deleteTarget !== null || bulkDeletePending}
				title={confirmTitle}
				description={confirmDesc}
				confirmLabel="Terminate"
				variant="danger"
				loading={deleteLoading}
				onConfirm={handleConfirmDelete}
				onCancel={() => {
					setDeleteTarget(null);
					setBulkDeletePending(false);
				}}
			/>
		</div>
	);
}

export default AgentTable;
