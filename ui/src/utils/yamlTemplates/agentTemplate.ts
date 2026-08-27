/**
 * Agent YAML template import/export.
 *
 * Mirrors the `AgentTemplate` shape consumed by `agent apply`
 * (crates/cli/src/commands/apply.rs): the same field names and defaults,
 * operating on the form state so the form and the YAML can never drift.
 *
 * Import never hard-fails past a YAML parse error — unsupported or
 * unknown constructs are surfaced as warnings instead.
 */

import { parse, stringify } from "yaml";
import {
	type AgentFormState,
	DEFAULT_AGENT_FORM,
} from "@/components/agents/form/agentFormModel";
import {
	DEFAULT_POLICY_DRAFT,
	policyDraftFromPolicy,
	policyFromDraft,
	type ToolPolicyDraft,
} from "@/components/common/form";
import type { ToolPolicy } from "@/types/orchestrator";
import { ENV_REDACTED } from "@/types/orchestrator";
import { findEnvSubstitutions } from "./envSubstitution";

export interface YamlImportResult<T> {
	state: T;
	warnings: string[];
}

export interface YamlExportResult {
	yaml: string;
	warnings: string[];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const AGENT_TEMPLATE_KEYS = new Set([
	"name",
	"working_dir",
	"shell",
	"interactive",
	"worktree",
	"prompt",
	"user",
	"system_prompt",
	"system_prompt_file",
	"append_system_prompt",
	"tool_policy",
	"model",
	"env",
	"auto_clear_threshold",
	"additional_dirs",
	"rooms",
]);

function asString(value: unknown): string {
	return typeof value === "string" ? value : value != null ? String(value) : "";
}

function asBool(value: unknown): boolean {
	return value === true;
}

function policyDraft(value: unknown): ToolPolicyDraft {
	if (value && typeof value === "object" && "mode" in value) {
		return policyDraftFromPolicy(value as ToolPolicy);
	}
	return DEFAULT_POLICY_DRAFT;
}

function warnUnknownKeys(
	raw: Record<string, unknown>,
	known: Set<string>,
	warnings: string[],
) {
	const unknown = Object.keys(raw).filter((k) => !known.has(k));
	if (unknown.length > 0) {
		warnings.push(`Ignored unknown field(s): ${unknown.join(", ")}.`);
	}
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

/**
 * Parse an `.agentd/agents/*.yml` template into form state.
 * Throws on invalid YAML; everything else degrades to warnings.
 */
export function importAgentYaml(
	text: string,
): YamlImportResult<AgentFormState> {
	const raw = parse(text);
	if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
		throw new Error("Template must be a YAML mapping.");
	}
	const doc = raw as Record<string, unknown>;
	const warnings: string[] = [];
	warnUnknownKeys(doc, AGENT_TEMPLATE_KEYS, warnings);

	// Rooms: entries may be plain names or {name, role} objects; the API only
	// stores names.
	const rooms: string[] = [];
	if (Array.isArray(doc.rooms)) {
		let droppedRole = false;
		for (const entry of doc.rooms) {
			if (typeof entry === "string") {
				rooms.push(entry);
			} else if (entry && typeof entry === "object" && "name" in entry) {
				rooms.push(asString((entry as Record<string, unknown>).name));
				if ("role" in entry) droppedRole = true;
			}
		}
		if (droppedRole) {
			warnings.push(
				"Room roles are not supported by the API and were dropped; only room names are kept.",
			);
		}
	}

	const env: Array<{ key: string; value: string }> = [];
	if (doc.env && typeof doc.env === "object") {
		for (const [key, value] of Object.entries(
			doc.env as Record<string, unknown>,
		)) {
			env.push({ key, value: asString(value) });
		}
	}
	const substitutions = findEnvSubstitutions(env.map((row) => row.value));
	if (substitutions.length > 0) {
		warnings.push(
			`Environment substitution (${substitutions.join(", ")}) is performed by \`agent apply\`, not the UI — the literal text will be sent to the API.`,
		);
	}

	if (typeof doc.system_prompt_file === "string" && doc.system_prompt_file) {
		warnings.push(
			"system_prompt_file paths are resolved on the orchestrator host, not relative to the template file.",
		);
	}
	if (typeof doc.user === "string" && doc.user) {
		warnings.push(
			"The OS user field is applied at agent launch; ensure the orchestrator host has sudo access for it.",
		);
	}

	const state: AgentFormState = {
		...DEFAULT_AGENT_FORM,
		name: asString(doc.name),
		// Template defaults mirror apply.rs: working_dir "." and shell "zsh".
		workingDir: asString(doc.working_dir) || ".",
		shell: asString(doc.shell) || "zsh",
		model: asString(doc.model),
		user: asString(doc.user),
		interactive: asBool(doc.interactive),
		worktree: asBool(doc.worktree),
		prompt: asString(doc.prompt),
		systemPromptMode: doc.system_prompt_file ? "file" : "inline",
		systemPrompt: asString(doc.system_prompt),
		systemPromptFile: asString(doc.system_prompt_file),
		appendSystemPrompt: asBool(doc.append_system_prompt),
		toolPolicy: policyDraft(doc.tool_policy),
		env,
		autoClearThreshold:
			doc.auto_clear_threshold != null
				? asString(doc.auto_clear_threshold)
				: "",
		additionalDirs: Array.isArray(doc.additional_dirs)
			? doc.additional_dirs.map(asString)
			: [],
		rooms,
	};

	return { state, warnings };
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/**
 * Serialize form state as an `.agentd/agents/*.yml` template, omitting
 * empty and default values for clean output.
 */
export function exportAgentYaml(state: AgentFormState): YamlExportResult {
	const warnings: string[] = [];

	const env: Record<string, string> = {};
	let redacted = false;
	for (const row of state.env) {
		if (!row.key.trim()) continue;
		env[row.key.trim()] = row.value;
		if (row.value === ENV_REDACTED) redacted = true;
	}
	if (redacted) {
		warnings.push(
			`Env values shown as "${ENV_REDACTED}" are redacted by the API — fill in real values or use \${VAR} substitution before running \`agent apply\`.`,
		);
	}

	const inline = state.systemPromptMode === "inline";

	const threshold = Number.parseInt(state.autoClearThreshold, 10);
	const dirs = state.additionalDirs.map((d) => d.trim()).filter(Boolean);
	const rooms = state.rooms.map((r) => r.trim()).filter(Boolean);

	const doc: Record<string, unknown> = {
		name: state.name.trim(),
		working_dir: state.workingDir.trim() || ".",
		...(state.shell.trim() ? { shell: state.shell.trim() } : {}),
		...(state.interactive ? { interactive: true } : {}),
		...(state.worktree ? { worktree: true } : {}),
		...(state.prompt.trim() && !state.interactive
			? { prompt: state.prompt.trim() }
			: {}),
		...(state.user.trim() ? { user: state.user.trim() } : {}),
		...(inline && state.systemPrompt.trim()
			? { system_prompt: state.systemPrompt }
			: {}),
		...(!inline && state.systemPromptFile.trim()
			? { system_prompt_file: state.systemPromptFile.trim() }
			: {}),
		...(state.appendSystemPrompt ? { append_system_prompt: true } : {}),
		tool_policy: policyFromDraft(state.toolPolicy),
		...(state.model.trim() ? { model: state.model.trim() } : {}),
		...(Object.keys(env).length > 0 ? { env } : {}),
		...(Number.isNaN(threshold) || threshold <= 0
			? {}
			: { auto_clear_threshold: threshold }),
		...(dirs.length > 0 ? { additional_dirs: dirs } : {}),
		...(rooms.length > 0 ? { rooms } : {}),
	};

	return { yaml: stringify(doc), warnings };
}
