/**
 * Tests for template variable extraction and preview rendering.
 */

import { describe, expect, it } from "vitest";
import type { Task } from "@/types/orchestrator";
import {
	classifyVariable,
	extractTemplateVariables,
	renderTemplatePreview,
} from "@/utils/templateVariables";

// ---------------------------------------------------------------------------
// extractTemplateVariables
// ---------------------------------------------------------------------------

describe("extractTemplateVariables", () => {
	it("extracts variables in order of first appearance", () => {
		const vars = extractTemplateVariables(
			"Fix {{title}} at {{url}}: {{body}}",
		);
		expect(vars.map((v) => v.name)).toEqual(["title", "url", "body"]);
	});

	it("deduplicates repeated variables", () => {
		const vars = extractTemplateVariables("{{title}} and {{title}} again");
		expect(vars).toHaveLength(1);
		expect(vars[0].name).toBe("title");
	});

	it("tolerates whitespace inside braces", () => {
		const vars = extractTemplateVariables("Handle {{ title }}");
		expect(vars.map((v) => v.name)).toEqual(["title"]);
	});

	it("ignores unclosed placeholders", () => {
		const vars = extractTemplateVariables("Broken {{title");
		expect(vars).toHaveLength(0);
	});

	it("returns empty for a template without variables", () => {
		expect(extractTemplateVariables("static prompt")).toEqual([]);
	});

	it("classifies variables by how they are supplied", () => {
		const vars = extractTemplateVariables(
			"{{title}} {{body}} {{url}} {{assignee}} {{labels}} {{metadata}} {{source_id}} {{fire_time}}",
		);
		const kinds = Object.fromEntries(vars.map((v) => [v.name, v.kind]));
		expect(kinds).toEqual({
			title: "field",
			body: "field",
			url: "field",
			assignee: "field",
			labels: "labels",
			metadata: "metadata-map",
			source_id: "readonly",
			fire_time: "metadata",
		});
	});
});

// ---------------------------------------------------------------------------
// classifyVariable
// ---------------------------------------------------------------------------

describe("classifyVariable", () => {
	it("treats unknown names as metadata-backed", () => {
		expect(classifyVariable("custom_thing")).toBe("metadata");
	});
});

// ---------------------------------------------------------------------------
// renderTemplatePreview
// ---------------------------------------------------------------------------

describe("renderTemplatePreview", () => {
	const task: Task = {
		source_id: "manual:abc",
		title: "Login bug",
		body: "Fix it",
		url: "https://example.com/42",
		labels: ["bug", "p1"],
		assignee: "geoff",
		metadata: { fire_time: "2026-06-10T00:00:00Z" },
	};

	it("replaces top-level fields and metadata variables", () => {
		const result = renderTemplatePreview(
			"{{title}} ({{labels}}) by {{assignee}} at {{fire_time}}",
			task,
		);
		expect(result).toBe(
			"Login bug (bug, p1) by geoff at 2026-06-10T00:00:00Z",
		);
	});

	it("renders the whole metadata map for {{metadata}}", () => {
		const result = renderTemplatePreview("Meta:\n{{metadata}}", task);
		expect(result).toBe("Meta:\nfire_time: 2026-06-10T00:00:00Z");
	});

	it("preserves unknown variables as literal placeholders", () => {
		const result = renderTemplatePreview("Value: {{unknown_var}}", task);
		expect(result).toBe("Value: {{unknown_var}}");
	});

	it("renders missing assignee as empty", () => {
		const result = renderTemplatePreview("By: {{assignee}}", {
			...task,
			assignee: undefined,
		});
		expect(result).toBe("By: ");
	});
});
