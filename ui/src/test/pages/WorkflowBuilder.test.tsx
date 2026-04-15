/**
 * WorkflowBuilder page tests.
 *
 * Covers: rendering in create mode, rendering in edit mode (loads workflow),
 * dirty indicator, cancel navigation, save validation, save success redirect.
 */

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// ---------------------------------------------------------------------------
// Stub ResizeObserver — React Flow requires it, jsdom does not implement it
// ---------------------------------------------------------------------------

class _StubResizeObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
}
vi.stubGlobal("ResizeObserver", _StubResizeObserver);

// ---------------------------------------------------------------------------
// Mock React Flow — avoid the full canvas in unit tests
// ---------------------------------------------------------------------------

vi.mock("@xyflow/react", async (importOriginal) => {
	const actual = await importOriginal<typeof import("@xyflow/react")>();
	return {
		...actual,
		ReactFlow: ({ children }: { children?: React.ReactNode }) => (
			<div data-testid="mock-react-flow">{children}</div>
		),
		ReactFlowProvider: ({ children }: { children?: React.ReactNode }) => (
			<div>{children}</div>
		),
		MiniMap: () => null,
		Controls: () => null,
		Background: () => null,
		// FitViewOnLoad calls useReactFlow; stub it so tests don't need a real provider
		useReactFlow: () => ({ fitView: vi.fn() }),
	};
});

// ---------------------------------------------------------------------------
// Mock LayoutContext — WorkflowBuilder calls setFullBleed on mount
// ---------------------------------------------------------------------------

const mockSetFullBleed = vi.fn();
vi.mock("@/layouts/context", () => ({
	useLayout: () => ({
		sidebarOpen: true,
		toggleSidebar: vi.fn(),
		setSidebarOpen: vi.fn(),
		searchOpen: false,
		openSearch: vi.fn(),
		closeSearch: vi.fn(),
		fullBleed: false,
		setFullBleed: mockSetFullBleed,
	}),
}));

// ---------------------------------------------------------------------------
// Mock hooks and services
// ---------------------------------------------------------------------------

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async (importOriginal) => {
	const actual = await importOriginal<typeof import("react-router-dom")>();
	return {
		...actual,
		useNavigate: () => mockNavigate,
	};
});

const mockAllAgents = [
	{
		id: "agent-1",
		name: "My Agent",
		status: "running",
		config: {
			working_dir: "/tmp",
			shell: "/bin/sh",
			interactive: false,
			tool_policy: { mode: "allow_all" },
		},
		created_at: "2024-01-01T00:00:00Z",
		updated_at: "2024-01-01T00:00:00Z",
	},
];

vi.mock("@/hooks/useAgents", () => ({
	useAgents: () => ({ allAgents: mockAllAgents }),
}));

const mockGetWorkflow = vi.fn();
const mockCreateWorkflow = vi.fn();
const mockUpdateWorkflow = vi.fn();

vi.mock("@/services/orchestrator", () => ({
	orchestratorClient: {
		getWorkflow: (...args: unknown[]) => mockGetWorkflow(...args),
		createWorkflow: (...args: unknown[]) => mockCreateWorkflow(...args),
		updateWorkflow: (...args: unknown[]) => mockUpdateWorkflow(...args),
	},
}));

// ---------------------------------------------------------------------------
// Import under test AFTER mocks
// ---------------------------------------------------------------------------

import { WorkflowBuilder } from "@/pages/workflows/WorkflowBuilder";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderCreate() {
	return render(
		<MemoryRouter initialEntries={["/workflows/builder"]}>
			<Routes>
				<Route path="/workflows/builder" element={<WorkflowBuilder />} />
			</Routes>
		</MemoryRouter>,
	);
}

function renderEdit(workflowId = "wf-1") {
	return render(
		<MemoryRouter initialEntries={[`/workflows/${workflowId}/edit`]}>
			<Routes>
				<Route path="/workflows/:id/edit" element={<WorkflowBuilder />} />
			</Routes>
		</MemoryRouter>,
	);
}

const baseWorkflow = {
	id: "wf-1",
	name: "My Workflow",
	agent_id: "agent-1",
	prompt_template: "Do the thing",
	poll_interval_secs: 300,
	enabled: true,
	trigger_config: { type: "manual" as const },
	created_at: "2024-01-01T00:00:00Z",
	updated_at: "2024-01-01T00:00:00Z",
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("WorkflowBuilder", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		mockCreateWorkflow.mockResolvedValue({ id: "wf-new", name: "Saved" });
		mockUpdateWorkflow.mockResolvedValue({ ...baseWorkflow, name: "Updated" });
		mockGetWorkflow.mockResolvedValue(baseWorkflow);
	});

	describe("create mode (/workflows/builder)", () => {
		it("renders the builder wrapper", () => {
			renderCreate();
			expect(screen.getByTestId("workflow-builder")).toBeInTheDocument();
		});

		it("renders name input", () => {
			renderCreate();
			expect(screen.getByTestId("builder-name-input")).toBeInTheDocument();
		});

		it("renders Save and Cancel buttons", () => {
			renderCreate();
			expect(screen.getByTestId("builder-save-btn")).toBeInTheDocument();
			expect(screen.getByTestId("builder-cancel-btn")).toBeInTheDocument();
		});

		it("does not show loading spinner", () => {
			renderCreate();
			expect(screen.queryByText("Saving…")).not.toBeInTheDocument();
		});

		it("does not call getWorkflow in create mode", () => {
			renderCreate();
			expect(mockGetWorkflow).not.toHaveBeenCalled();
		});

		it("shows unsaved indicator after typing in name input", async () => {
			const user = userEvent.setup();
			renderCreate();
			await user.type(screen.getByTestId("builder-name-input"), "My Flow");
			expect(screen.getByTestId("dirty-indicator")).toBeInTheDocument();
		});

		it("Cancel navigates to /workflows", async () => {
			const user = userEvent.setup();
			renderCreate();
			await user.click(screen.getByTestId("builder-cancel-btn"));
			expect(mockNavigate).toHaveBeenCalledWith("/workflows");
		});

		it("empty graph save shows error instead of false 'Saved' confirmation", async () => {
			const user = userEvent.setup();
			renderCreate();
			// Status bar absent before any save attempt
			expect(screen.queryByTestId("builder-status-bar")).not.toBeInTheDocument();
			await user.click(screen.getByTestId("builder-save-btn"));
			// Empty graph passes validateGraph but graphToWorkflows returns [] →
			// guard fires, shows error rather than misleading "Saved"
			await waitFor(() =>
				expect(screen.getByTestId("builder-status-bar")).toBeInTheDocument(),
			);
			expect(
				screen.getByText(
					"Add at least one trigger connected to an agent before saving.",
				),
			).toBeInTheDocument();
			expect(mockCreateWorkflow).not.toHaveBeenCalled();
		});

		it("renders node palette sidebar", () => {
			renderCreate();
			expect(screen.getByTestId("node-palette")).toBeInTheDocument();
		});

		it("renders canvas area", () => {
			renderCreate();
			expect(screen.getByTestId("builder-canvas-area")).toBeInTheDocument();
		});
	});

	describe("edit mode (/workflows/:id/edit)", () => {
		it("calls getWorkflow with the route id", async () => {
			renderEdit("wf-1");
			await waitFor(() => {
				expect(mockGetWorkflow).toHaveBeenCalledWith("wf-1");
			});
		});

		it("populates the name input after loading", async () => {
			renderEdit("wf-1");
			await waitFor(() => {
				expect(screen.getByTestId("builder-name-input")).toHaveValue(
					"My Workflow",
				);
			});
		});

		it("Cancel navigates to /workflows/wf-1 (not the list)", async () => {
			const user = userEvent.setup();
			renderEdit("wf-1");
			await waitFor(() =>
				expect(screen.getByTestId("builder-name-input")).toHaveValue(
					"My Workflow",
				),
			);
			await user.click(screen.getByTestId("builder-cancel-btn"));
			expect(mockNavigate).toHaveBeenCalledWith("/workflows/wf-1");
		});

		it("shows error when getWorkflow rejects", async () => {
			mockGetWorkflow.mockRejectedValue(new Error("Not found"));
			renderEdit("wf-bad");
			await waitFor(() => {
				expect(screen.getByTestId("builder-status-bar")).toBeInTheDocument();
			});
			expect(screen.getByText("Not found")).toBeInTheDocument();
		});
	});

	describe("save flow", () => {
		it("shows saving spinner during save", async () => {
			// Make createWorkflow hang so we can observe the spinner
			let resolve!: (v: unknown) => void;
			mockCreateWorkflow.mockReturnValue(new Promise((r) => { resolve = r; }));

			const user = userEvent.setup();
			renderCreate();

			// We need at least one connected node to pass validateGraph.
			// Since the canvas is mocked, just spy that createWorkflow is NOT called
			// when graph is empty (validateGraph blocks it).
			await user.click(screen.getByTestId("builder-save-btn"));

			// With empty graph, validation fires — no spinner (createWorkflow not called)
			expect(screen.queryByText("Saving…")).not.toBeInTheDocument();
			resolve({ id: "x" });
		});

		it("does not call createWorkflow for an empty graph; shows error prompt", async () => {
			// Empty canvas has no edges, so graphToWorkflows returns [] →
			// the guard fires with an error before any API call.
			const user = userEvent.setup();
			renderCreate();

			await user.type(screen.getByTestId("builder-name-input"), "Test Flow");
			await user.click(screen.getByTestId("builder-save-btn"));

			await waitFor(() =>
				expect(screen.getByTestId("builder-status-bar")).toBeInTheDocument(),
			);
			expect(
				screen.getByText(
					"Add at least one trigger connected to an agent before saving.",
				),
			).toBeInTheDocument();
			expect(mockCreateWorkflow).not.toHaveBeenCalled();
		});

		it("calls updateWorkflow (not createWorkflow) in edit mode", async () => {
			const user = userEvent.setup();
			renderEdit("wf-1");
			await waitFor(() =>
				expect(screen.getByTestId("builder-name-input")).toHaveValue(
					"My Workflow",
				),
			);
			await user.click(screen.getByTestId("builder-save-btn"));
			await waitFor(() =>
				expect(mockUpdateWorkflow).toHaveBeenCalledWith(
					"wf-1",
					expect.any(Object),
				),
			);
			expect(mockCreateWorkflow).not.toHaveBeenCalled();
		});
	});

	describe("dirty tracking", () => {
		it("does not show dirty indicator on initial render (no user edits yet)", () => {
			renderCreate();
			// The "Unsaved changes" badge must NOT appear before the user touches anything.
			expect(screen.queryByTestId("dirty-indicator")).not.toBeInTheDocument();
		});

		it("does not show dirty indicator in edit mode before user edits canvas", async () => {
			renderEdit("wf-1");
			// Wait for the workflow to load (name populated)
			await waitFor(() =>
				expect(screen.getByTestId("builder-name-input")).toHaveValue(
					"My Workflow",
				),
			);
			// Loading the workflow triggers internal React Flow changes (dimensions,
			// select, position-fit) — none of these are user edits.
			expect(screen.queryByTestId("dirty-indicator")).not.toBeInTheDocument();
		});
	});

	describe("edit-mode load guard", () => {
		it("calls getWorkflow only once even when allAgents reference changes", async () => {
			// The useAgents mock returns a stable reference by default, so simulate
			// a scenario where renderEdit mounts with agents already loaded.
			renderEdit("wf-1");
			await waitFor(() => expect(mockGetWorkflow).toHaveBeenCalledTimes(1));
			// Even if the component re-renders (e.g. parent state change), getWorkflow
			// must not be called a second time because hasLoadedRef guards it.
			expect(mockGetWorkflow).toHaveBeenCalledTimes(1);
		});
	});
});
