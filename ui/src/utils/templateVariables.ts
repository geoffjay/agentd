/**
 * Utilities for working with workflow prompt template variables.
 *
 * Mirrors the backend's template semantics (crates/orchestrator/src/scheduler/template.rs):
 * top-level task fields ({{title}}, {{body}}, {{url}}, {{labels}}, {{assignee}},
 * {{source_id}}, {{metadata}}) are replaced first, then any remaining
 * {{variable}} placeholders are resolved from task metadata. Unknown variables
 * are preserved as-is in the rendered prompt.
 */

import type { Task } from "@/types/orchestrator";

/** How a template variable is supplied when manually triggering a workflow. */
export type VariableKind =
	/** Maps to a top-level field of TriggerWorkflowRequest (title, body, url, assignee). */
	| "field"
	/** Comma-separated list mapping to TriggerWorkflowRequest.labels. */
	| "labels"
	/** The special {{metadata}} variable rendering the whole metadata map. */
	| "metadata-map"
	/** Assigned by the server on trigger; cannot be set manually. */
	| "readonly"
	/** Resolved from the metadata map by name. */
	| "metadata";

export interface TemplateVariable {
	name: string;
	kind: VariableKind;
}

const FIELD_VARIABLES = new Set(["title", "body", "url", "assignee"]);

/** Classify a single variable name into how it must be supplied. */
export function classifyVariable(name: string): VariableKind {
	if (FIELD_VARIABLES.has(name)) return "field";
	if (name === "labels") return "labels";
	if (name === "metadata") return "metadata-map";
	if (name === "source_id") return "readonly";
	return "metadata";
}

/**
 * Extract {{variable}} placeholders from a prompt template, deduplicated and
 * in order of first appearance. Whitespace inside braces is tolerated
 * ({{ title }} === {{title}}), matching the backend's `.trim()`.
 */
export function extractTemplateVariables(template: string): TemplateVariable[] {
	const seen = new Set<string>();
	const variables: TemplateVariable[] = [];
	const pattern = /\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g;

	for (const match of template.matchAll(pattern)) {
		const name = match[1];
		if (seen.has(name)) continue;
		seen.add(name);
		variables.push({ name, kind: classifyVariable(name) });
	}

	return variables;
}

/**
 * Client-side mirror of the backend's render_template, used for live previews.
 * Unknown variables are preserved as literal {{placeholders}}, matching the
 * backend behavior.
 */
export function renderTemplatePreview(template: string, task: Task): string {
	const metadataBlock = Object.entries(task.metadata)
		.map(([k, v]) => `${k}: ${v}`)
		.join("\n");

	return template.replace(
		/\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g,
		(placeholder, name: string) => {
			switch (name) {
				case "title":
					return task.title;
				case "body":
					return task.body;
				case "url":
					return task.url;
				case "labels":
					return task.labels.join(", ");
				case "assignee":
					return task.assignee ?? "";
				case "source_id":
					return task.source_id;
				case "metadata":
					return metadataBlock;
				default:
					return task.metadata[name] ?? placeholder;
			}
		},
	);
}
