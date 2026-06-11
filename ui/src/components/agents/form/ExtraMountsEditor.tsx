/**
 * ExtraMountsEditor — rows of host/container path pairs with a read-only
 * toggle, for Docker-backed agents.
 */

import { Plus, Trash2 } from "lucide-react";
import { fieldClass } from "@/components/common/form";
import type { MountRow } from "./agentFormModel";

export interface ExtraMountsEditorProps {
	mounts: MountRow[];
	onChange: (mounts: MountRow[]) => void;
	disabled?: boolean;
}

export function ExtraMountsEditor({
	mounts,
	onChange,
	disabled,
}: ExtraMountsEditorProps) {
	function update(index: number, patch: Partial<MountRow>) {
		const next = [...mounts];
		next[index] = { ...next[index], ...patch };
		onChange(next);
	}

	return (
		<div className="space-y-2">
			{mounts.map((mount, i) => (
				// biome-ignore lint/suspicious/noArrayIndexKey: rows are positional and editable in place
				<div key={i} className="flex items-center gap-2">
					<input
						type="text"
						aria-label={`Mount host path ${i + 1}`}
						value={mount.hostPath}
						onChange={(e) => update(i, { hostPath: e.target.value })}
						placeholder="/host/path"
						disabled={disabled}
						className={fieldClass(undefined, "flex-1 font-mono text-xs")}
					/>
					<span className="text-th-text-muted">→</span>
					<input
						type="text"
						aria-label={`Mount container path ${i + 1}`}
						value={mount.containerPath}
						onChange={(e) => update(i, { containerPath: e.target.value })}
						placeholder="/container/path"
						disabled={disabled}
						className={fieldClass(undefined, "flex-1 font-mono text-xs")}
					/>
					<label className="flex items-center gap-1 text-xs text-th-text-muted whitespace-nowrap">
						<input
							type="checkbox"
							checked={mount.readOnly}
							onChange={(e) => update(i, { readOnly: e.target.checked })}
							disabled={disabled}
						/>
						ro
					</label>
					<button
						type="button"
						aria-label={`Remove mount ${i + 1}`}
						onClick={() => onChange(mounts.filter((_, idx) => idx !== i))}
						disabled={disabled}
						className="rounded p-1 text-th-text-muted hover:text-th-status-error-text disabled:opacity-30"
					>
						<Trash2 size={13} />
					</button>
				</div>
			))}
			<button
				type="button"
				onClick={() =>
					onChange([
						...mounts,
						{ hostPath: "", containerPath: "", readOnly: false },
					])
				}
				disabled={disabled}
				className="flex items-center gap-1 text-xs text-th-text-link hover:opacity-80 disabled:opacity-50"
			>
				<Plus size={12} />
				Add mount
			</button>
		</div>
	);
}

export default ExtraMountsEditor;
