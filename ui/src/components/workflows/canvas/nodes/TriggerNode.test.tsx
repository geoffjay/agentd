/**
 * TriggerNode tests.
 *
 * Verifies that each trigger type variant renders the correct icon label,
 * config summary, handle placement, and selected/disabled visual states.
 */

import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, expect, it } from "vitest";
import type { TriggerConfig } from "@/types/orchestrator";
import { getTriggerCategory } from "@/types/orchestrator";
import type { TriggerNodeData } from "./TriggerNode";
import { TriggerNode } from "./TriggerNode";

// React Flow requires ResizeObserver in jsdom
global.ResizeObserver = class ResizeObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderTriggerNode(
	config: TriggerConfig,
	overrides: Partial<TriggerNodeData> = {},
) {
	const data: TriggerNodeData = {
		triggerConfig: config,
		category: getTriggerCategory(config.type),
		enabled: true,
		...overrides,
	};

	// React Flow nodes must be rendered inside a ReactFlowProvider
	return render(
		<ReactFlowProvider>
			<TriggerNode
				id="n1"
				type="trigger"
				data={data}
				selected={false}
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

describe("TriggerNode", () => {
	describe("github_issues", () => {
		const config: TriggerConfig = {
			type: "github_issues",
			owner: "geoffjay",
			repo: "agentd",
			labels: ["bug", "enhancement"],
			state: "open",
		};

		it("renders the node wrapper", () => {
			renderTriggerNode(config);
			expect(screen.getByTestId("trigger-node")).toBeInTheDocument();
		});

		it("shows the trigger type label", () => {
			renderTriggerNode(config);
			expect(screen.getByText("GitHub Issues")).toBeInTheDocument();
		});

		it("shows owner/repo in summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText(/geoffjay\/agentd/)).toBeInTheDocument();
		});

		it("has an output handle", () => {
			renderTriggerNode(config);
			expect(
				screen.getByTestId("trigger-node-handle-out"),
			).toBeInTheDocument();
		});

		it("shows the category badge", () => {
			renderTriggerNode(config);
			expect(screen.getByText("external")).toBeInTheDocument();
		});
	});

	describe("github_pull_requests", () => {
		const config: TriggerConfig = {
			type: "github_pull_requests",
			owner: "acme",
			repo: "myrepo",
			labels: [],
			state: "open",
		};

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByTestId("trigger-node")).toBeInTheDocument();
		});

		it("shows the correct label", () => {
			renderTriggerNode(config);
			expect(screen.getByText("GitHub Pull Requests")).toBeInTheDocument();
		});
	});

	describe("cron", () => {
		const config: TriggerConfig = {
			type: "cron",
			expression: "0 */6 * * *",
		};

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByTestId("trigger-node")).toBeInTheDocument();
		});

		it("shows the cron expression in summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText("0 */6 * * *")).toBeInTheDocument();
		});

		it("shows schedule category", () => {
			renderTriggerNode(config);
			expect(screen.getByText("schedule")).toBeInTheDocument();
		});
	});

	describe("webhook", () => {
		const config: TriggerConfig = {
			type: "webhook",
			source: "github",
		};

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByText("Webhook")).toBeInTheDocument();
		});

		it("shows source in summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText(/Source: github/)).toBeInTheDocument();
		});
	});

	describe("manual", () => {
		const config: TriggerConfig = { type: "manual" };

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByText("Manual")).toBeInTheDocument();
		});

		it("shows manual trigger summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText("Manual trigger")).toBeInTheDocument();
		});

		it("shows internal category", () => {
			renderTriggerNode(config);
			expect(screen.getByText("internal")).toBeInTheDocument();
		});
	});

	describe("linear_issues", () => {
		const config: TriggerConfig = {
			type: "linear_issues",
			labels: [],
			team_key: "ENG",
			project: "agentd",
		};

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByText("Linear Issues")).toBeInTheDocument();
		});

		it("shows team/project in summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText(/ENG.*agentd/)).toBeInTheDocument();
		});
	});

	describe("agent_lifecycle", () => {
		const config: TriggerConfig = {
			type: "agent_lifecycle",
			event: "session_start",
		};

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByText("Agent Lifecycle")).toBeInTheDocument();
		});

		it("shows event type in summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText("session_start")).toBeInTheDocument();
		});

		it("shows event category", () => {
			renderTriggerNode(config);
			expect(screen.getByText("event")).toBeInTheDocument();
		});
	});

	describe("agent_idle", () => {
		const config: TriggerConfig = {
			type: "agent_idle",
			idle_seconds: 600,
		};

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByText("Agent Idle")).toBeInTheDocument();
		});

		it("shows idle seconds in summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText(/Idle: 600s/)).toBeInTheDocument();
		});
	});

	describe("queue", () => {
		const config: TriggerConfig = {
			type: "queue",
			queue_name: "my-queue",
		};

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByText("Queue")).toBeInTheDocument();
		});

		it("shows queue name in summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText("my-queue")).toBeInTheDocument();
		});
	});

	describe("composite", () => {
		const config: TriggerConfig = {
			type: "composite",
			mode: "and",
			triggers: [],
		};

		it("renders the node", () => {
			renderTriggerNode(config);
			expect(screen.getByText("Composite")).toBeInTheDocument();
		});

		it("shows mode and trigger count in summary", () => {
			renderTriggerNode(config);
			expect(screen.getByText(/AND.*0 trigger/)).toBeInTheDocument();
		});
	});

	describe("selected state", () => {
		it("does not have ring when not selected", () => {
			renderTriggerNode({ type: "manual" });
			const node = screen.getByTestId("trigger-node");
			expect(node.className).not.toContain("ring-2");
		});

		it("has ring-2 class when selected", () => {
			const config: TriggerConfig = { type: "manual" };
			const data: TriggerNodeData = {
				triggerConfig: config,
				category: "internal",
				enabled: true,
			};
			render(
				<ReactFlowProvider>
					<TriggerNode
						id="n1"
						type="trigger"
						data={data}
						selected={true}
						dragging={false}
						zIndex={0}
						isConnectable={true}
						positionAbsoluteX={0}
						positionAbsoluteY={0}
					/>
				</ReactFlowProvider>,
			);
			const node = screen.getByTestId("trigger-node");
			expect(node.className).toContain("ring-2");
		});
	});

	describe("disabled state", () => {
		it("shows Disabled text when enabled=false", () => {
			renderTriggerNode({ type: "manual" }, { enabled: false });
			expect(screen.getByText("Disabled")).toBeInTheDocument();
		});

		it("does not show Disabled text when enabled=true", () => {
			renderTriggerNode({ type: "manual" }, { enabled: true });
			expect(screen.queryByText("Disabled")).not.toBeInTheDocument();
		});
	});

	describe("custom label", () => {
		it("uses custom label when provided", () => {
			renderTriggerNode(
				{ type: "manual" },
				{ label: "My Custom Label" },
			);
			expect(screen.getByText("My Custom Label")).toBeInTheDocument();
		});
	});
});
