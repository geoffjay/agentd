/**
 * Workflow YAML template import/export.
 *
 * Mirrors the `WorkflowTemplate` shape consumed by `agent apply`
 * (crates/cli/src/commands/apply.rs): `agent: <name>` instead of an
 * agent_id, a `source:` trigger block, and `poll_interval` in seconds.
 *
 * The CLI's SourceTemplate currently parses a subset of the trigger
 * types the API supports — exporting one of the unsupported types still
 * produces valid API-shaped YAML but carries a warning.
 */

import { parse, stringify } from "yaml";
import {
	DEFAULT_POLICY_DRAFT,
	policyDraftFromPolicy,
	policyFromDraft,
} from "@/components/common/form";
import { TRIGGER_DEFS } from "@/components/workflows/form/triggerDefs";
import {
	draftFromConfig,
	draftToConfig,
} from "@/components/workflows/form/triggerDraft";
import {
	DEFAULT_WORKFLOW_FORM,
	type WorkflowFormState,
} from "@/components/workflows/form/workflowFormModel";
import type {
	Agent,
	ToolPolicy,
	TriggerConfig,
	TriggerType,
} from "@/types/orchestrator";
import type { YamlExportResult, YamlImportResult } from "./agentTemplate";

const WORKFLOW_TEMPLATE_KEYS = new Set([
	"name",
	"agent",
	"source",
	"poll_interval",
	"enabled",
	"tool_policy",
	"prompt_template",
	"prompt_template_file",
]);

/** Trigger types the CLI's SourceTemplate cannot parse yet. */
const CLI_UNSUPPORTED_TRIGGERS: ReadonlySet<TriggerType> = new Set([
	"agent_idle",
	"composite",
	"queue",
	"ask_response",
]);

const KNOWN_TRIGGER_TYPES = new Set<string>(TRIGGER_DEFS.map((d) => d.type));

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

/**
 * Parse an `.agentd/workflows/*.yml` template into form state, resolving
 * `agent: <name>` against the provided agent list.
 * Throws on invalid YAML or an unknown trigger type.
 */
export function importWorkflowYaml(
	text: string,
	agents: Agent[],
): YamlImportResult<WorkflowFormState> {
	const raw = parse(text);
	if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
		throw new Error("Template must be a YAML mapping.");
	}
	const doc = raw as Record<string, unknown>;
	const warnings: string[] = [];

	const unknownKeys = Object.keys(doc).filter(
		(k) => !WORKFLOW_TEMPLATE_KEYS.has(k),
	);
	if (unknownKeys.length > 0) {
		warnings.push(`Ignored unknown field(s): ${unknownKeys.join(", ")}.`);
	}

	// Trigger: template key is `source`, API name is trigger_config.
	const source = doc.source;
	if (!source || typeof source !== "object" || !("type" in source)) {
		throw new Error("Template is missing a `source:` block with a `type`.");
	}
	const sourceType = String((source as Record<string, unknown>).type);
	if (!KNOWN_TRIGGER_TYPES.has(sourceType)) {
		throw new Error(`Unknown trigger type: ${sourceType}`);
	}
	const trigger = draftFromConfig(source as TriggerConfig);

	// Agent: resolved by name (case-sensitive, matching apply.rs).
	const agentName = typeof doc.agent === "string" ? doc.agent : "";
	const agent = agents.find((a) => a.name === agentName);
	if (agentName && !agent) {
		warnings.push(
			`Agent "${agentName}" was not found — select an agent manually.`,
		);
	}

	// Prompt template: the file indirection is resolved by `agent apply`
	// relative to the template; the API needs the final string.
	let promptTemplate =
		typeof doc.prompt_template === "string" ? doc.prompt_template : "";
	if (
		typeof doc.prompt_template_file === "string" &&
		doc.prompt_template_file
	) {
		warnings.push(
			`prompt_template_file (${doc.prompt_template_file}) cannot be resolved by the UI — paste the file contents into the prompt template.`,
		);
		promptTemplate = "";
	}

	const pollSecs = Number.parseInt(String(doc.poll_interval ?? ""), 10);

	const state: WorkflowFormState = {
		...DEFAULT_WORKFLOW_FORM,
		name: typeof doc.name === "string" ? doc.name : "",
		agentId: agent?.id ?? "",
		trigger,
		promptTemplate,
		pollMinutes: String(
			Math.max(1, Math.round((Number.isNaN(pollSecs) ? 60 : pollSecs) / 60)),
		),
		enabled: doc.enabled !== false,
		toolPolicy:
			doc.tool_policy && typeof doc.tool_policy === "object"
				? policyDraftFromPolicy(doc.tool_policy as ToolPolicy)
				: DEFAULT_POLICY_DRAFT,
	};

	return { state, warnings };
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/**
 * Serialize form state as an `.agentd/workflows/*.yml` template. The
 * selected agent's name is reverse-resolved from the agent list; an
 * unresolvable id is emitted as-is with a warning.
 */
export function exportWorkflowYaml(
	state: WorkflowFormState,
	agents: Agent[],
): YamlExportResult {
	const warnings: string[] = [];

	const agent = agents.find((a) => a.id === state.agentId);
	if (state.agentId && !agent) {
		warnings.push(
			"The selected agent could not be resolved to a name; the raw id was written to `agent:`.",
		);
	}

	const trigger = draftToConfig(state.trigger);
	if (CLI_UNSUPPORTED_TRIGGERS.has(trigger.type)) {
		warnings.push(
			`The \`${trigger.type}\` trigger is not yet supported by \`agent apply\` — this template only works via the API until the CLI's SourceTemplate is extended.`,
		);
	}
	if (
		trigger.type === "webhook" &&
		trigger.source &&
		trigger.source !== "any"
	) {
		warnings.push(
			"The webhook `source` filter is not yet parsed by `agent apply`; it only takes effect when created via the API.",
		);
	}

	const mins = Number.parseInt(state.pollMinutes, 10);
	const pollSecs = (Number.isNaN(mins) ? 1 : Math.max(1, mins)) * 60;

	const doc: Record<string, unknown> = {
		name: state.name.trim(),
		agent: agent?.name ?? state.agentId,
		source: trigger,
		...(pollSecs !== 60 ? { poll_interval: pollSecs } : {}),
		...(state.enabled ? {} : { enabled: false }),
		tool_policy: policyFromDraft(state.toolPolicy),
		prompt_template: state.promptTemplate,
	};

	return { yaml: stringify(doc), warnings };
}
