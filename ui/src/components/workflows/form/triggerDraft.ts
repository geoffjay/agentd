/**
 * TriggerDraft — editable form representation of a TriggerConfig.
 *
 * Flat triggers keep their values as strings (per the codebase convention
 * for numeric inputs); the composite trigger nests child drafts. Pure
 * functions convert between drafts and API configs and validate drafts,
 * mirroring the orchestrator's create/update validation.
 */

import type { TriggerConfig, TriggerType } from "@/types/orchestrator";
import {
	defaultValues,
	type TriggerFieldValues,
	triggerDef,
} from "./triggerDefs";

export const MAX_COMPOSITE_DEPTH = 3;

export interface CompositeDraft {
	mode: "or" | "and";
	/** Seconds; kept as a string for the number input. */
	correlationWindowSecs: string;
	triggers: TriggerDraft[];
}

export interface TriggerDraft {
	type: TriggerType;
	/** Flat field values for non-composite triggers. */
	values: TriggerFieldValues;
	/** Present only when type === "composite". */
	composite?: CompositeDraft;
}

/** A fresh draft for the given type, seeded with field defaults. */
export function newTriggerDraft(type: TriggerType): TriggerDraft {
	if (type === "composite") {
		return {
			type,
			values: {},
			composite: {
				mode: "or",
				correlationWindowSecs: "",
				triggers: [newTriggerDraft("manual"), newTriggerDraft("manual")],
			},
		};
	}
	return { type, values: defaultValues(type) };
}

/** Build a draft from an existing API config (for edit mode / YAML import). */
export function draftFromConfig(config: TriggerConfig): TriggerDraft {
	if (config.type === "composite") {
		return {
			type: "composite",
			values: {},
			composite: {
				mode: config.mode,
				correlationWindowSecs:
					config.correlation_window_secs != null
						? String(config.correlation_window_secs)
						: "",
				triggers: config.triggers.map(draftFromConfig),
			},
		};
	}
	return {
		type: config.type,
		values: triggerDef(config.type).fromConfig(config),
	};
}

/** Convert a draft to the typed TriggerConfig sent to the API. */
export function draftToConfig(draft: TriggerDraft): TriggerConfig {
	if (draft.type === "composite") {
		const composite = draft.composite;
		if (!composite) return { type: "composite", mode: "or", triggers: [] };
		const window = Number.parseInt(composite.correlationWindowSecs, 10);
		return {
			type: "composite",
			mode: composite.mode,
			triggers: composite.triggers.map(draftToConfig),
			correlation_window_secs: Number.isNaN(window) ? undefined : window,
		};
	}
	return triggerDef(draft.type).toConfig(draft.values);
}

/**
 * Validate a draft, mirroring the orchestrator's trigger validation.
 * Returns human-readable error messages (empty array = valid).
 */
export function validateTriggerDraft(draft: TriggerDraft, depth = 0): string[] {
	const errors: string[] = [];

	if (draft.type === "composite") {
		const composite = draft.composite;
		if (!composite || composite.triggers.length < 2) {
			errors.push("Composite trigger requires at least 2 sub-triggers.");
		}
		if (depth >= MAX_COMPOSITE_DEPTH) {
			errors.push(
				`Composite trigger nesting exceeds the maximum depth of ${MAX_COMPOSITE_DEPTH}.`,
			);
		}
		for (const sub of composite?.triggers ?? []) {
			errors.push(...validateTriggerDraft(sub, depth + 1));
		}
		return errors;
	}

	const def = triggerDef(draft.type);
	for (const field of def.fields) {
		const raw = draft.values[field.key];
		const empty = Array.isArray(raw)
			? raw.filter((s) => s.trim()).length === 0
			: !(raw ?? "").toString().trim();
		if (field.required && empty) {
			errors.push(`${def.label}: ${field.label} is required.`);
		}
	}

	// Type-specific rules the field defs can't express.
	switch (draft.type) {
		case "agent_idle": {
			const n = Number.parseInt(String(draft.values.idle_seconds ?? ""), 10);
			if (Number.isNaN(n) || n <= 0)
				errors.push("Agent Idle: idle seconds must be a positive integer.");
			break;
		}
		case "linear_issues": {
			const config = draftToConfig(draft);
			if (config.type === "linear_issues") {
				const hasFilter =
					Boolean(config.team_key) ||
					Boolean(config.project) ||
					(config.status?.length ?? 0) > 0 ||
					(config.labels?.length ?? 0) > 0 ||
					Boolean(config.assignee);
				if (!hasFilter)
					errors.push(
						"Linear Issues: at least one filter (team, project, status, labels, or assignee) is required.",
					);
			}
			break;
		}
		case "queue": {
			const name = String(draft.values.queue_name ?? "").trim();
			if (name && !/^[A-Za-z0-9-]{1,64}$/.test(name))
				errors.push(
					"Queue: name may only contain alphanumeric characters and hyphens (max 64 chars).",
				);
			break;
		}
		default:
			break;
	}

	return errors;
}
