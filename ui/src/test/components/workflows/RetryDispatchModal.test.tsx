/**
 * RetryDispatchModal tests.
 *
 * Covers: prefill from the dispatch's persisted task, the missing-task
 * banner for legacy records, error mapping (409 busy), and the submit
 * payload shape sent to triggerWorkflow.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RetryDispatchModal } from "@/components/workflows/RetryDispatchModal";
import { orchestratorClient } from "@/services/orchestrator";
import { ApiError } from "@/types/common";
import type { DispatchRecord, Workflow } from "@/types/orchestrator";

vi.mock("@/services/orchestrator", () => ({
	orchestratorClient: {
		triggerWorkflow: vi.fn(),
	},
}));

const triggerWorkflow = vi.mocked(orchestratorClient.triggerWorkflow);

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const workflow: Workflow = {
	id: "wf-1",
	name: "Fix Issues",
	agent_id: "agent-1",
	source_config: {
		type: "github_issues",
		owner: "geoffjay",
		repo: "agentd",
		labels: [],
		state: "open",
	},
	prompt_template: "Fix {{title}} at {{url}}: {{body}} ({{priority}})",
	poll_interval_secs: 900,
	enabled: true,
	tool_policy: { mode: "allow_all" },
	created_at: new Date().toISOString(),
	updated_at: new Date().toISOString(),
};

const dispatchWithTask: DispatchRecord = {
	id: "dispatch-1",
	workflow_id: "wf-1",
	source_id: "issue-42",
	agent_id: "agent-1",
	prompt_sent: "Fix Login bug at https://example.com/42: Broken (high)",
	status: "failed",
	dispatched_at: new Date().toISOString(),
	task: {
		source_id: "issue-42",
		title: "Login bug",
		body: "Broken",
		url: "https://example.com/42",
		labels: ["bug"],
		assignee: "geoff",
		metadata: { priority: "high", retry_of: "older-dispatch" },
	},
};

const dispatchWithoutTask: DispatchRecord = {
	...dispatchWithTask,
	id: "dispatch-2",
	task: undefined,
};

function renderModal(dispatch: DispatchRecord) {
	const onClose = vi.fn();
	const onRetried = vi.fn();
	render(
		<RetryDispatchModal
			open={true}
			workflow={workflow}
			dispatch={dispatch}
			onClose={onClose}
			onRetried={onRetried}
		/>,
	);
	return { onClose, onRetried };
}

beforeEach(() => {
	triggerWorkflow.mockReset();
});

// ---------------------------------------------------------------------------
// Prefill
// ---------------------------------------------------------------------------

describe("RetryDispatchModal prefill", () => {
	it("prefills inputs from the dispatch's persisted task", () => {
		renderModal(dispatchWithTask);

		expect(screen.getByDisplayValue("Login bug")).toBeInTheDocument();
		expect(screen.getByDisplayValue("Broken")).toBeInTheDocument();
		expect(
			screen.getByDisplayValue("https://example.com/42"),
		).toBeInTheDocument();
		// Named metadata variable from the template
		expect(screen.getByDisplayValue("high")).toBeInTheDocument();
	});

	it("strips internal retry_of metadata from prefill", () => {
		renderModal(dispatchWithTask);
		expect(
			screen.queryByDisplayValue("older-dispatch"),
		).not.toBeInTheDocument();
	});

	it("only renders inputs for variables in the template", () => {
		renderModal(dispatchWithTask);
		// {{labels}} and {{assignee}} are not in the template
		expect(screen.queryByText(/\{\{labels\}\}/)).not.toBeInTheDocument();
		expect(screen.queryByText(/\{\{assignee\}\}/)).not.toBeInTheDocument();
	});

	it("shows a banner when the original task was not recorded", () => {
		renderModal(dispatchWithoutTask);
		expect(
			screen.getByText(/Original input values were not recorded/),
		).toBeInTheDocument();
	});
});

// ---------------------------------------------------------------------------
// Submit
// ---------------------------------------------------------------------------

describe("RetryDispatchModal submit", () => {
	it("sends prefilled values with retry_of metadata and reports success", async () => {
		triggerWorkflow.mockResolvedValue({
			...dispatchWithTask,
			id: "dispatch-new",
			source_id: "manual:abc",
			status: "dispatched",
		});
		const { onClose, onRetried } = renderModal(dispatchWithTask);

		fireEvent.click(screen.getByRole("button", { name: "Re-trigger" }));

		await waitFor(() => expect(triggerWorkflow).toHaveBeenCalledOnce());
		// All original task fields are forwarded (even those not in the
		// template) so the new dispatch's persisted task stays complete.
		expect(triggerWorkflow).toHaveBeenCalledWith("wf-1", {
			title: "Login bug",
			body: "Broken",
			url: "https://example.com/42",
			labels: ["bug"],
			assignee: "geoff",
			metadata: {
				priority: "high",
				retry_of: "dispatch-1",
			},
		});
		await waitFor(() => expect(onRetried).toHaveBeenCalledOnce());
		expect(onClose).toHaveBeenCalledOnce();
	});

	it("shows a busy message on 409 and keeps the modal open", async () => {
		triggerWorkflow.mockRejectedValue(new ApiError(409, "Conflict"));
		const { onClose, onRetried } = renderModal(dispatchWithTask);

		fireEvent.click(screen.getByRole("button", { name: "Re-trigger" }));

		await waitFor(() =>
			expect(
				screen.getByText(/Agent is busy with another task/),
			).toBeInTheDocument(),
		);
		expect(onRetried).not.toHaveBeenCalled();
		expect(onClose).not.toHaveBeenCalled();
	});
});
