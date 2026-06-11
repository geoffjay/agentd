/**
 * PromptTemplateEditor — textarea with variable placeholder hints and live preview.
 *
 * Shows available template variables ({{title}}, {{body}}, etc.) and
 * renders a preview substituting sample values so the user can see
 * how the prompt will look when dispatched.
 */

import { ChevronDown, ChevronUp, Info } from "lucide-react";
import { useState } from "react";
import { HighlightedCode } from "@/components/common";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TemplateVariableHint {
	/** Variable name without braces, e.g. "title". */
	name: string;
	description: string;
	sample: string;
}

export interface PromptTemplateEditorProps {
	value: string;
	onChange: (value: string) => void;
	disabled?: boolean;
	error?: string;
	/**
	 * Variable hints to display and substitute in the preview. Defaults to
	 * the base task variables; the workflow form passes the selected
	 * trigger's variables.
	 */
	variables?: TemplateVariableHint[];
	/** Placeholder / preview fallback template. */
	defaultTemplate?: string;
}

// ---------------------------------------------------------------------------
// Template variable definitions
// ---------------------------------------------------------------------------

const TEMPLATE_VARS: TemplateVariableHint[] = [
	{ name: "title", description: "Task title", sample: "Fix login bug" },
	{
		name: "body",
		description: "Task body / description",
		sample: "Users cannot log in with SSO...",
	},
	{
		name: "url",
		description: "Source URL",
		sample: "https://github.com/owner/repo/issues/42",
	},
	{
		name: "labels",
		description: "Comma-separated labels",
		sample: "bug, high-priority",
	},
	{
		name: "source_id",
		description: "Source identifier (e.g. issue number)",
		sample: "42",
	},
];

const DEFAULT_TEMPLATE = `You are working on the following task:\n\nTitle: {{title}}\n\nDescription:\n{{body}}\n\nSource: {{url}}\nLabels: {{labels}}\n\nPlease work on this task and report back when complete.`;

/** Render a template with sample values for preview */
function renderPreview(
	template: string,
	variables: TemplateVariableHint[],
): string {
	let result = template;
	for (const v of variables) {
		result = result.replaceAll(`{{${v.name}}}`, v.sample);
	}
	return result;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function PromptTemplateEditor({
	value,
	onChange,
	disabled = false,
	error,
	variables = TEMPLATE_VARS,
	defaultTemplate = DEFAULT_TEMPLATE,
}: PromptTemplateEditorProps) {
	const [showPreview, setShowPreview] = useState(false);
	const [showVars, setShowVars] = useState(false);

	function insertVariable(varName: string) {
		// Append variable at cursor position. As a simple fallback, append at end.
		onChange(value + varName);
	}

	const preview = renderPreview(value || defaultTemplate, variables);
	const hasValue = value.trim().length > 0;

	return (
		<div className="space-y-2">
			{/* Textarea */}
			<textarea
				value={value}
				onChange={(e) => onChange(e.target.value)}
				disabled={disabled}
				rows={6}
				placeholder={defaultTemplate}
				className={[
					"w-full rounded-md border px-3 py-2 text-sm font-mono",
					"bg-th-input",
					"text-th-text",
					"placeholder:text-th-text-faint",
					"focus:outline-none focus:ring-2 focus:ring-th-focus-ring",
					"disabled:cursor-not-allowed disabled:opacity-50",
					error ? "border-th-status-error-border" : "border-th-border-input",
				].join(" ")}
			/>
			{error && <p className="text-xs text-th-status-error-text">{error}</p>}

			{/* Available variables */}
			<div className="rounded-md border border-th-border overflow-hidden">
				<button
					type="button"
					onClick={() => setShowVars((v) => !v)}
					className="flex w-full items-center justify-between px-3 py-2 text-xs text-th-text-muted hover:bg-th-surface-hover transition-colors"
				>
					<span className="flex items-center gap-1.5">
						<Info size={12} />
						Available variables
					</span>
					{showVars ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
				</button>

				{showVars && (
					<div className="border-t border-th-border bg-th-surface-sunken px-3 py-2">
						<div className="flex flex-wrap gap-2">
							{variables.map((v) => (
								<button
									key={v.name}
									type="button"
									onClick={() => insertVariable(`{{${v.name}}}`)}
									disabled={disabled}
									title={`${v.description} — click to insert`}
									className="inline-flex items-center gap-1 rounded border border-th-border bg-th-surface px-2 py-0.5 font-mono text-xs text-th-text-link hover:bg-th-accent-subtle transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
								>
									{`{{${v.name}}}`}
								</button>
							))}
						</div>
						<table className="mt-2 w-full text-xs">
							<tbody>
								{variables.map((v) => (
									<tr key={v.name} className="border-t border-th-border-subtle">
										<td className="py-1 pr-3 font-mono text-th-text-link whitespace-nowrap">
											{`{{${v.name}}}`}
										</td>
										<td className="py-1 pr-3 text-th-text-muted">
											{v.description}
										</td>
										<td className="py-1 text-th-text-faint italic">
											{v.sample}
										</td>
									</tr>
								))}
							</tbody>
						</table>
					</div>
				)}
			</div>

			{/* Preview */}
			{hasValue && (
				<div className="rounded-md border border-th-border overflow-hidden">
					<button
						type="button"
						onClick={() => setShowPreview((v) => !v)}
						className="flex w-full items-center justify-between px-3 py-2 text-xs text-th-text-muted hover:bg-th-surface-hover transition-colors"
					>
						<span>Preview with sample data</span>
						{showPreview ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
					</button>

					{showPreview && (
						<div className="border-t border-th-border">
							<HighlightedCode
								code={preview}
								language="markdown"
								maxHeight="12rem"
							/>
						</div>
					)}
				</div>
			)}
		</div>
	);
}

export default PromptTemplateEditor;
