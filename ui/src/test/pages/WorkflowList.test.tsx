/**
 * WorkflowList — smoke tests for the workflows page.
 */

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/hooks/useWorkflows", () => ({
	useWorkflows: () => ({
		workflows: [
			{
				id: "wf-1",
				name: "nightly-review",
				agent_id: "agent-1",
				prompt_template: "Review PRs",
				poll_interval_secs: 300,
				enabled: true,
				created_at: new Date().toISOString(),
			},
		],
		total: 1,
		loading: false,
		refreshing: false,
		error: undefined,
		refetch: vi.fn(),
		createWorkflow: vi.fn(),
		updateWorkflow: vi.fn(),
		deleteWorkflow: vi.fn(),
		toggleEnabled: vi.fn(),
	}),
}));

vi.mock("@/hooks/useAgents", () => ({
	useAgents: () => ({
		allAgents: [{ id: "agent-1", name: "review-bot" }],
	}),
}));

import { WorkflowList } from "@/pages/workflows/WorkflowList";

function renderPage() {
	return render(
		<MemoryRouter>
			<WorkflowList />
		</MemoryRouter>,
	);
}

describe("WorkflowList", () => {
	it("renders the page heading", () => {
		renderPage();
		expect(screen.getByText("Workflows")).toBeInTheDocument();
	});

	it("renders the New workflow button", () => {
		renderPage();
		expect(screen.getByText("New workflow")).toBeInTheDocument();
	});

	it("renders the search input", () => {
		renderPage();
		expect(
			screen.getByPlaceholderText(/search workflows/i),
		).toBeInTheDocument();
	});

	it("renders the refresh button", () => {
		renderPage();
		expect(
			screen.getByRole("button", { name: /refresh workflows/i }),
		).toBeInTheDocument();
	});
});
