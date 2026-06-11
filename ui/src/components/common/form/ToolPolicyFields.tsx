/**
 * ToolPolicyFields — form section for editing a ToolPolicy.
 *
 * Mode select, tool list (for allow/deny lists), and sandbox bypass globs.
 */

import { FormField, fieldClass } from "./FormField";
import { StringListEditor } from "./StringListEditor";
import {
	TOOL_POLICY_MODES,
	type ToolPolicyDraft,
	type ToolPolicyMode,
} from "./toolPolicyDraft";

export interface ToolPolicyFieldsProps {
	draft: ToolPolicyDraft;
	onChange: (draft: ToolPolicyDraft) => void;
	disabled?: boolean;
	idPrefix?: string;
}

export function ToolPolicyFields({
	draft,
	onChange,
	disabled,
	idPrefix = "policy",
}: ToolPolicyFieldsProps) {
	const showToolList =
		draft.mode === "allow_list" || draft.mode === "deny_list";

	return (
		<div className="space-y-3">
			<FormField htmlFor={`${idPrefix}-mode`} label="Mode">
				<select
					id={`${idPrefix}-mode`}
					value={draft.mode}
					onChange={(e) =>
						onChange({ ...draft, mode: e.target.value as ToolPolicyMode })
					}
					disabled={disabled}
					className={fieldClass()}
				>
					{TOOL_POLICY_MODES.map((m) => (
						<option key={m.value} value={m.value}>
							{m.label}
						</option>
					))}
				</select>
			</FormField>

			{showToolList && (
				<FormField
					htmlFor={`${idPrefix}-tools`}
					label="Tools"
					help='Comma-separated tool names or patterns, e.g. "Bash(git push --force*), Write(.agentd/agents/*)".'
				>
					<input
						id={`${idPrefix}-tools`}
						type="text"
						value={draft.toolsCsv}
						onChange={(e) => onChange({ ...draft, toolsCsv: e.target.value })}
						placeholder="Bash, Read, Write"
						disabled={disabled}
						className={fieldClass(undefined, "font-mono text-xs")}
					/>
				</FormField>
			)}

			<div>
				<span className="mb-1 block text-sm font-medium text-th-text-secondary">
					Sandbox bypass globs{" "}
					<span className="text-xs font-normal text-th-text-faint">
						(optional)
					</span>
				</span>
				<p className="mb-2 text-xs text-th-text-faint">
					Matching Bash calls are auto-approved with the Claude Code sandbox
					disabled, e.g. "Bash(git-spice *)".
				</p>
				<StringListEditor
					values={draft.sandboxBypass}
					onChange={(sandboxBypass) => onChange({ ...draft, sandboxBypass })}
					label="Sandbox bypass glob"
					placeholder="Bash(git-spice *)"
					addLabel="Add glob"
					disabled={disabled}
				/>
			</div>
		</div>
	);
}

export default ToolPolicyFields;
