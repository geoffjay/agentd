/**
 * KeyValueEditor — add/remove rows of key=value string pairs.
 *
 * Used for agent environment variables. Extracted from the former
 * CreateAgentDialog implementation.
 */

import { Plus, Trash2 } from "lucide-react";
import { fieldClass } from "./FormField";

export interface KeyValueRow {
	key: string;
	value: string;
}

export interface KeyValueEditorProps {
	rows: KeyValueRow[];
	onChange: (rows: KeyValueRow[]) => void;
	label: string;
	keyPlaceholder?: string;
	valuePlaceholder?: string;
	addLabel?: string;
	disabled?: boolean;
}

export function KeyValueEditor({
	rows,
	onChange,
	label,
	keyPlaceholder = "KEY",
	valuePlaceholder = "value",
	addLabel = "Add variable",
	disabled,
}: KeyValueEditorProps) {
	function update(index: number, patch: Partial<KeyValueRow>) {
		const next = [...rows];
		next[index] = { ...next[index], ...patch };
		onChange(next);
	}

	function remove(index: number) {
		onChange(rows.filter((_, i) => i !== index));
	}

	return (
		<div className="space-y-2">
			{rows.map((row, i) => (
				// biome-ignore lint/suspicious/noArrayIndexKey: rows are positional and editable in place
				<div key={i} className="flex items-center gap-2">
					<input
						type="text"
						aria-label={`${label} key ${i + 1}`}
						value={row.key}
						onChange={(e) => update(i, { key: e.target.value })}
						placeholder={keyPlaceholder}
						disabled={disabled}
						className={fieldClass(undefined, "flex-1 font-mono text-xs")}
					/>
					<span className="text-th-text-muted">=</span>
					<input
						type="text"
						aria-label={`${label} value ${i + 1}`}
						value={row.value}
						onChange={(e) => update(i, { value: e.target.value })}
						placeholder={valuePlaceholder}
						disabled={disabled}
						className={fieldClass(undefined, "flex-1 font-mono text-xs")}
					/>
					<button
						type="button"
						aria-label={`Remove ${label} ${i + 1}`}
						onClick={() => remove(i)}
						disabled={disabled}
						className="rounded p-1 text-th-text-muted hover:text-th-status-error-text disabled:opacity-30"
					>
						<Trash2 size={13} />
					</button>
				</div>
			))}
			<button
				type="button"
				onClick={() => onChange([...rows, { key: "", value: "" }])}
				disabled={disabled}
				className="flex items-center gap-1 text-xs text-th-text-link hover:opacity-80 disabled:opacity-50"
			>
				<Plus size={12} />
				{addLabel}
			</button>
		</div>
	);
}

export default KeyValueEditor;
