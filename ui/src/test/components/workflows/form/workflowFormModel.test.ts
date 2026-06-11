/**
 * workflowFormModel tests — validation rules and request building.
 */

import { describe, expect, it } from "vitest";
import { newTriggerDraft } from "@/components/workflows/form/triggerDraft";
import {
	DEFAULT_WORKFLOW_FORM,
	hasWorkflowErrors,
	validateWorkflowForm,
	type WorkflowFormState,
	workflowFormFromWorkflow,
	workflowToCreateRequest,
	workflowToUpdateRequest,
} from "@/components/workflows/form/workflowFormModel";
import { makeTrigger, makeWorkflow } from "@/test/mocks/factories";

function validState(): WorkflowFormState {
	return {
		...DEFAULT_WORKFLOW_FORM,
		name: "wf",
		agentId: "agent-1",
		trigger: {
			type: "github_issues",
			values: {
				owner: "geoffjay",
				repo: "agentd",
				labels: "bug, ui",
				state: "open",
				assignee: "",
			},
		},
	};
}

describe("validateWorkflowForm", () => {
	it("accepts a complete form", () => {
		expect(hasWorkflowErrors(validateWorkflowForm(validState()))).toBe(false);
	});

	it("requires name, agent, and prompt template", () => {
		const errors = validateWorkflowForm({
			...validState(),
			name: " ",
			agentId: "",
			promptTemplate: "",
		});
		expect(errors.name).toBeTruthy();
		expect(errors.agentId).toBeTruthy();
		expect(errors.promptTemplate).toBeTruthy();
	});

	it("only validates the poll interval for polling triggers", () => {
		const polling = validateWorkflowForm({ ...validState(), pollMinutes: "0" });
		expect(polling.pollMinutes).toBeTruthy();

		const manual = validateWorkflowForm({
			...validState(),
			trigger: newTriggerDraft("manual"),
			pollMinutes: "0",
		});
		expect(manual.pollMinutes).toBeUndefined();
	});

	it("surfaces trigger validation errors", () => {
		const errors = validateWorkflowForm({
			...validState(),
			trigger: newTriggerDraft("github_issues"),
		});
		expect(errors.trigger?.join(" ")).toMatch(/Owner is required/);
	});
});

describe("request building", () => {
	it("builds a create request with the typed trigger config", () => {
		const request = workflowToCreateRequest(validState());
		expect(request.trigger_config).toEqual({
			type: "github_issues",
			owner: "geoffjay",
			repo: "agentd",
			labels: ["bug", "ui"],
			state: "open",
		});
		expect(request.poll_interval_secs).toBe(15 * 60);
		expect(request.tool_policy).toEqual({ mode: "allow_all" });
	});

	it("builds an update request including trigger_config and agent_id", () => {
		const request = workflowToUpdateRequest(validState());
		expect(request.agent_id).toBe("agent-1");
		expect(request.trigger_config?.type).toBe("github_issues");
	});

	it("round-trips a workflow through the form state", () => {
		const workflow = makeWorkflow({
			trigger_config: makeTrigger("cron"),
			poll_interval_secs: 120,
			tool_policy: { mode: "deny_list", tools: ["Bash"] },
		});
		const state = workflowFormFromWorkflow(workflow);
		const request = workflowToUpdateRequest(state);

		expect(request.name).toBe(workflow.name);
		expect(request.agent_id).toBe(workflow.agent_id);
		expect(request.trigger_config).toEqual(workflow.trigger_config);
		expect(request.poll_interval_secs).toBe(120);
		expect(request.tool_policy).toEqual(workflow.tool_policy);
	});
});
