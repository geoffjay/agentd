/**
 * WorkflowDetail — detail view for a single workflow.
 *
 * Layout:
 * - Header: name, enabled status, agent, created/updated timestamps, actions
 * - Configuration card: source config, prompt template, poll interval
 * - Dispatch history table
 */

import { ArrowLeft, Edit2, GitFork, RefreshCw, Trash2, Zap } from "lucide-react";
import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { HighlightedCode } from "@/components/common";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { CardSkeleton } from "@/components/common/LoadingSkeleton";
import { StatusBadge } from "@/components/common/StatusBadge";
import { DispatchHistory } from "@/components/workflows/DispatchHistory";
import { WorkflowForm } from "@/components/workflows/WorkflowForm";
import { useAgents } from "@/hooks/useAgents";
import { useWorkflowDetail } from "@/hooks/useWorkflows";
import type { CreateWorkflowRequest, TriggerConfig } from "@/types/orchestrator";
import { getTriggerLabel } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function sourceDetail(src: TriggerConfig | undefined): string {
	if (!src) return "No source configured";
	switch (src.type) {
		case "github_issues":
		case "github_pull_requests": {
			const parts: string[] = [`${src.owner}/${src.repo}`];
			if (src.labels.length > 0) parts.push(`Labels: ${src.labels.join(", ")}`);
			if (src.state) parts.push(`State: ${src.state}`);
			return parts.join(" · ");
		}
		case "cron":
			return `Cron: ${src.expression}`;
		case "delay":
			return `Run at: ${new Date(src.run_at).toLocaleString()}`;
		case "webhook":
			return `Webhook (${src.source})`;
		case "manual":
			return "Manual trigger";
		case "linear_issues": {
			const parts: string[] = [];
			if (src.team_key) parts.push(src.team_key);
			if (src.project) parts.push(src.project);
			return parts.length > 0 ? `Linear: ${parts.join(" / ")}` : "Linear Issues";
		}
		case "agent_lifecycle":
			return `Agent lifecycle: ${src.event}`;
		case "agent_idle":
			return `Agent idle: ${src.idle_seconds}s`;
		case "dispatch_result":
			return src.source_workflow_id
				? `Dispatch result from ${src.source_workflow_id}`
				: "Dispatch result";
		case "composite":
			return `Composite (${src.mode.toUpperCase()}, ${src.triggers.length} triggers)`;
		case "queue":
			return `Queue: ${src.queue_name}`;
		case "ask_response":
			return src.category ? `Ask response: ${src.category}` : "Ask response";
		default:
			return getTriggerLabel((src as TriggerConfig).type);
	}
}

function formatDateTime(iso: string): string {
	return new Date(iso).toLocaleString(undefined, {
		year: "numeric",
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	});
}

// ---------------------------------------------------------------------------
// Config detail card
// ---------------------------------------------------------------------------

function ConfigRow({
	label,
	value,
}: {
	label: string;
	value: React.ReactNode;
}) {
	return (
		<div className="grid grid-cols-3 gap-4 py-3 border-t border-th-border-subtle first:border-t-0">
			<dt className="text-sm font-medium text-th-text-muted">
				{label}
			</dt>
			<dd className="col-span-2 text-sm text-th-text">
				{value}
			</dd>
		</div>
	);
}

// ---------------------------------------------------------------------------
// WorkflowDetail
// ---------------------------------------------------------------------------

export function WorkflowDetail() {
	const { id } = useParams<{ id: string }>();
	const navigate = useNavigate();

	const { workflow, loading, error, refetch, updateWorkflow, deleteWorkflow } =
		useWorkflowDetail(id ?? "");

	const { allAgents } = useAgents({ pageSize: 200 });
	const [formOpen, setFormOpen] = useState(false);
	const [confirmDelete, setConfirmDelete] = useState(false);
	const [deleting, setDeleting] = useState(false);

	async function handleSave(request: CreateWorkflowRequest) {
		await updateWorkflow({
			name: request.name,
			prompt_template: request.prompt_template,
			poll_interval_secs: request.poll_interval_secs,
			enabled: request.enabled,
			tool_policy: request.tool_policy,
		});
	}

	async function handleDelete() {
		setDeleting(true);
		try {
			await deleteWorkflow();
			navigate("/workflows");
		} finally {
			setDeleting(false);
			setConfirmDelete(false);
		}
	}

	if (loading) {
		return (
			<div id="main-content" className="space-y-4">
				<CardSkeleton />
				<CardSkeleton />
			</div>
		);
	}

	if (error || !workflow) {
		return (
			<div id="main-content" className="space-y-4">
				<Link
					to="/workflows"
					className="inline-flex items-center gap-2 text-sm text-th-text-muted hover:text-th-text transition-colors"
				>
					<ArrowLeft size={16} />
					Back to Workflows
				</Link>
				<p className="text-sm text-th-status-error-text">
					{error ?? "Workflow not found."}
				</p>
			</div>
		);
	}

	const agent = allAgents.find((a) => a.id === workflow.agent_id);

	return (
		<div id="main-content" className="space-y-6">
			{/* Back nav */}
			<Link
				to="/workflows"
				className="inline-flex items-center gap-2 text-sm text-th-text-muted hover:text-th-text transition-colors focus-visible:outline-none focus-visible:ring-2 focus:ring-th-focus-ring rounded"
			>
				<ArrowLeft size={16} />
				Back to Workflows
			</Link>

			{/* Header */}
			<div className="flex items-start justify-between">
				<div className="flex items-start gap-4">
					<div className="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-xl bg-th-accent-subtle">
						<Zap size={24} className="text-th-text-link" />
					</div>
					<div>
						<div className="flex items-center gap-3">
							<h1 className="text-2xl font-semibold text-th-text">
								{workflow.name}
							</h1>
							<StatusBadge status={workflow.enabled ? "healthy" : "unknown"} />
						</div>
						<p className="mt-1 text-sm text-th-text-muted">
							Agent: {agent?.name ?? workflow.agent_id}
						</p>
					</div>
				</div>

				<div className="flex items-center gap-2">
					<button
						type="button"
						onClick={refetch}
						className="rounded-md p-2 text-th-text-muted hover:text-th-text-secondary hover:bg-th-surface-hover transition-colors"
						aria-label="Refresh"
					>
						<RefreshCw size={18} />
					</button>
					<Link
						to={`/workflows/${workflow.id}/edit`}
						data-testid="edit-in-builder-btn"
						className="inline-flex items-center gap-2 rounded-md border border-th-border-strong bg-th-surface px-3 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover transition-colors focus-visible:outline-none focus-visible:ring-2 focus:ring-th-focus-ring"
					>
						<GitFork size={15} />
						Edit in builder
					</Link>
					<button
						type="button"
						onClick={() => setFormOpen(true)}
						className="inline-flex items-center gap-2 rounded-md border border-th-border-strong bg-th-surface px-3 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover transition-colors focus-visible:outline-none focus-visible:ring-2 focus:ring-th-focus-ring"
					>
						<Edit2 size={15} />
						Edit
					</button>
					<button
						type="button"
						onClick={() => setConfirmDelete(true)}
						className="inline-flex items-center gap-2 rounded-md border border-th-status-error-border bg-th-surface px-3 py-2 text-sm font-medium text-th-status-error-text hover:opacity-90 transition-colors focus-visible:outline-none focus-visible:ring-2 focus:ring-th-focus-ring"
					>
						<Trash2 size={15} />
						Delete
					</button>
				</div>
			</div>

			{/* Configuration card */}
			<div className="rounded-lg border border-th-border bg-th-surface p-6">
				<h2 className="text-base font-semibold text-th-text mb-4">
					Configuration
				</h2>
				<dl>
					<ConfigRow
						label="Source"
						value={sourceDetail(workflow.trigger_config)}
					/>
					<ConfigRow
						label="Poll interval"
						value={
							workflow.poll_interval_secs < 60
								? `${workflow.poll_interval_secs}s`
								: `${Math.round(workflow.poll_interval_secs / 60)}m`
						}
					/>
					<ConfigRow label="Enabled" value={workflow.enabled ? "Yes" : "No"} />
					<ConfigRow
						label="Created"
						value={formatDateTime(workflow.created_at)}
					/>
					<ConfigRow
						label="Updated"
						value={formatDateTime(workflow.updated_at)}
					/>
					<ConfigRow
						label="Prompt template"
						value={
							<HighlightedCode
								code={workflow.prompt_template}
								language="markdown"
								maxHeight="8rem"
								className="border border-th-border"
							/>
						}
					/>
				</dl>
			</div>

			{/* Dispatch history */}
			<div className="rounded-lg border border-th-border bg-th-surface p-6">
				<h2 className="text-base font-semibold text-th-text mb-4">
					Dispatch history
				</h2>
				<DispatchHistory workflowId={workflow.id} />
			</div>

			{/* Edit dialog */}
			<WorkflowForm
				open={formOpen}
				workflow={workflow}
				agents={allAgents}
				onSave={handleSave}
				onClose={() => setFormOpen(false)}
			/>

			{/* Delete confirmation */}
			<ConfirmDialog
				open={confirmDelete}
				title="Delete workflow"
				description={`Delete "${workflow.name}"? This cannot be undone.`}
				confirmLabel="Delete"
				variant="danger"
				loading={deleting}
				onConfirm={handleDelete}
				onCancel={() => setConfirmDelete(false)}
			/>
		</div>
	);
}

export default WorkflowDetail;
