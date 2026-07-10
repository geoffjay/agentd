/**
 * AgentForm — full agent configuration form body.
 *
 * Sections: Identity, Execution, System Prompt, Tool Policy, Workspace,
 * and Advanced (env, auto-clear, user).
 * State lives in the parent page via the agentFormModel draft; this
 * component is purely presentational over that draft.
 */

import { AlertTriangle, ChevronDown, ChevronRight } from "lucide-react";
import { type ReactNode, useState } from "react";
import { HighlightedCode } from "@/components/common";
import {
	FormField,
	fieldClass,
	KeyValueEditor,
	StringListEditor,
	ToggleSwitch,
	ToolPolicyFields,
} from "@/components/common/form";
import type { AgentFormErrors, AgentFormState } from "./agentFormModel";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MODEL_SUGGESTIONS = ["sonnet", "opus", "haiku", "claude-sonnet-4-6"];

// ---------------------------------------------------------------------------
// Section
// ---------------------------------------------------------------------------

function Section({ title, children }: { title: string; children: ReactNode }) {
	return (
		<section className="rounded-lg border border-th-border bg-th-surface p-5">
			<h2 className="mb-4 text-sm font-semibold text-th-text">{title}</h2>
			<div className="space-y-4">{children}</div>
		</section>
	);
}

function CollapsibleSection({
	title,
	subtitle,
	children,
}: {
	title: string;
	subtitle?: string;
	children: ReactNode;
}) {
	const [open, setOpen] = useState(false);
	return (
		<section className="rounded-lg border border-th-border bg-th-surface">
			<button
				type="button"
				aria-expanded={open}
				onClick={() => setOpen((v) => !v)}
				className="flex w-full items-center justify-between px-5 py-4 text-sm font-semibold text-th-text hover:bg-th-surface-hover"
			>
				<span>
					{title}
					{subtitle && (
						<span className="ml-2 text-xs font-normal text-th-text-faint">
							{subtitle}
						</span>
					)}
				</span>
				{open ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
			</button>
			{open && (
				<div className="space-y-4 border-t border-th-border px-5 py-4">
					{children}
				</div>
			)}
		</section>
	);
}

function ToggleRow({
	label,
	description,
	checked,
	onChange,
	disabled,
}: {
	label: string;
	description?: string;
	checked: boolean;
	onChange: (v: boolean) => void;
	disabled?: boolean;
}) {
	return (
		<div className="flex items-center justify-between gap-4">
			<div className="flex-1">
				<span className="block text-sm font-medium text-th-text-secondary">
					{label}
				</span>
				{description && (
					<span className="mt-0.5 block text-xs text-th-text-faint">
						{description}
					</span>
				)}
			</div>
			<ToggleSwitch
				checked={checked}
				onChange={onChange}
				label={label}
				disabled={disabled}
			/>
		</div>
	);
}

// ---------------------------------------------------------------------------
// AgentForm
// ---------------------------------------------------------------------------

export interface AgentFormProps {
	state: AgentFormState;
	errors: AgentFormErrors;
	onChange: <K extends keyof AgentFormState>(
		key: K,
		value: AgentFormState[K],
	) => void;
	disabled?: boolean;
	/** Edit mode: name changes allowed, but show interactive as fixed. */
	editing?: boolean;
}

export function AgentForm({
	state,
	errors,
	onChange,
	disabled,
	editing,
}: AgentFormProps) {
	return (
		<div className="space-y-5">
			{/* Identity */}
			<Section title="Identity">
				<FormField htmlFor="agent-name" label="Name" error={errors.name}>
					<input
						id="agent-name"
						type="text"
						required
						value={state.name}
						onChange={(e) => onChange("name", e.target.value)}
						placeholder="my-agent"
						disabled={disabled}
						className={fieldClass(errors.name)}
					/>
				</FormField>

				<FormField
					htmlFor="agent-working-dir"
					label="Working directory"
					error={errors.workingDir}
					help="Resolved on the orchestrator host."
				>
					<input
						id="agent-working-dir"
						type="text"
						required
						value={state.workingDir}
						onChange={(e) => onChange("workingDir", e.target.value)}
						placeholder="/home/user/project"
						disabled={disabled}
						className={fieldClass(errors.workingDir, "font-mono")}
					/>
				</FormField>
			</Section>

			{/* Execution */}
			<Section title="Execution">
				<FormField
					htmlFor="agent-model"
					label="Model"
					optional
					help="Alias (sonnet, opus, haiku) or a full model name. Empty inherits the default."
				>
					<input
						id="agent-model"
						type="text"
						list="agent-model-suggestions"
						value={state.model}
						onChange={(e) => onChange("model", e.target.value)}
						placeholder="Default"
						disabled={disabled}
						className={fieldClass()}
					/>
					<datalist id="agent-model-suggestions">
						{MODEL_SUGGESTIONS.map((m) => (
							<option key={m} value={m} />
						))}
					</datalist>
				</FormField>

				<FormField htmlFor="agent-shell" label="Shell" optional>
					<input
						id="agent-shell"
						type="text"
						value={state.shell}
						onChange={(e) => onChange("shell", e.target.value)}
						placeholder="/bin/zsh"
						disabled={disabled}
						className={fieldClass(undefined, "font-mono")}
					/>
				</FormField>

				<ToggleRow
					label="Interactive mode"
					description="Runs Claude without the SDK protocol. The Terminal tab becomes the primary interface; cost tracking and tool policies are unavailable."
					checked={state.interactive}
					onChange={(v) => onChange("interactive", v)}
					disabled={disabled || editing}
				/>
				{state.interactive && (
					<div
						role="note"
						className="flex items-start gap-2 rounded-md border border-th-status-warning-border bg-th-status-warning-bg px-3 py-2 text-xs text-th-status-warning-text"
					>
						<AlertTriangle
							size={13}
							className="mt-0.5 shrink-0"
							aria-hidden="true"
						/>
						<span>
							Interactive mode enabled. Cost tracking and tool policies will not
							be available, and the initial prompt field is disabled.
						</span>
					</div>
				)}

				<ToggleRow
					label="Git worktree"
					description="Start the session with --worktree so the agent works on an isolated copy of the repository."
					checked={state.worktree}
					onChange={(v) => onChange("worktree", v)}
					disabled={disabled}
				/>

				{!state.interactive && (
					<FormField htmlFor="agent-prompt" label="Initial prompt" optional>
						<textarea
							id="agent-prompt"
							rows={3}
							value={state.prompt}
							onChange={(e) => onChange("prompt", e.target.value)}
							placeholder="Initial prompt for the agent…"
							disabled={disabled}
							className={fieldClass(undefined, "resize-none")}
						/>
					</FormField>
				)}
			</Section>

			{/* System prompt */}
			<Section title="System Prompt">
				<div className="flex items-center gap-4 text-sm">
					<label className="flex items-center gap-1.5 text-th-text-secondary">
						<input
							type="radio"
							name="agent-system-prompt-mode"
							checked={state.systemPromptMode === "inline"}
							onChange={() => onChange("systemPromptMode", "inline")}
							disabled={disabled}
						/>
						Inline text
					</label>
					<label className="flex items-center gap-1.5 text-th-text-secondary">
						<input
							type="radio"
							name="agent-system-prompt-mode"
							checked={state.systemPromptMode === "file"}
							onChange={() => onChange("systemPromptMode", "file")}
							disabled={disabled}
						/>
						From file
					</label>
				</div>

				{state.systemPromptMode === "inline" ? (
					<FormField
						htmlFor="agent-system-prompt"
						label="System prompt"
						optional
					>
						<textarea
							id="agent-system-prompt"
							rows={4}
							value={state.systemPrompt}
							onChange={(e) => onChange("systemPrompt", e.target.value)}
							placeholder="System prompt override…"
							disabled={disabled}
							className={fieldClass(undefined, "resize-none")}
						/>
						{state.systemPrompt.trim().length > 0 && (
							<div className="mt-2">
								<p className="mb-1 text-xs text-th-text-muted">Preview</p>
								<HighlightedCode
									code={state.systemPrompt}
									language="markdown"
									maxHeight="10rem"
									className="border border-th-border"
								/>
							</div>
						)}
					</FormField>
				) : (
					<FormField
						htmlFor="agent-system-prompt-file"
						label="System prompt file"
						optional
						help="Path on the orchestrator host; validated and canonicalized server-side."
					>
						<input
							id="agent-system-prompt-file"
							type="text"
							value={state.systemPromptFile}
							onChange={(e) => onChange("systemPromptFile", e.target.value)}
							placeholder="/path/to/prompt.md"
							disabled={disabled}
							className={fieldClass(undefined, "font-mono")}
						/>
					</FormField>
				)}

				<ToggleRow
					label="Append instead of replace"
					description="Append to Claude Code's default system prompt rather than replacing it."
					checked={state.appendSystemPrompt}
					onChange={(v) => onChange("appendSystemPrompt", v)}
					disabled={disabled}
				/>
			</Section>

			{/* Tool policy */}
			{!state.interactive && (
				<Section title="Tool Policy">
					<ToolPolicyFields
						draft={state.toolPolicy}
						onChange={(toolPolicy) => onChange("toolPolicy", toolPolicy)}
						disabled={disabled}
						idPrefix="agent-policy"
					/>
				</Section>
			)}

			{/* Workspace */}
			<Section title="Workspace">
				<div>
					<span className="mb-1 block text-sm font-medium text-th-text-secondary">
						Additional directories{" "}
						<span className="text-xs font-normal text-th-text-faint">
							(optional)
						</span>
					</span>
					<p className="mb-2 text-xs text-th-text-faint">
						Extra paths the agent may access (maps to --add-dir).
					</p>
					<StringListEditor
						values={state.additionalDirs}
						onChange={(additionalDirs) =>
							onChange("additionalDirs", additionalDirs)
						}
						label="Additional directory"
						placeholder="/path/to/dir"
						addLabel="Add directory"
						disabled={disabled}
					/>
				</div>

				<div>
					<span className="mb-1 block text-sm font-medium text-th-text-secondary">
						Rooms{" "}
						<span className="text-xs font-normal text-th-text-faint">
							(optional)
						</span>
					</span>
					<p className="mb-2 text-xs text-th-text-faint">
						Communicate rooms the agent auto-joins when it connects.
					</p>
					<StringListEditor
						values={state.rooms}
						onChange={(rooms) => onChange("rooms", rooms)}
						label="Room"
						placeholder="engineering"
						addLabel="Add room"
						disabled={disabled}
						mono={false}
					/>
				</div>
			</Section>

			{/* Advanced */}
			<CollapsibleSection title="Advanced">
				<FormField
					htmlFor="agent-auto-clear"
					label="Auto-clear threshold"
					optional
					error={errors.autoClearThreshold}
					help="Automatically clear context when cumulative input tokens exceed this. Empty disables."
				>
					<input
						id="agent-auto-clear"
						type="number"
						min={1}
						step={1}
						value={state.autoClearThreshold}
						onChange={(e) => onChange("autoClearThreshold", e.target.value)}
						placeholder="e.g. 100000"
						disabled={disabled}
						className={fieldClass(errors.autoClearThreshold)}
					/>
				</FormField>

				<FormField
					htmlFor="agent-user"
					label="OS user"
					optional
					help="Run the agent as this user (via sudo)."
				>
					<input
						id="agent-user"
						type="text"
						value={state.user}
						onChange={(e) => onChange("user", e.target.value)}
						placeholder="current user"
						disabled={disabled}
						className={fieldClass(undefined, "font-mono")}
					/>
				</FormField>

				<div>
					<span className="mb-1 block text-sm font-medium text-th-text-secondary">
						Environment variables{" "}
						<span className="text-xs font-normal text-th-text-faint">
							(optional)
						</span>
					</span>
					{editing && (
						<p className="mb-2 text-xs text-th-text-faint">
							Existing values show as "***"; leave them untouched to keep the
							stored secrets.
						</p>
					)}
					<KeyValueEditor
						rows={state.env}
						onChange={(env) => onChange("env", env)}
						label="Environment variable"
						disabled={disabled}
					/>
					{errors.env && (
						<p className="mt-1 text-xs text-th-status-error-text">
							{errors.env}
						</p>
					)}
				</div>
			</CollapsibleSection>
		</div>
	);
}

export default AgentForm;
