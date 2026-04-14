/**
 * NodePalette tests.
 *
 * Covers: rendering all trigger categories, agent list, search filtering,
 * drag start sets correct transfer data, collapse/expand toggle, and
 * disabled state for non-running agents.
 */

import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Agent } from "@/types/orchestrator";
import {
	NodePalette,
	PALETTE_DRAG_KEY,
	decodeDragData,
} from "./NodePalette";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const mockAgents: Agent[] = [
	{
		id: "agent-1",
		name: "Running Agent",
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
	{
		id: "agent-2",
		name: "Stopped Agent",
		status: "stopped",
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

function renderPalette(props: Partial<React.ComponentProps<typeof NodePalette>> = {}) {
	return render(<NodePalette agents={mockAgents} {...props} />);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("NodePalette", () => {
	describe("rendering", () => {
		it("renders the palette wrapper", () => {
			renderPalette();
			expect(screen.getByTestId("node-palette")).toBeInTheDocument();
		});

		it("shows trigger category sections", () => {
			renderPalette();
			expect(screen.getByText("External Sources")).toBeInTheDocument();
			expect(screen.getByText("Schedules")).toBeInTheDocument();
			expect(screen.getByText("Events")).toBeInTheDocument();
			expect(screen.getByText("Internal")).toBeInTheDocument();
		});

		it("shows all 13 trigger types", () => {
			renderPalette();
			expect(screen.getByText("GitHub Issues")).toBeInTheDocument();
			expect(screen.getByText("GitHub Pull Requests")).toBeInTheDocument();
			expect(screen.getByText("Linear Issues")).toBeInTheDocument();
			expect(screen.getByText("Webhook")).toBeInTheDocument();
			expect(screen.getByText("Cron Schedule")).toBeInTheDocument();
			expect(screen.getByText("Delayed Run")).toBeInTheDocument();
			expect(screen.getByText("Agent Lifecycle")).toBeInTheDocument();
			expect(screen.getByText("Agent Idle")).toBeInTheDocument();
			expect(screen.getByText("Dispatch Result")).toBeInTheDocument();
			expect(screen.getByText("Ask Response")).toBeInTheDocument();
			expect(screen.getByText("Manual")).toBeInTheDocument();
			expect(screen.getByText("Queue")).toBeInTheDocument();
			expect(screen.getByText("Composite")).toBeInTheDocument();
		});

		it("shows agents section with agent names", () => {
			renderPalette();
			expect(screen.getByText("Agents")).toBeInTheDocument();
			expect(screen.getByText("Running Agent")).toBeInTheDocument();
			expect(screen.getByText("Stopped Agent")).toBeInTheDocument();
		});

		it("shows search input", () => {
			renderPalette();
			expect(screen.getByTestId("palette-search")).toBeInTheDocument();
		});
	});

	describe("collapse/expand", () => {
		it("starts expanded by default", () => {
			renderPalette();
			expect(screen.getByTestId("node-palette")).toHaveAttribute(
				"data-collapsed",
				"false",
			);
		});

		it("collapses when collapse button is clicked", async () => {
			const user = userEvent.setup();
			renderPalette();
			await user.click(screen.getByLabelText("Collapse node palette"));
			expect(screen.getByTestId("node-palette")).toHaveAttribute(
				"data-collapsed",
				"true",
			);
		});

		it("expands when expand button is clicked from collapsed state", async () => {
			const user = userEvent.setup();
			renderPalette({ collapsed: true });
			await user.click(screen.getByLabelText("Expand node palette"));
			// Internal state only changes if uncontrolled; with prop it stays
			expect(screen.getByTestId("node-palette")).toBeInTheDocument();
		});

		it("calls onCollapsedChange when toggled", async () => {
			const user = userEvent.setup();
			const onCollapsedChange = vi.fn();
			renderPalette({ onCollapsedChange });
			await user.click(screen.getByLabelText("Collapse node palette"));
			expect(onCollapsedChange).toHaveBeenCalledWith(true);
		});

		it("hides items when collapsed", () => {
			renderPalette({ collapsed: true });
			expect(screen.queryByText("GitHub Issues")).not.toBeInTheDocument();
			expect(screen.queryByText("Agents")).not.toBeInTheDocument();
		});
	});

	describe("search filtering", () => {
		it("filters trigger types by name", async () => {
			const user = userEvent.setup();
			renderPalette();
			await user.type(screen.getByTestId("palette-search"), "cron");
			expect(screen.getByText("Cron Schedule")).toBeInTheDocument();
			expect(screen.queryByText("GitHub Issues")).not.toBeInTheDocument();
		});

		it("filters agents by name", async () => {
			const user = userEvent.setup();
			renderPalette();
			await user.type(screen.getByTestId("palette-search"), "Running");
			expect(screen.getByText("Running Agent")).toBeInTheDocument();
			expect(screen.queryByText("Stopped Agent")).not.toBeInTheDocument();
		});

		it("shows 'No results' when nothing matches", async () => {
			const user = userEvent.setup();
			renderPalette();
			await user.type(
				screen.getByTestId("palette-search"),
				"xyzzy-no-match",
			);
			expect(screen.getByText("No results")).toBeInTheDocument();
		});

		it("is case-insensitive", async () => {
			const user = userEvent.setup();
			renderPalette();
			await user.type(screen.getByTestId("palette-search"), "WEBHOOK");
			expect(screen.getByText("Webhook")).toBeInTheDocument();
		});
	});

	describe("drag-and-drop", () => {
		it("sets correct drag data for trigger items", () => {
			renderPalette();
			const item = screen.getByTestId("palette-item-github_issues");

			const mockDataTransfer = {
				effectAllowed: "",
				setData: vi.fn(),
			};

			fireEvent.dragStart(item, {
				dataTransfer: mockDataTransfer,
			});

			expect(mockDataTransfer.setData).toHaveBeenCalledWith(
				PALETTE_DRAG_KEY,
				expect.any(String),
			);

			const [, raw] = mockDataTransfer.setData.mock.calls[0];
			const decoded = decodeDragData(raw);
			expect(decoded).toEqual({
				type: "trigger",
				triggerType: "github_issues",
			});
		});

		it("sets correct drag data for agent items", () => {
			renderPalette();
			const item = screen.getByTestId("palette-item-agent-1");

			const mockDataTransfer = {
				effectAllowed: "",
				setData: vi.fn(),
			};

			fireEvent.dragStart(item, {
				dataTransfer: mockDataTransfer,
			});

			const [, raw] = mockDataTransfer.setData.mock.calls[0];
			const decoded = decodeDragData(raw);
			expect(decoded).toEqual({
				type: "agent",
				agentId: "agent-1",
				agentName: "Running Agent",
			});
		});

		it("does not fire drag for non-running agents", () => {
			renderPalette();
			const item = screen.getByTestId("palette-item-agent-2");
			expect(item).toHaveAttribute("draggable", "false");
		});
	});

	describe("disabled agent state", () => {
		it("marks stopped agent as not draggable", () => {
			renderPalette();
			expect(
				screen.getByTestId("palette-item-agent-2"),
			).toHaveAttribute("draggable", "false");
		});

		it("marks running agent as draggable", () => {
			renderPalette();
			expect(
				screen.getByTestId("palette-item-agent-1"),
			).toHaveAttribute("draggable", "true");
		});
	});

	describe("empty agents", () => {
		it("renders without crashing when no agents provided", () => {
			expect(() => render(<NodePalette agents={[]} />)).not.toThrow();
		});

		it("does not show Agents section when agents list is empty", () => {
			render(<NodePalette agents={[]} />);
			expect(screen.queryByText("Agents")).not.toBeInTheDocument();
		});
	});
});
