/**
 * RetryDispatchModal — re-trigger a workflow dispatch with editable variables.
 *
 * Opened by clicking a dispatch entry in the dispatch history table. Renders
 * one input per {{variable}} found in the workflow's prompt template,
 * prefilled from the task persisted on the dispatch being retried (when
 * available), plus a live preview of the rendered prompt.
 */

import { X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { orchestratorClient } from "@/services/orchestrator";
import { toastStore } from "@/stores/toastStore";
import { ApiError } from "@/types/common";
import type {
	DispatchRecord,
	Task,
	TriggerWorkflowRequest,
	Workflow,
} from "@/types/orchestrator";
import {
	extractTemplateVariables,
	renderTemplatePreview,
} from "@/utils/templateVariables";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface RetryDispatchModalProps {
	open: boolean;
	workflow: Workflow;
	dispatch: DispatchRecord | null;
	onClose: () => void;
	/** Called with the newly created dispatch after a successful re-trigger. */
	onRetried: (dispatch: DispatchRecord) => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Metadata keys injected by the retry flow itself; excluded from prefill. */
const INTERNAL_METADATA_KEYS = new Set(["retry_of"]);

function errorMessage(err: unknown): string {
	if (err instanceof ApiError) {
		switch (err.status) {
			case 400:
				return "Workflow is disabled — enable it before re-triggering.";
			case 409:
				return "Agent is busy with another task. Try again when the current dispatch completes.";
			case 503:
				return "Agent is not connected. Start the agent and try again.";
			default:
				return err.message;
		}
	}
	return err instanceof Error ? err.message : "Failed to trigger workflow";
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function RetryDispatchModal({
	open,
	workflow,
	dispatch,
	onClose,
	onRetried,
}: RetryDispatchModalProps) {
	const firstFieldRef = useRef<HTMLInputElement | HTMLTextAreaElement>(null);

	// Form state — top-level task fields plus named metadata variables.
	const [title, setTitle] = useState("");
	const [body, setBody] = useState("");
	const [url, setUrl] = useState("");
	const [labelsRaw, setLabelsRaw] = useState("");
	const [assignee, setAssignee] = useState("");
	const [metadataValues, setMetadataValues] = useState<Record<string, string>>(
		{},
	);
	const [newMetaKey, setNewMetaKey] = useState("");
	const [newMetaValue, setNewMetaValue] = useState("");
	const [submitting, setSubmitting] = useState(false);
	const [submitError, setSubmitError] = useState<string | undefined>();

	const variables = useMemo(
		() => extractTemplateVariables(workflow.prompt_template),
		[workflow.prompt_template],
	);
	const hasVariable = (name: string) => variables.some((v) => v.name === name);
	const metadataVariables = variables.filter((v) => v.kind === "metadata");
	const usesMetadataMap = variables.some((v) => v.kind === "metadata-map");

	const originalTask = dispatch?.task ?? null;

	// Prefill from the dispatch being retried whenever the modal opens.
	useEffect(() => {
		if (!open) return;
		setTitle(originalTask?.title ?? "");
		setBody(originalTask?.body ?? "");
		setUrl(originalTask?.url ?? "");
		setLabelsRaw((originalTask?.labels ?? []).join(", "));
		setAssignee(originalTask?.assignee ?? "");
		setMetadataValues(
			Object.fromEntries(
				Object.entries(originalTask?.metadata ?? {}).filter(
					([key]) => !INTERNAL_METADATA_KEYS.has(key),
				),
			),
		);
		setNewMetaKey("");
		setNewMetaValue("");
		setSubmitError(undefined);
		setSubmitting(false);
		setTimeout(() => firstFieldRef.current?.focus(), 50);
	}, [open, originalTask]);

	// Close on Escape
	useEffect(() => {
		if (!open) return;
		function handleKeyDown(e: KeyboardEvent) {
			if (e.key === "Escape") onClose();
		}
		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, [open, onClose]);

	if (!open || !dispatch) return null;

	const labels = labelsRaw
		.split(",")
		.map((l) => l.trim())
		.filter(Boolean);

	// Live preview of the rendered prompt using the current form values.
	const previewTask: Task = {
		source_id: "manual:<generated>",
		title: title || "Manual trigger",
		body,
		url,
		labels,
		assignee: assignee || undefined,
		metadata: metadataValues,
	};
	const preview = renderTemplatePreview(workflow.prompt_template, previewTask);

	async function handleSubmit() {
		setSubmitting(true);
		setSubmitError(undefined);
		try {
			const request: TriggerWorkflowRequest = {
				...(title.trim() ? { title: title.trim() } : {}),
				...(body ? { body } : {}),
				...(url.trim() ? { url: url.trim() } : {}),
				...(labels.length > 0 ? { labels } : {}),
				...(assignee.trim() ? { assignee: assignee.trim() } : {}),
				metadata: {
					...metadataValues,
					retry_of: dispatch!.id,
				},
			};
			const created = await orchestratorClient.triggerWorkflow(
				workflow.id,
				request,
			);
			toastStore.success("Workflow re-triggered", {
				message: `Dispatch ${created.source_id} created`,
			});
			onRetried(created);
			onClose();
		} catch (err) {
			setSubmitError(errorMessage(err));
		} finally {
			setSubmitting(false);
		}
	}

	const editableCount =
		Number(hasVariable("title")) +
		Number(hasVariable("body")) +
		Number(hasVariable("url")) +
		Number(hasVariable("labels")) +
		Number(hasVariable("assignee")) +
		metadataVariables.length +
		Number(usesMetadataMap);

	return (
		<div
			className="fixed inset-0 z-50 flex items-center justify-center p-4"
			role="dialog"
			aria-modal="true"
			aria-labelledby="retry-dispatch-title"
		>
			{/* Backdrop */}
			<div
				className="absolute inset-0 bg-th-overlay"
				onClick={onClose}
				aria-hidden="true"
			/>

			{/* Panel */}
			<div className="relative z-10 w-full max-w-2xl max-h-[90vh] overflow-y-auto rounded-xl bg-th-surface shadow-xl">
				{/* Header */}
				<div className="sticky top-0 z-10 flex items-center justify-between border-b border-th-border bg-th-surface px-6 py-4">
					<h2
						id="retry-dispatch-title"
						className="text-lg font-semibold text-th-text"
					>
						Re-trigger Workflow
					</h2>
					<button
						type="button"
						onClick={onClose}
						className="rounded p-1 text-th-text-muted hover:text-th-text-secondary focus-visible:outline-none focus-visible:ring-2 focus:ring-th-focus-ring"
						aria-label="Close dialog"
					>
						<X size={18} />
					</button>
				</div>

				{/* Body */}
				<div className="px-6 py-5 space-y-5">
					<p className="text-sm text-th-text-muted">
						Re-runs <span className="font-medium">{workflow.name}</span> as a
						new manual dispatch. Original dispatch:{" "}
						<span className="font-mono text-xs">{dispatch.source_id}</span>
					</p>

					{submitError && (
						<p className="rounded-md bg-th-status-error-bg px-3 py-2 text-sm text-th-status-error-text">
							{submitError}
						</p>
					)}

					{!originalTask && (
						<p className="rounded-md bg-th-status-warning-bg px-3 py-2 text-sm text-th-status-warning-text">
							Original input values were not recorded for this dispatch. Fill in
							the variables below to re-run it.
						</p>
					)}

					{editableCount === 0 && (
						<p className="text-sm text-th-text-faint">
							This workflow's prompt has no editable variables — it will be
							re-run as-is.
						</p>
					)}

					{/* Title */}
					{hasVariable("title") && (
						<div>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								Title{" "}
								<span className="font-mono text-xs text-th-text-faint">
									{"{{title}}"}
								</span>
							</label>
							<input
								ref={firstFieldRef as React.RefObject<HTMLInputElement>}
								type="text"
								value={title}
								onChange={(e) => setTitle(e.target.value)}
								placeholder="Manual trigger"
								className={fieldClass()}
							/>
						</div>
					)}

					{/* Body */}
					{hasVariable("body") && (
						<div>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								Body{" "}
								<span className="font-mono text-xs text-th-text-faint">
									{"{{body}}"}
								</span>
							</label>
							<textarea
								value={body}
								onChange={(e) => setBody(e.target.value)}
								rows={4}
								className={fieldClass("", "font-mono text-xs leading-relaxed")}
							/>
						</div>
					)}

					{/* URL */}
					{hasVariable("url") && (
						<div>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								URL{" "}
								<span className="font-mono text-xs text-th-text-faint">
									{"{{url}}"}
								</span>
							</label>
							<input
								type="text"
								value={url}
								onChange={(e) => setUrl(e.target.value)}
								className={fieldClass()}
							/>
						</div>
					)}

					{/* Labels */}
					{hasVariable("labels") && (
						<div>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								Labels{" "}
								<span className="font-mono text-xs text-th-text-faint">
									{"{{labels}}"}
								</span>
							</label>
							<input
								type="text"
								value={labelsRaw}
								onChange={(e) => setLabelsRaw(e.target.value)}
								placeholder="bug, urgent (comma-separated)"
								className={fieldClass()}
							/>
						</div>
					)}

					{/* Assignee */}
					{hasVariable("assignee") && (
						<div>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								Assignee{" "}
								<span className="font-mono text-xs text-th-text-faint">
									{"{{assignee}}"}
								</span>
							</label>
							<input
								type="text"
								value={assignee}
								onChange={(e) => setAssignee(e.target.value)}
								className={fieldClass()}
							/>
						</div>
					)}

					{/* Named metadata variables */}
					{metadataVariables.map((variable) => (
						<div key={variable.name}>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								{variable.name}{" "}
								<span className="font-mono text-xs text-th-text-faint">
									{`{{${variable.name}}}`}
								</span>
							</label>
							<input
								type="text"
								value={metadataValues[variable.name] ?? ""}
								onChange={(e) =>
									setMetadataValues((prev) => ({
										...prev,
										[variable.name]: e.target.value,
									}))
								}
								className={fieldClass()}
							/>
						</div>
					))}

					{/* Free-form metadata editor (template uses the whole {{metadata}} map) */}
					{usesMetadataMap && (
						<fieldset className="rounded-lg border border-th-border p-4 space-y-3">
							<legend className="text-sm font-medium text-th-text-secondary px-1">
								Metadata{" "}
								<span className="font-mono text-xs text-th-text-faint">
									{"{{metadata}}"}
								</span>
							</legend>
							{Object.entries(metadataValues)
								.filter(
									([key]) => !metadataVariables.some((v) => v.name === key),
								)
								.map(([key, value]) => (
									<div key={key} className="flex items-center gap-2">
										<span className="w-1/3 truncate font-mono text-xs text-th-text-muted">
											{key}
										</span>
										<input
											type="text"
											value={value}
											onChange={(e) =>
												setMetadataValues((prev) => ({
													...prev,
													[key]: e.target.value,
												}))
											}
											className={fieldClass()}
										/>
										<button
											type="button"
											onClick={() =>
												setMetadataValues((prev) => {
													const next = { ...prev };
													delete next[key];
													return next;
												})
											}
											className="rounded p-1 text-th-text-muted hover:text-th-status-error-text"
											aria-label={`Remove metadata key ${key}`}
										>
											<X size={14} />
										</button>
									</div>
								))}
							<div className="flex items-center gap-2">
								<input
									type="text"
									value={newMetaKey}
									onChange={(e) => setNewMetaKey(e.target.value)}
									placeholder="key"
									className={fieldClass("", "w-1/3")}
								/>
								<input
									type="text"
									value={newMetaValue}
									onChange={(e) => setNewMetaValue(e.target.value)}
									placeholder="value"
									className={fieldClass()}
								/>
								<button
									type="button"
									onClick={() => {
										const key = newMetaKey.trim();
										if (!key) return;
										setMetadataValues((prev) => ({
											...prev,
											[key]: newMetaValue,
										}));
										setNewMetaKey("");
										setNewMetaValue("");
									}}
									className="rounded-md border border-th-border-strong px-2 py-1 text-xs font-medium text-th-text-secondary hover:bg-th-surface-hover"
								>
									Add
								</button>
							</div>
						</fieldset>
					)}

					{/* Prompt preview */}
					<div>
						<label className="block text-sm font-medium text-th-text-secondary mb-1">
							Prompt preview
						</label>
						<pre className="max-h-48 overflow-y-auto rounded-md border border-th-border bg-th-surface-sunken px-3 py-2 text-xs font-mono text-th-text-secondary whitespace-pre-wrap">
							{preview}
						</pre>
					</div>
				</div>

				{/* Footer */}
				<div className="sticky bottom-0 flex items-center justify-end gap-3 border-t border-th-border bg-th-surface px-6 py-4">
					<button
						type="button"
						onClick={onClose}
						disabled={submitting}
						className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover transition-colors disabled:opacity-50"
					>
						Cancel
					</button>
					<button
						type="button"
						onClick={handleSubmit}
						disabled={submitting}
						className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors disabled:opacity-50"
					>
						{submitting ? "Triggering…" : "Re-trigger"}
					</button>
				</div>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

function fieldClass(error?: string, extra = ""): string {
	return [
		"w-full rounded-md border px-3 py-2 text-sm",
		"bg-th-input",
		"text-th-text",
		"focus:outline-none focus:ring-2 focus:ring-th-focus-ring",
		"disabled:cursor-not-allowed disabled:opacity-50",
		error ? "border-th-status-error-border" : "border-th-border-input",
		extra,
	]
		.filter(Boolean)
		.join(" ");
}

export default RetryDispatchModal;
