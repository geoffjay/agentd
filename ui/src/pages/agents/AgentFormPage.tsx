/**
 * AgentFormPage — dedicated page for creating (/agents/new) and editing
 * (/agents/:id/edit) an agent.
 *
 * Create POSTs CreateAgentRequest; edit PATCHes UpdateAgentRequest with
 * merge semantics (redacted env values round-trip via the "***"
 * sentinel). After an edit that changes launch-affecting fields the
 * response flags requires_restart and the page offers a restart.
 */

import { AlertTriangle, ArrowLeft, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { AgentForm } from "@/components/agents/form/AgentForm";
import {
	type AgentFormErrors,
	type AgentFormState,
	agentFormFromAgent,
	agentToCreateRequest,
	agentToUpdateRequest,
	DEFAULT_AGENT_FORM,
	hasAgentErrors,
	validateAgentForm,
} from "@/components/agents/form/agentFormModel";
import { CardSkeleton } from "@/components/common/LoadingSkeleton";
import { orchestratorClient } from "@/services/orchestrator";

export function AgentFormPage() {
	const { id } = useParams<{ id: string }>();
	const navigate = useNavigate();
	const editing = Boolean(id);

	const [state, setState] = useState<AgentFormState>(DEFAULT_AGENT_FORM);
	const [errors, setErrors] = useState<AgentFormErrors>({});
	const [loading, setLoading] = useState(editing);
	const [loadError, setLoadError] = useState<string | undefined>();
	const [submitting, setSubmitting] = useState(false);
	const [requiresRestart, setRequiresRestart] = useState(false);
	const [restarting, setRestarting] = useState(false);

	// Load the agent when editing.
	useEffect(() => {
		if (!id) return;
		let cancelled = false;
		(async () => {
			try {
				const agent = await orchestratorClient.getAgent(id);
				if (!cancelled) setState(agentFormFromAgent(agent));
			} catch (err) {
				if (!cancelled)
					setLoadError(
						err instanceof Error ? err.message : "Failed to load agent",
					);
			} finally {
				if (!cancelled) setLoading(false);
			}
		})();
		return () => {
			cancelled = true;
		};
	}, [id]);

	const onChange = useCallback(
		<K extends keyof AgentFormState>(key: K, value: AgentFormState[K]) => {
			setState((prev) => ({ ...prev, [key]: value }));
			setErrors((prev) => ({ ...prev, [key]: undefined, general: undefined }));
		},
		[],
	);

	async function handleSubmit(e: React.FormEvent) {
		e.preventDefault();
		const errs = validateAgentForm(state, { editing });
		if (hasAgentErrors(errs)) {
			setErrors(errs);
			return;
		}

		setSubmitting(true);
		setErrors({});
		try {
			if (editing && id) {
				const result = await orchestratorClient.updateAgent(
					id,
					agentToUpdateRequest(state),
				);
				if (result.requires_restart) {
					setRequiresRestart(true);
				} else {
					navigate(`/agents/${id}`);
				}
			} else {
				const created = await orchestratorClient.createAgent(
					agentToCreateRequest(state),
				);
				navigate(`/agents/${created.id}`);
			}
		} catch (err) {
			setErrors({
				general:
					err instanceof Error
						? err.message
						: `Failed to ${editing ? "update" : "create"} agent`,
			});
		} finally {
			setSubmitting(false);
		}
	}

	async function handleRestart() {
		if (!id) return;
		setRestarting(true);
		try {
			await orchestratorClient.restartAgent(id);
			navigate(`/agents/${id}`);
		} catch (err) {
			setErrors({
				general: err instanceof Error ? err.message : "Failed to restart agent",
			});
		} finally {
			setRestarting(false);
		}
	}

	const backTarget = editing && id ? `/agents/${id}` : "/agents";

	if (loading) {
		return (
			<div id="main-content" className="mx-auto max-w-3xl space-y-5">
				<CardSkeleton />
			</div>
		);
	}

	if (loadError) {
		return (
			<div id="main-content" className="mx-auto max-w-3xl space-y-5">
				<div className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					{loadError}
				</div>
				<Link to="/agents" className="text-sm text-th-text-link">
					Back to Agents
				</Link>
			</div>
		);
	}

	return (
		<div id="main-content" className="mx-auto max-w-3xl space-y-5">
			<Link
				to={backTarget}
				className="inline-flex items-center gap-2 text-sm text-th-text-muted hover:text-th-text-secondary"
			>
				<ArrowLeft size={16} />
				{editing ? "Back to agent" : "Back to Agents"}
			</Link>

			<div>
				<h1 className="text-2xl font-semibold text-th-text">
					{editing ? `Edit ${state.name || "agent"}` : "Create Agent"}
				</h1>
				<p className="mt-1 text-sm text-th-text-muted">
					{editing
						? "Changes are saved to the agent's stored config; launch-affecting changes apply on restart."
						: "Configure and launch a new agent."}
				</p>
			</div>

			{errors.general && (
				<div className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					{errors.general}
				</div>
			)}

			{requiresRestart && (
				<div
					role="status"
					className="flex items-center justify-between gap-3 rounded-md border border-th-status-warning-border bg-th-status-warning-bg px-4 py-3 text-sm text-th-status-warning-text"
				>
					<span className="flex items-start gap-2">
						<AlertTriangle
							size={15}
							className="mt-0.5 shrink-0"
							aria-hidden="true"
						/>
						Saved. The running process still uses the old configuration —
						restart the agent to apply the changes.
					</span>
					<div className="flex shrink-0 items-center gap-2">
						<button
							type="button"
							onClick={handleRestart}
							disabled={restarting}
							className="inline-flex items-center gap-1.5 rounded-md bg-th-accent px-3 py-1.5 text-xs font-medium text-th-accent-text hover:bg-th-accent-hover disabled:opacity-50"
						>
							<RefreshCw
								size={12}
								className={restarting ? "animate-spin" : ""}
							/>
							{restarting ? "Restarting…" : "Restart now"}
						</button>
						<button
							type="button"
							onClick={() => navigate(`/agents/${id}`)}
							className="rounded-md border border-th-border-strong px-3 py-1.5 text-xs font-medium text-th-text-secondary hover:bg-th-surface-hover"
						>
							Later
						</button>
					</div>
				</div>
			)}

			<form onSubmit={handleSubmit} noValidate>
				<AgentForm
					state={state}
					errors={errors}
					onChange={onChange}
					disabled={submitting}
					editing={editing}
				/>

				<div className="mt-5 flex justify-end gap-3">
					<button
						type="button"
						onClick={() => navigate(backTarget)}
						disabled={submitting}
						className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover focus:outline-none focus:ring-2 focus:ring-th-focus-ring disabled:opacity-50"
					>
						Cancel
					</button>
					<button
						type="submit"
						disabled={submitting}
						className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover focus:outline-none focus:ring-2 focus:ring-th-focus-ring disabled:opacity-50 transition-colors"
					>
						{submitting
							? editing
								? "Saving…"
								: "Creating…"
							: editing
								? "Save changes"
								: "Create Agent"}
					</button>
				</div>
			</form>
		</div>
	);
}

export default AgentFormPage;
