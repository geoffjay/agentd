/**
 * agentFormModel — pure form-state model for the agent form page.
 *
 * Draft type, defaults, conversions to/from API requests, and validation.
 * Shared by the page component, tests, and the YAML import/export.
 */

import {
	DEFAULT_POLICY_DRAFT,
	type KeyValueRow,
	policyDraftFromPolicy,
	policyFromDraft,
	type ToolPolicyDraft,
} from "@/components/common/form";
import type {
	Agent,
	CreateAgentRequest,
	UpdateAgentRequest,
} from "@/types/orchestrator";
import { ENV_REDACTED } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export interface AgentFormState {
	name: string;
	workingDir: string;
	model: string;
	shell: string;
	user: string;
	interactive: boolean;
	worktree: boolean;
	prompt: string;
	/** Which system prompt source is active. */
	systemPromptMode: "inline" | "file";
	systemPrompt: string;
	systemPromptFile: string;
	appendSystemPrompt: boolean;
	toolPolicy: ToolPolicyDraft;
	env: KeyValueRow[];
	/** String for the number input; empty = disabled. */
	autoClearThreshold: string;
	additionalDirs: string[];
	rooms: string[];
}

export interface AgentFormErrors {
	name?: string;
	workingDir?: string;
	autoClearThreshold?: string;
	env?: string;
	general?: string;
}

export const DEFAULT_AGENT_FORM: AgentFormState = {
	name: "",
	workingDir: "",
	model: "",
	shell: "",
	user: "",
	interactive: false,
	worktree: false,
	prompt: "",
	systemPromptMode: "inline",
	systemPrompt: "",
	systemPromptFile: "",
	appendSystemPrompt: false,
	toolPolicy: DEFAULT_POLICY_DRAFT,
	env: [],
	autoClearThreshold: "",
	additionalDirs: [],
	rooms: [],
};

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

export function agentFormFromAgent(agent: Agent): AgentFormState {
	const config = agent.config;
	return {
		name: agent.name,
		workingDir: config.working_dir,
		model: config.model ?? "",
		shell: config.shell ?? "",
		user: config.user ?? "",
		interactive: config.interactive,
		worktree: config.worktree ?? false,
		prompt: config.prompt ?? "",
		systemPromptMode: config.system_prompt_file ? "file" : "inline",
		systemPrompt: config.system_prompt ?? "",
		systemPromptFile: config.system_prompt_file ?? "",
		appendSystemPrompt: config.append_system_prompt ?? false,
		toolPolicy: policyDraftFromPolicy(config.tool_policy),
		env: Object.entries(config.env ?? {}).map(([key, value]) => ({
			key,
			value,
		})),
		autoClearThreshold:
			config.auto_clear_threshold != null
				? String(config.auto_clear_threshold)
				: "",
		additionalDirs: config.additional_dirs ?? [],
		rooms: config.rooms ?? [],
	};
}

function envRecord(state: AgentFormState): Record<string, string> | undefined {
	const env: Record<string, string> = {};
	for (const row of state.env) {
		if (row.key.trim()) env[row.key.trim()] = row.value;
	}
	return Object.keys(env).length > 0 ? env : undefined;
}

function parsedThreshold(state: AgentFormState): number | undefined {
	const raw = state.autoClearThreshold.trim();
	if (!raw) return undefined;
	const n = Number.parseInt(raw, 10);
	return !Number.isNaN(n) && n > 0 ? n : undefined;
}

function cleanList(values: string[]): string[] {
	return values.map((s) => s.trim()).filter(Boolean);
}

export function agentToCreateRequest(
	state: AgentFormState,
): CreateAgentRequest {
	const inline = state.systemPromptMode === "inline";
	const dirs = cleanList(state.additionalDirs);
	const rooms = cleanList(state.rooms);
	return {
		name: state.name.trim(),
		working_dir: state.workingDir.trim(),
		user: state.user.trim() || undefined,
		shell: state.shell.trim() || "/bin/sh",
		interactive: state.interactive,
		prompt: state.interactive ? undefined : state.prompt.trim() || undefined,
		worktree: state.worktree || undefined,
		system_prompt: inline ? state.systemPrompt.trim() || undefined : undefined,
		system_prompt_file: !inline
			? state.systemPromptFile.trim() || undefined
			: undefined,
		append_system_prompt: state.appendSystemPrompt || undefined,
		tool_policy: policyFromDraft(state.toolPolicy),
		model: state.model || undefined,
		env: envRecord(state),
		auto_clear_threshold: parsedThreshold(state),
		additional_dirs: dirs.length > 0 ? dirs : undefined,
		rooms: rooms.length > 0 ? rooms : undefined,
	};
}

/**
 * Build the PATCH body for an edit.
 *
 * All editable fields are sent; the redaction sentinel keeps untouched env
 * secrets server-side, and empty strings explicitly clear the prompt
 * fields (the API treats empty string as "clear").
 */
export function agentToUpdateRequest(
	state: AgentFormState,
	options?: { restart?: boolean },
): UpdateAgentRequest {
	const inline = state.systemPromptMode === "inline";
	const env: Record<string, string> = {};
	for (const row of state.env) {
		if (row.key.trim()) env[row.key.trim()] = row.value;
	}
	return {
		name: state.name.trim(),
		working_dir: state.workingDir.trim(),
		shell: state.shell.trim() || "/bin/sh",
		prompt: state.interactive ? "" : state.prompt.trim(),
		// Setting one non-empty clears the other server-side; sending the
		// inactive field as "" makes the clear explicit either way.
		system_prompt: inline ? state.systemPrompt.trim() : "",
		system_prompt_file: inline ? "" : state.systemPromptFile.trim(),
		append_system_prompt: state.appendSystemPrompt,
		model: state.model || undefined,
		tool_policy: policyFromDraft(state.toolPolicy),
		env,
		auto_clear_threshold: parsedThreshold(state),
		additional_dirs: cleanList(state.additionalDirs),
		rooms: cleanList(state.rooms),
		worktree: state.worktree,
		restart: options?.restart ?? false,
	};
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

export function validateAgentForm(
	state: AgentFormState,
	options?: { editing?: boolean },
): AgentFormErrors {
	const errors: AgentFormErrors = {};

	if (!state.name.trim()) errors.name = "Name is required.";
	if (!state.workingDir.trim())
		errors.workingDir = "Working directory is required.";

	const threshold = state.autoClearThreshold.trim();
	if (threshold) {
		const n = Number.parseInt(threshold, 10);
		if (Number.isNaN(n) || n <= 0 || !Number.isInteger(Number(threshold))) {
			errors.autoClearThreshold = "Must be a positive integer.";
		}
	}

	// Catch redaction placeholders typed into newly added env rows: the API
	// rejects them (there is no stored value to preserve). Rows loaded from
	// an existing agent legitimately carry the sentinel.
	if (!options?.editing) {
		const hasSentinel = state.env.some(
			(row) => row.key.trim() && row.value === ENV_REDACTED,
		);
		if (hasSentinel)
			errors.env = `"${ENV_REDACTED}" is the redaction placeholder and cannot be used as a value.`;
	}

	return errors;
}

export function hasAgentErrors(errors: AgentFormErrors): boolean {
	return Object.values(errors).some(Boolean);
}
