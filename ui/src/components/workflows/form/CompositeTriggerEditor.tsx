/**
 * CompositeTriggerEditor — recursive editor for the composite trigger.
 *
 * Edits the AND/OR mode, the correlation window (AND only), and a list of
 * sub-trigger drafts, each rendered with TriggerTypeSelect + TriggerFields.
 * Nesting is capped at MAX_COMPOSITE_DEPTH levels to match the backend.
 */

import { Plus, Trash2 } from "lucide-react";
import { FormField, fieldClass } from "@/components/common/form";
import {
	MAX_COMPOSITE_DEPTH,
	newTriggerDraft,
	type TriggerDraft,
} from "./triggerDraft";
import { TriggerFields } from "./TriggerFields";
import { TriggerTypeSelect } from "./TriggerTypeSelect";

export interface CompositeTriggerEditorProps {
	draft: TriggerDraft;
	onChange: (draft: TriggerDraft) => void;
	disabled?: boolean;
	/** Current nesting depth (0 = top-level composite). */
	depth?: number;
	idPrefix?: string;
}

export function CompositeTriggerEditor({
	draft,
	onChange,
	disabled,
	depth = 0,
	idPrefix = "wf",
}: CompositeTriggerEditorProps) {
	const composite = draft.composite ?? {
		mode: "or" as const,
		correlationWindowSecs: "",
		triggers: [],
	};

	function patch(patchValue: Partial<typeof composite>) {
		onChange({ ...draft, composite: { ...composite, ...patchValue } });
	}

	function setSubTrigger(index: number, sub: TriggerDraft) {
		const triggers = [...composite.triggers];
		triggers[index] = sub;
		patch({ triggers });
	}

	function removeSubTrigger(index: number) {
		patch({ triggers: composite.triggers.filter((_, i) => i !== index) });
	}

	// Sub-triggers may themselves be composite only while under the depth cap.
	const allowNestedComposite = depth + 1 < MAX_COMPOSITE_DEPTH;

	return (
		<div className="space-y-3">
			<div className="grid grid-cols-2 gap-3">
				<FormField
					htmlFor={`${idPrefix}-composite-mode`}
					label="Mode"
					help={
						composite.mode === "and"
							? "Fires when all sub-triggers produce tasks within the window."
							: "Fires as soon as any sub-trigger produces tasks."
					}
				>
					<select
						id={`${idPrefix}-composite-mode`}
						value={composite.mode}
						onChange={(e) => patch({ mode: e.target.value as "or" | "and" })}
						disabled={disabled}
						className={fieldClass()}
					>
						<option value="or">OR — any sub-trigger</option>
						<option value="and">AND — all sub-triggers</option>
					</select>
				</FormField>

				{composite.mode === "and" && (
					<FormField
						htmlFor={`${idPrefix}-composite-window`}
						label="Correlation window (seconds)"
						optional
						help="How long after the first sub-trigger fires before partial state resets. Default 60."
					>
						<input
							id={`${idPrefix}-composite-window`}
							type="number"
							min={1}
							value={composite.correlationWindowSecs}
							onChange={(e) => patch({ correlationWindowSecs: e.target.value })}
							placeholder="60"
							disabled={disabled}
							className={fieldClass()}
						/>
					</FormField>
				)}
			</div>

			<div className="space-y-3">
				{composite.triggers.map((sub, i) => (
					<fieldset
						// biome-ignore lint/suspicious/noArrayIndexKey: sub-triggers are positional and edited in place
						key={i}
						className="rounded-lg border border-th-border p-3 space-y-3"
					>
						<legend className="flex items-center gap-2 px-1 text-xs font-medium text-th-text-secondary">
							Sub-trigger {i + 1}
							<button
								type="button"
								aria-label={`Remove sub-trigger ${i + 1}`}
								onClick={() => removeSubTrigger(i)}
								disabled={disabled || composite.triggers.length <= 2}
								className="rounded p-0.5 text-th-text-muted hover:text-th-status-error-text disabled:opacity-30"
							>
								<Trash2 size={12} />
							</button>
						</legend>

						<TriggerTypeSelect
							value={sub.type}
							onChange={(type) => setSubTrigger(i, newTriggerDraft(type))}
							disabled={disabled}
							excludeComposite={!allowNestedComposite}
							id={`${idPrefix}-sub-${i}-type`}
						/>

						{sub.type === "composite" ? (
							<CompositeTriggerEditor
								draft={sub}
								onChange={(next) => setSubTrigger(i, next)}
								disabled={disabled}
								depth={depth + 1}
								idPrefix={`${idPrefix}-sub-${i}`}
							/>
						) : (
							<TriggerFields
								draft={sub}
								onChange={(next) => setSubTrigger(i, next)}
								disabled={disabled}
								idPrefix={`${idPrefix}-sub-${i}`}
							/>
						)}
					</fieldset>
				))}
			</div>

			<button
				type="button"
				onClick={() =>
					patch({
						triggers: [...composite.triggers, newTriggerDraft("manual")],
					})
				}
				disabled={disabled}
				className="flex items-center gap-1 text-xs text-th-text-link hover:opacity-80 disabled:opacity-50"
			>
				<Plus size={12} />
				Add sub-trigger
			</button>
		</div>
	);
}

export default CompositeTriggerEditor;
