/**
 * AgentNode tests.
 *
 * Verifies that the agent node renders name, status badge, model, tool policy
 * summary, and the input handle. Also verifies selected-state styling and
 * status-dependent border colours.
 */

import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, expect, it } from "vitest";
import type { AgentStatus, ToolPolicy } from "@/types/orchestrator";
import type { AgentNodeData } from "./AgentNode";
import { AgentNode } from "./AgentNode";

// React Flow requires ResizeObserver in jsdom
global.ResizeObserver = class ResizeObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderAgentNode(
	overrides: Partial<AgentNodeData> = {},
	selected = false,
) {
	const data: AgentNodeData = {
		agentId: "agent-1",
		name: "Test Agent",
		status: "running",
		toolPolicy: { mode: "allow_all" },
		...overrides,
	};

	return render(
		<ReactFlowProvider>
			<AgentNode
				id="n1"
				type="agent"
				data={data}
				selected={selected}
				dragging={false}
				zIndex={0}
				isConnectable={true}
				positionAbsoluteX={0}
				positionAbsoluteY={0}
			/>
		</ReactFlowProvider>,
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AgentNode", () => {
	describe("rendering", () => {
		it("renders the node wrapper", () => {
			renderAgentNode();
			expect(screen.getByTestId("agent-node")).toBeInTheDocument();
		});

		it("shows the agent name", () => {
			renderAgentNode({ name: "My Worker Agent" });
			expect(screen.getByText("My Worker Agent")).toBeInTheDocument();
		});

		it("has an input handle on the left", () => {
			renderAgentNode();
			expect(screen.getByTestId("agent-node-handle-in")).toBeInTheDocument();
		});
	});

	describe("tool policy summary", () => {
		it("shows 'Allow all tools' for allow_all", () => {
			renderAgentNode({ toolPolicy: { mode: "allow_all" } });
			expect(screen.getByText("Allow all tools")).toBeInTheDocument();
		});

		it("shows 'Deny all tools' for deny_all", () => {
			renderAgentNode({ toolPolicy: { mode: "deny_all" } });
			expect(screen.getByText("Deny all tools")).toBeInTheDocument();
		});

		it("shows tool count for allow_list", () => {
			const policy: ToolPolicy = {
				mode: "allow_list",
				tools: ["Bash", "Read", "Edit"],
			};
			renderAgentNode({ toolPolicy: policy });
			expect(screen.getByText("3 tools allowed")).toBeInTheDocument();
		});

		it("shows singular 'tool' for single allow_list entry", () => {
			const policy: ToolPolicy = { mode: "allow_list", tools: ["Bash"] };
			renderAgentNode({ toolPolicy: policy });
			expect(screen.getByText("1 tool allowed")).toBeInTheDocument();
		});

		it("shows tool count for deny_list", () => {
			const policy: ToolPolicy = {
				mode: "deny_list",
				tools: ["Bash", "Write"],
			};
			renderAgentNode({ toolPolicy: policy });
			expect(screen.getByText("2 tools denied")).toBeInTheDocument();
		});

		it("shows 'Requires approval' for require_approval", () => {
			renderAgentNode({ toolPolicy: { mode: "require_approval" } });
			expect(screen.getByText("Requires approval")).toBeInTheDocument();
		});
	});

	describe("model display", () => {
		it("shows model when provided", () => {
			renderAgentNode({ model: "claude-sonnet-4-20250514" });
			expect(
				screen.getByTestId("agent-node-model"),
			).toHaveTextContent("claude-sonnet-4-20250514");
		});

		it("does not show model element when not provided", () => {
			renderAgentNode({ model: undefined });
			expect(
				screen.queryByTestId("agent-node-model"),
			).not.toBeInTheDocument();
		});
	});

	describe("status-aware border colouring", () => {
		const statuses: AgentStatus[] = ["running", "failed", "pending", "stopped"];

		for (const status of statuses) {
			it(`renders without throwing for status=${status}`, () => {
				expect(() => renderAgentNode({ status })).not.toThrow();
			});
		}

		it("applies green border for running status", () => {
			renderAgentNode({ status: "running" });
			const node = screen.getByTestId("agent-node");
			expect(node.className).toContain("green");
		});

		it("applies red border for failed status", () => {
			renderAgentNode({ status: "failed" });
			const node = screen.getByTestId("agent-node");
			expect(node.className).toContain("red");
		});
	});

	describe("selected state", () => {
		it("shows ring when selected", () => {
			renderAgentNode({}, true);
			const node = screen.getByTestId("agent-node");
			expect(node.className).toContain("ring-2");
		});

		it("does not show ring when not selected", () => {
			renderAgentNode({}, false);
			const node = screen.getByTestId("agent-node");
			expect(node.className).not.toContain("ring-2");
		});
	});
});
