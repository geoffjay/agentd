/**
 * WorkflowFormPage — dedicated page for creating (/workflows/new) and
 * editing (/workflows/:id/edit) a workflow.
 *
 * Supports every trigger type via the trigger registry; the prompt
 * template editor shows the variables provided by the selected trigger.
 */

import { ArrowLeft } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
	FormField,
	fieldClass,
	ToggleSwitch,
	ToolPolicyFields,
} from "@/components/common/form";
import { CardSkeleton } from "@/components/common/LoadingSkeleton";
import { YamlPanel } from "@/components/templates/YamlPanel";
import { CompositeTriggerEditor } from "@/components/workflows/form/CompositeTriggerEditor";
import { TriggerFields } from "@/components/workflows/form/TriggerFields";
import { TriggerTypeSelect } from "@/components/workflows/form/TriggerTypeSelect";
import {
	triggerDef,
	variablesFor,
} from "@/components/workflows/form/triggerDefs";
import {
	draftToConfig,
	newTriggerDraft,
	type TriggerDraft,
} from "@/components/workflows/form/triggerDraft";
import {
	DEFAULT_WORKFLOW_FORM,
	hasWorkflowErrors,
	validateWorkflowForm,
	type WorkflowFormErrors,
	type WorkflowFormState,
	workflowFormFromWorkflow,
	workflowToCreateRequest,
	workflowToUpdateRequest,
} from "@/components/workflows/form/workflowFormModel";
import { PromptTemplateEditor } from "@/components/workflows/PromptTemplateEditor";
import { useAgents } from "@/hooks/useAgents";
import { orchestratorClient } from "@/services/orchestrator";
import type { TriggerType } from "@/types/orchestrator";
import {
	exportWorkflowYaml,
	importWorkflowYaml,
} from "@/utils/yamlTemplates/workflowTemplate";

export function WorkflowFormPage() {
	const { id } = useParams<{ id: string }>();
	const navigate = useNavigate();
	const editing = Boolean(id);

	const [state, setState] = useState<WorkflowFormState>(DEFAULT_WORKFLOW_FORM);
	const [errors, setErrors] = useState<WorkflowFormErrors>({});
	const [loading, setLoading] = useState(editing);
	const [loadError, setLoadError] = useState<string | undefined>();
	const [submitting, setSubmitting] = useState(false);
	const [saveError, setSaveError] = useState<string | undefined>();
	// Per-type cache so switching trigger types doesn't destroy edits.
	const [triggerCache, setTriggerCache] = useState<
		Partial<Record<TriggerType, TriggerDraft>>
	>({});

	const { allAgents } = useAgents({ pageSize: 200, paused: true });

	// Load the workflow when editing.
	useEffect(() => {
		if (!id) return;
		let cancelled = false;
		(async () => {
			try {
				const workflow = await orchestratorClient.getWorkflow(id);
				if (!cancelled) setState(workflowFormFromWorkflow(workflow));
			} catch (err) {
				if (!cancelled)
					setLoadError(
						err instanceof Error ? err.message : "Failed to load workflow",
					);
			} finally {
				if (!cancelled) setLoading(false);
			}
		})();
		return () => {
			cancelled = true;
		};
	}, [id]);

	const set = useCallback(
		<K extends keyof WorkflowFormState>(
			key: K,
			value: WorkflowFormState[K],
		) => {
			setState((prev) => ({ ...prev, [key]: value }));
			setErrors((prev) => ({ ...prev, [key]: undefined, trigger: undefined }));
			setSaveError(undefined);
		},
		[],
	);

	function switchTriggerType(type: TriggerType) {
		setTriggerCache((prev) => ({
			...prev,
			[state.trigger.type]: state.trigger,
		}));
		set("trigger", triggerCache[type] ?? newTriggerDraft(type));
	}

	const def = triggerDef(state.trigger.type);
	const variables = useMemo(
		() => variablesFor(draftToConfig(state.trigger)),
		[state.trigger],
	);

	const yamlExport = useMemo(
		() => exportWorkflowYaml(state, allAgents),
		[state, allAgents],
	);

	const handleYamlImport = useCallback(
		(text: string) => {
			const result = importWorkflowYaml(text, allAgents);
			setState(result.state);
			setErrors({});
			return result.warnings;
		},
		[allAgents],
	);

	const runningAgents = allAgents.filter((a) => a.status === "running");
	const otherAgents = allAgents.filter((a) => a.status !== "running");
	const selectedAgent = allAgents.find((a) => a.id === state.agentId);
	const selectedNotRunning =
		Boolean(selectedAgent) && selectedAgent?.status !== "running";

	async function handleSubmit(e: React.FormEvent) {
		e.preventDefault();
		const errs = validateWorkflowForm(state);
		if (hasWorkflowErrors(errs)) {
			setErrors(errs);
			return;
		}

		setSubmitting(true);
		setSaveError(undefined);
		try {
			if (editing && id) {
				await orchestratorClient.updateWorkflow(
					id,
					workflowToUpdateRequest(state),
				);
				navigate(`/workflows/${id}`);
			} else {
				const created = await orchestratorClient.createWorkflow(
					workflowToCreateRequest(state),
				);
				navigate(`/workflows/${created.id}`);
			}
		} catch (err) {
			setSaveError(
				err instanceof Error
					? err.message
					: `Failed to ${editing ? "update" : "create"} workflow`,
			);
		} finally {
			setSubmitting(false);
		}
	}

	const backTarget = editing && id ? `/workflows/${id}` : "/workflows";

	if (loading) {
		return (
			<div id="main-content" className="space-y-5">
				<CardSkeleton />
			</div>
		);
	}

	if (loadError) {
		return (
			<div id="main-content" className="space-y-5">
				<div className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					{loadError}
				</div>
				<Link to="/workflows" className="text-sm text-th-text-link">
					Back to Workflows
				</Link>
			</div>
		);
	}

	return (
		<div id="main-content" className="space-y-5">
			<Link
				to={backTarget}
				className="inline-flex items-center gap-2 text-sm text-th-text-muted hover:text-th-text-secondary"
			>
				<ArrowLeft size={16} />
				{editing ? "Back to workflow" : "Back to Workflows"}
			</Link>

			<div>
				<h1 className="text-2xl font-semibold text-th-text">
					{editing ? `Edit ${state.name || "workflow"}` : "Create Workflow"}
				</h1>
				<p className="mt-1 text-sm text-th-text-muted">
					Workflows watch a trigger and dispatch rendered prompts to an agent.
				</p>
			</div>

			{saveError && (
				<div className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					{saveError}
				</div>
			)}

			<div className="grid grid-cols-1 gap-5 xl:grid-cols-[minmax(0,1fr)_minmax(0,26rem)]">
				<form onSubmit={handleSubmit} noValidate className="space-y-5">
					{/* Basics */}
					<section className="rounded-lg border border-th-border bg-th-surface p-5 space-y-4">
						<h2 className="text-sm font-semibold text-th-text">Basics</h2>

						<FormField
							htmlFor="wf-name"
							label="Workflow name"
							error={errors.name}
						>
							<input
								id="wf-name"
								type="text"
								required
								value={state.name}
								onChange={(e) => set("name", e.target.value)}
								placeholder="e.g. Dispatch GitHub Issues"
								disabled={submitting}
								className={fieldClass(errors.name)}
							/>
						</FormField>

						<FormField
							htmlFor="wf-agent"
							label="Agent"
							error={errors.agentId}
							help={
								selectedNotRunning
									? undefined
									: "The agent that receives dispatched tasks. Must be running."
							}
						>
							<select
								id="wf-agent"
								value={state.agentId}
								onChange={(e) => set("agentId", e.target.value)}
								disabled={submitting}
								className={fieldClass(errors.agentId)}
							>
								<option value="">Select an agent…</option>
								{runningAgents.length > 0 && (
									<optgroup label="Running">
										{runningAgents.map((a) => (
											<option key={a.id} value={a.id}>
												{a.name}
											</option>
										))}
									</optgroup>
								)}
								{otherAgents.length > 0 && (
									<optgroup label="Not running">
										{otherAgents.map((a) => (
											<option key={a.id} value={a.id}>
												{a.name} ({a.status})
											</option>
										))}
									</optgroup>
								)}
							</select>
							{selectedNotRunning && (
								<p className="mt-1 text-xs text-th-status-warning-text">
									{selectedAgent?.name} is {selectedAgent?.status}; the
									orchestrator only accepts workflows for running agents. Start
									it first.
								</p>
							)}
							{allAgents.length === 0 && (
								<p className="mt-1 text-xs text-th-status-warning-text">
									No agents found. Create an agent first.
								</p>
							)}
						</FormField>
					</section>

					{/* Trigger */}
					<section className="rounded-lg border border-th-border bg-th-surface p-5 space-y-4">
						<h2 className="text-sm font-semibold text-th-text">Trigger</h2>

						<TriggerTypeSelect
							value={state.trigger.type}
							onChange={switchTriggerType}
							disabled={submitting}
						/>

						{state.trigger.type === "composite" ? (
							<CompositeTriggerEditor
								draft={state.trigger}
								onChange={(trigger) => set("trigger", trigger)}
								disabled={submitting}
							/>
						) : (
							<TriggerFields
								draft={state.trigger}
								onChange={(trigger) => set("trigger", trigger)}
								disabled={submitting}
							/>
						)}

						{errors.trigger && errors.trigger.length > 0 && (
							<ul className="space-y-1 rounded-md bg-th-status-error-bg px-3 py-2 text-xs text-th-status-error-text">
								{errors.trigger.map((msg) => (
									<li key={msg}>{msg}</li>
								))}
							</ul>
						)}

						{def.polls && (
							<FormField
								htmlFor="wf-poll-interval"
								label="Poll interval (minutes)"
								error={errors.pollMinutes}
							>
								<input
									id="wf-poll-interval"
									type="number"
									min={1}
									value={state.pollMinutes}
									onChange={(e) => set("pollMinutes", e.target.value)}
									disabled={submitting}
									className={fieldClass(errors.pollMinutes)}
								/>
							</FormField>
						)}
					</section>

					{/* Prompt */}
					<section className="rounded-lg border border-th-border bg-th-surface p-5 space-y-4">
						<h2 className="text-sm font-semibold text-th-text">
							Prompt template
						</h2>
						<PromptTemplateEditor
							value={state.promptTemplate}
							onChange={(promptTemplate) =>
								set("promptTemplate", promptTemplate)
							}
							disabled={submitting}
							error={errors.promptTemplate}
							variables={variables}
						/>
					</section>

					{/* Policy + enabled */}
					<section className="rounded-lg border border-th-border bg-th-surface p-5 space-y-4">
						<h2 className="text-sm font-semibold text-th-text">
							Tool policy override
						</h2>
						<p className="text-xs text-th-text-faint">
							Applied to the agent while it works on tasks from this workflow.
						</p>
						<ToolPolicyFields
							draft={state.toolPolicy}
							onChange={(toolPolicy) => set("toolPolicy", toolPolicy)}
							disabled={submitting}
							idPrefix="wf-policy"
						/>

						<div className="flex items-center justify-between border-t border-th-border pt-4">
							<div>
								<span className="block text-sm font-medium text-th-text-secondary">
									Enabled
								</span>
								<span className="mt-0.5 block text-xs text-th-text-faint">
									Disabled workflows keep their config but never fire.
								</span>
							</div>
							<ToggleSwitch
								checked={state.enabled}
								onChange={(enabled) => set("enabled", enabled)}
								label="Enabled"
								disabled={submitting}
							/>
						</div>
					</section>

					<div className="flex justify-end gap-3">
						<button
							type="button"
							onClick={() => navigate(backTarget)}
							disabled={submitting}
							className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover transition-colors disabled:opacity-50"
						>
							Cancel
						</button>
						<button
							type="submit"
							disabled={submitting}
							className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors disabled:opacity-50"
						>
							{submitting
								? "Saving…"
								: editing
									? "Save changes"
									: "Create workflow"}
						</button>
					</div>
				</form>

				<YamlPanel
					title="Workflow template"
					exportedYaml={yamlExport.yaml}
					exportWarnings={yamlExport.warnings}
					onImport={handleYamlImport}
					disabled={submitting}
				/>
			</div>
		</div>
	);
}

export default WorkflowFormPage;
