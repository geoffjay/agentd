/**
 * TriggerFields — generic renderer for a trigger definition's field list.
 *
 * Renders every non-composite trigger type from its TriggerFieldDef[]
 * (one switch over input kinds). Composite triggers are handled by
 * CompositeTriggerEditor instead.
 */

import { FormField, fieldClass } from "@/components/common/form";
import type { TriggerDraft } from "./triggerDraft";
import { type TriggerFieldDef, triggerDef } from "./triggerDefs";

export interface TriggerFieldsProps {
	draft: TriggerDraft;
	onChange: (draft: TriggerDraft) => void;
	disabled?: boolean;
	/** Unique prefix for input ids (supports nested composite editors). */
	idPrefix?: string;
}

function fieldId(prefix: string, key: string): string {
	return `${prefix}-trigger-${key}`;
}

function inputType(field: TriggerFieldDef): string {
	switch (field.input) {
		case "number":
			return "number";
		case "secret":
			return "password";
		case "datetime":
			return "datetime-local";
		default:
			return "text";
	}
}

export function TriggerFields({
	draft,
	onChange,
	disabled,
	idPrefix = "wf",
}: TriggerFieldsProps) {
	const def = triggerDef(draft.type);

	function setValue(key: string, value: string) {
		onChange({ ...draft, values: { ...draft.values, [key]: value } });
	}

	if (def.fields.length === 0) {
		return <p className="text-xs text-th-text-muted">{def.description}</p>;
	}

	return (
		<div className="space-y-3">
			{def.fields.map((field) => {
				const id = fieldId(idPrefix, field.key);
				const value = String(draft.values[field.key] ?? "");

				if (field.input === "select") {
					return (
						<FormField
							key={field.key}
							htmlFor={id}
							label={field.label}
							optional={!field.required}
							help={field.help}
						>
							<select
								id={id}
								value={value}
								onChange={(e) => setValue(field.key, e.target.value)}
								disabled={disabled}
								className={fieldClass()}
							>
								{field.options?.map((opt) => (
									<option key={opt.value} value={opt.value}>
										{opt.label}
									</option>
								))}
							</select>
						</FormField>
					);
				}

				return (
					<FormField
						key={field.key}
						htmlFor={id}
						label={field.label}
						optional={!field.required}
						help={field.help}
					>
						<input
							id={id}
							type={inputType(field)}
							value={value}
							onChange={(e) => setValue(field.key, e.target.value)}
							placeholder={field.placeholder}
							disabled={disabled}
							min={field.input === "number" ? 0 : undefined}
							className={fieldClass(
								undefined,
								field.input === "regex" ? "font-mono" : "",
							)}
						/>
					</FormField>
				);
			})}
		</div>
	);
}

export default TriggerFields;
