/**
 * StringListEditor — add/remove rows of free-text strings.
 *
 * Used for additional directories, rooms, sandbox bypass globs, and any
 * other repeated-string config fields.
 */

import { Plus, Trash2 } from "lucide-react";
import { fieldClass } from "./FormField";

export interface StringListEditorProps {
	values: string[];
	onChange: (values: string[]) => void;
	label: string;
	placeholder?: string;
	addLabel?: string;
	disabled?: boolean;
	mono?: boolean;
}

export function StringListEditor({
	values,
	onChange,
	label,
	placeholder,
	addLabel = "Add entry",
	disabled,
	mono = true,
}: StringListEditorProps) {
	function update(index: number, value: string) {
		const next = [...values];
		next[index] = value;
		onChange(next);
	}

	function remove(index: number) {
		onChange(values.filter((_, i) => i !== index));
	}

	return (
		<div className="space-y-2">
			{values.map((value, i) => (
				// biome-ignore lint/suspicious/noArrayIndexKey: rows are positional and editable in place
				<div key={i} className="flex items-center gap-2">
					<input
						type="text"
						aria-label={`${label} ${i + 1}`}
						value={value}
						onChange={(e) => update(i, e.target.value)}
						placeholder={placeholder}
						disabled={disabled}
						className={fieldClass(undefined, mono ? "font-mono text-xs" : "")}
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
				onClick={() => onChange([...values, ""])}
				disabled={disabled}
				className="flex items-center gap-1 text-xs text-th-text-link hover:opacity-80 disabled:opacity-50"
			>
				<Plus size={12} />
				{addLabel}
			</button>
		</div>
	);
}

export default StringListEditor;
