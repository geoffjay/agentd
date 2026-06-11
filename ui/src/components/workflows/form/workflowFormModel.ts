/**
 * workflowFormModel — pure form-state model for the workflow form page.
 *
 * The draft type, defaults, conversions to/from API requests, and
 * validation live here so the page component, tests, and the YAML
 * import/export share one substrate.
 */

import {
	DEFAULT_POLICY_DRAFT,
	policyDraftFromPolicy,
	policyFromDraft,
	type ToolPolicyDraft,
} from "@/components/common/form";
import type {
	CreateWorkflowRequest,
	UpdateWorkflowRequest,
	Workflow,
} from "@/types/orchestrator";
import {
	draftFromConfig,
	draftToConfig,
	newTriggerDraft,
	type TriggerDraft,
	validateTriggerDraft,
} from "./triggerDraft";
import { triggerDef } from "./triggerDefs";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export interface WorkflowFormState {
	name: string;
	agentId: string;
	trigger: TriggerDraft;
	promptTemplate: string;
	/** Poll interval in minutes (string for the number input). */
	pollMinutes: string;
	enabled: boolean;
	toolPolicy: ToolPolicyDraft;
}

export interface WorkflowFormErrors {
	name?: string;
	agentId?: string;
	promptTemplate?: string;
	pollMinutes?: string;
	trigger?: string[];
}

export const DEFAULT_WORKFLOW_TEMPLATE = `You are working on the following task:

Title: {{title}}

Description:
{{body}}

Source: {{url}}
Labels: {{labels}}

Please work on this task and report back when complete.`;

export const DEFAULT_WORKFLOW_FORM: WorkflowFormState = {
	name: "",
	agentId: "",
	trigger: newTriggerDraft("github_issues"),
	promptTemplate: DEFAULT_WORKFLOW_TEMPLATE,
	pollMinutes: "15",
	enabled: true,
	toolPolicy: DEFAULT_POLICY_DRAFT,
};

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

export function workflowFormFromWorkflow(
	workflow: Workflow,
): WorkflowFormState {
	return {
		name: workflow.name,
		agentId: workflow.agent_id,
		trigger: workflow.trigger_config
			? draftFromConfig(workflow.trigger_config)
			: newTriggerDraft("manual"),
		promptTemplate: workflow.prompt_template,
		pollMinutes: String(
			Math.max(1, Math.round(workflow.poll_interval_secs / 60)),
		),
		enabled: workflow.enabled,
		toolPolicy: policyDraftFromPolicy(workflow.tool_policy),
	};
}

function pollIntervalSecs(state: WorkflowFormState): number {
	const mins = Number.parseInt(state.pollMinutes, 10);
	return Number.isNaN(mins) ? 60 : Math.max(1, mins) * 60;
}

export function workflowToCreateRequest(
	state: WorkflowFormState,
): CreateWorkflowRequest {
	return {
		name: state.name.trim(),
		agent_id: state.agentId,
		trigger_config: draftToConfig(state.trigger),
		prompt_template: state.promptTemplate.trim(),
		poll_interval_secs: pollIntervalSecs(state),
		enabled: state.enabled,
		tool_policy: policyFromDraft(state.toolPolicy),
	};
}

export function workflowToUpdateRequest(
	state: WorkflowFormState,
): UpdateWorkflowRequest {
	return {
		name: state.name.trim(),
		agent_id: state.agentId,
		trigger_config: draftToConfig(state.trigger),
		prompt_template: state.promptTemplate.trim(),
		poll_interval_secs: pollIntervalSecs(state),
		enabled: state.enabled,
		tool_policy: policyFromDraft(state.toolPolicy),
	};
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

export function validateWorkflowForm(
	state: WorkflowFormState,
): WorkflowFormErrors {
	const errors: WorkflowFormErrors = {};

	if (!state.name.trim()) errors.name = "Name is required";
	if (!state.agentId) errors.agentId = "Select an agent";
	if (!state.promptTemplate.trim())
		errors.promptTemplate = "Prompt template is required";

	if (triggerDef(state.trigger.type).polls) {
		const mins = Number.parseInt(state.pollMinutes, 10);
		if (Number.isNaN(mins) || mins < 1)
			errors.pollMinutes = "Minimum poll interval is 1 minute";
	}

	const triggerErrors = validateTriggerDraft(state.trigger);
	if (triggerErrors.length > 0) errors.trigger = triggerErrors;

	return errors;
}

export function hasWorkflowErrors(errors: WorkflowFormErrors): boolean {
	return Boolean(
		errors.name ||
			errors.agentId ||
			errors.promptTemplate ||
			errors.pollMinutes ||
			(errors.trigger && errors.trigger.length > 0),
	);
}
