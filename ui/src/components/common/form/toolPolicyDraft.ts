/**
 * ToolPolicyDraft — editable form representation of a ToolPolicy.
 *
 * Keeps the tool list as a comma-separated string (form convention) and
 * sandbox bypass globs as rows for the StringListEditor.
 */

import type { ToolPolicy } from "@/types/orchestrator";

export type ToolPolicyMode = ToolPolicy["mode"];

export interface ToolPolicyDraft {
	mode: ToolPolicyMode;
	/** Comma-separated tool names/patterns (allow_list / deny_list only). */
	toolsCsv: string;
	/** Sandbox bypass globs, e.g. "Bash(git-spice *)". */
	sandboxBypass: string[];
}

export const TOOL_POLICY_MODES: Array<{
	label: string;
	value: ToolPolicyMode;
}> = [
	{ label: "Allow All", value: "allow_all" },
	{ label: "Deny All", value: "deny_all" },
	{ label: "Allow List", value: "allow_list" },
	{ label: "Deny List", value: "deny_list" },
	{ label: "Require Approval", value: "require_approval" },
];

export const DEFAULT_POLICY_DRAFT: ToolPolicyDraft = {
	mode: "allow_all",
	toolsCsv: "",
	sandboxBypass: [],
};

export function policyDraftFromPolicy(policy: ToolPolicy): ToolPolicyDraft {
	return {
		mode: policy.mode,
		toolsCsv: "tools" in policy ? policy.tools.join(", ") : "",
		sandboxBypass: policy.sandbox_bypass ?? [],
	};
}

export function policyFromDraft(draft: ToolPolicyDraft): ToolPolicy {
	const sandbox_bypass = draft.sandboxBypass
		.map((s) => s.trim())
		.filter(Boolean);
	const bypass = sandbox_bypass.length > 0 ? { sandbox_bypass } : {};

	if (draft.mode === "allow_list" || draft.mode === "deny_list") {
		const tools = draft.toolsCsv
			.split(",")
			.map((t) => t.trim())
			.filter(Boolean);
		return { mode: draft.mode, tools, ...bypass };
	}
	return { mode: draft.mode, ...bypass };
}
