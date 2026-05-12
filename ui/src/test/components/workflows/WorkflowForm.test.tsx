/**
 * WorkflowForm tests.
 *
 * Covers the null-guard fix for trigger_config: editing a workflow where
 * trigger_config is undefined/null must not throw, and editing one with a
 * valid github_issues trigger_config must populate the form fields correctly.
 */

import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WorkflowForm } from "@/components/workflows/WorkflowForm";
import type { Agent, Workflow } from "@/types/orchestrator";

// Mock PromptTemplateEditor to keep tests simple (it has its own test suite)
vi.mock("@/components/workflows/PromptTemplateEditor", () => ({
	PromptTemplateEditor: ({
		value,
		onChange,
	}: {
		value: string;
		onChange: (v: string) => void;
	}) => (
		<textarea
			aria-label="Prompt template"
			value={value}
			onChange={(e) => onChange(e.target.value)}
		/>
	),
}));

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const mockAgents: Agent[] = [
	{
		id: "agent-1",
		name: "Test Agent",
		status: "running",
		config: {
			working_dir: "/tmp",
			shell: "/bin/sh",
			interactive: false,
			tool_policy: { mode: "allow_all" },
		},
		created_at: new Date().toISOString(),
		updated_at: new Date().toISOString(),
	},
];

const validWorkflow: Workflow = {
	id: "wf-1",
	name: "My Workflow",
	agent_id: "agent-1",
	trigger_config: {
		type: "github_issues",
		owner: "geoffjay",
		repo: "agentd",
		labels: ["bug", "enhancement"],
		state: "open",
	},
	prompt_template: "Fix this: {{title}}",
	poll_interval_secs: 900,
	enabled: true,
	tool_policy: { mode: "allow_all" },
	created_at: new Date().toISOString(),
	updated_at: new Date().toISOString(),
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderForm(workflow?: Workflow) {
	const onSave = vi.fn().mockResolvedValue(undefined);
	const onClose = vi.fn();

	render(
		<WorkflowForm
			open={true}
			workflow={workflow}
			agents={mockAgents}
			onSave={onSave}
			onClose={onClose}
		/>,
	);

	return { onSave, onClose };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("WorkflowForm", () => {
	describe("create mode (no workflow)", () => {
		it("renders with empty fields", () => {
			renderForm();
			expect(
				screen.getByRole("dialog", { name: /create workflow/i }),
			).toBeInTheDocument();
			expect(screen.getByPlaceholderText(/dispatch github issues/i)).toHaveValue(
				"",
			);
		});
	});

	describe("edit mode — valid trigger_config", () => {
		it("populates the name field", async () => {
			renderForm(validWorkflow);
			await waitFor(() => {
				expect(screen.getByPlaceholderText(/dispatch github issues/i)).toHaveValue(
					"My Workflow",
				);
			});
		});

		it("populates the owner field", async () => {
			renderForm(validWorkflow);
			await waitFor(() => {
				expect(screen.getByPlaceholderText(/e\.g\. geoffjay/i)).toHaveValue(
					"geoffjay",
				);
			});
		});

		it("populates the repo field", async () => {
			renderForm(validWorkflow);
			await waitFor(() => {
				expect(screen.getByPlaceholderText(/e\.g\. agentd/i)).toHaveValue(
					"agentd",
				);
			});
		});

		it("populates labels as comma-separated string", async () => {
			renderForm(validWorkflow);
			await waitFor(() => {
				expect(
					screen.getByPlaceholderText(/bug, enhancement/i),
				).toHaveValue("bug, enhancement");
			});
		});
	});

	describe("edit mode — missing trigger_config (null guard)", () => {
		it("does not throw when trigger_config is undefined", () => {
			// Cast to bypass TypeScript: simulates an API response with missing field
			const brokenWorkflow = {
				...validWorkflow,
				trigger_config: undefined,
			} as unknown as Workflow;

			expect(() => renderForm(brokenWorkflow)).not.toThrow();
		});

		it("renders the dialog without crashing when trigger_config is undefined", () => {
			const brokenWorkflow = {
				...validWorkflow,
				trigger_config: undefined,
			} as unknown as Workflow;

			renderForm(brokenWorkflow);

			expect(
				screen.getByRole("dialog", { name: /edit workflow/i }),
			).toBeInTheDocument();
		});

		it("leaves GitHub fields at defaults when trigger_config is undefined", async () => {
			const brokenWorkflow = {
				...validWorkflow,
				trigger_config: undefined,
			} as unknown as Workflow;

			renderForm(brokenWorkflow);

			await waitFor(() => {
				// Owner and repo should be empty (default), not crashed
				expect(screen.getByPlaceholderText(/e\.g\. geoffjay/i)).toHaveValue("");
				expect(screen.getByPlaceholderText(/e\.g\. agentd/i)).toHaveValue("");
			});
		});

		it("does not throw when trigger_config is null", () => {
			const brokenWorkflow = {
				...validWorkflow,
				trigger_config: null,
			} as unknown as Workflow;

			expect(() => renderForm(brokenWorkflow)).not.toThrow();
		});
	});
});
