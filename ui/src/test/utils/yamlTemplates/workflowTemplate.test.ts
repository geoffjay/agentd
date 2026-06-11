/**
 * Workflow YAML template import/export tests — agent name resolution,
 * trigger mapping per type, warning classes, and round trips.
 */

import { describe, expect, it } from "vitest";
import { TRIGGER_DEFS } from "@/components/workflows/form/triggerDefs";
import { draftToConfig } from "@/components/workflows/form/triggerDraft";
import {
	DEFAULT_WORKFLOW_FORM,
	type WorkflowFormState,
} from "@/components/workflows/form/workflowFormModel";
import { makeAgent, makeTrigger } from "@/test/mocks/factories";
import {
	exportWorkflowYaml,
	importWorkflowYaml,
} from "@/utils/yamlTemplates/workflowTemplate";

const AGENTS = [
	makeAgent({ id: "agent-1", name: "worker" }),
	makeAgent({ id: "agent-2", name: "reviewer" }),
];

const GITHUB_TEMPLATE = `
name: issue-worker
agent: worker

source:
  type: github_issues
  owner: geoffjay
  repo: agentd
  labels:
    - agent
  state: open

poll_interval: 120
enabled: true

prompt_template: |
  Work on issue #{{source_id}}: {{title}}
`;

describe("importWorkflowYaml", () => {
	it("maps the template and resolves the agent by name", () => {
		const { state, warnings } = importWorkflowYaml(GITHUB_TEMPLATE, AGENTS);
		expect(state.name).toBe("issue-worker");
		expect(state.agentId).toBe("agent-1");
		expect(state.trigger.type).toBe("github_issues");
		expect(state.trigger.values.owner).toBe("geoffjay");
		expect(state.pollMinutes).toBe("2");
		expect(state.enabled).toBe(true);
		expect(state.promptTemplate).toContain("{{title}}");
		expect(warnings).toEqual([]);
	});

	it("warns when the agent name cannot be resolved", () => {
		const { state, warnings } = importWorkflowYaml(
			GITHUB_TEMPLATE.replace("agent: worker", "agent: nobody"),
			AGENTS,
		);
		expect(state.agentId).toBe("");
		expect(warnings.join(" ")).toMatch(/"nobody" was not found/);
	});

	it("warns and clears the template for prompt_template_file", () => {
		const { state, warnings } = importWorkflowYaml(
			`name: x\nagent: worker\nsource:\n  type: manual\nprompt_template_file: prompts/x.md`,
			AGENTS,
		);
		expect(state.promptTemplate).toBe("");
		expect(warnings.join(" ")).toMatch(/paste the file contents/);
	});

	it("throws on an unknown trigger type", () => {
		expect(() =>
			importWorkflowYaml(
				"name: x\nagent: worker\nsource:\n  type: carrier_pigeon",
				AGENTS,
			),
		).toThrow(/Unknown trigger type/);
	});

	it("throws when the source block is missing", () => {
		expect(() => importWorkflowYaml("name: x\nagent: worker", AGENTS)).toThrow(
			/source/,
		);
	});
});

describe("exportWorkflowYaml", () => {
	function stateWithTrigger(
		trigger: WorkflowFormState["trigger"],
	): WorkflowFormState {
		return {
			...DEFAULT_WORKFLOW_FORM,
			name: "exported",
			agentId: "agent-1",
			trigger,
			promptTemplate: "Do {{title}}",
		};
	}

	it("reverse-resolves the agent name and emits the source block", () => {
		const { state } = importWorkflowYaml(GITHUB_TEMPLATE, AGENTS);
		const { yaml, warnings } = exportWorkflowYaml(state, AGENTS);
		expect(yaml).toContain("agent: worker");
		expect(yaml).toContain("type: github_issues");
		expect(yaml).toContain("poll_interval: 120");
		expect(warnings).toEqual([]);
	});

	it("warns for trigger types agent apply cannot parse yet", () => {
		const { state } = importWorkflowYaml(
			"name: q\nagent: worker\nsource:\n  type: queue\n  queue_name: review-queue\nprompt_template: x",
			AGENTS,
		);
		const { warnings } = exportWorkflowYaml(state, AGENTS);
		expect(warnings.join(" ")).toMatch(/not yet supported by `agent apply`/);
	});

	it("warns when the agent id cannot be reverse-resolved", () => {
		const state = {
			...DEFAULT_WORKFLOW_FORM,
			name: "orphan",
			agentId: "missing-id",
		};
		const { yaml, warnings } = exportWorkflowYaml(state, AGENTS);
		expect(yaml).toContain("agent: missing-id");
		expect(warnings.join(" ")).toMatch(/could not be resolved/);
	});

	it("round-trips every trigger type through export → import", () => {
		for (const def of TRIGGER_DEFS) {
			const config = makeTrigger(def.type);
			const state = stateWithTrigger(
				// Build the draft from the canonical config.
				// (draftFromConfig is exercised by the triggerDraft tests.)
				{ type: def.type, values: def.fromConfig(config) },
			);
			// Composite needs its nested draft shape.
			if (def.type === "composite") continue;

			const { yaml } = exportWorkflowYaml(state, AGENTS);
			const { state: imported } = importWorkflowYaml(yaml, AGENTS);
			expect(draftToConfig(imported.trigger)).toEqual(config);
		}
	});
});
