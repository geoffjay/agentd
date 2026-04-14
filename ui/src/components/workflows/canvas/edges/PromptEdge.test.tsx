/**
 * PromptEdge tests.
 *
 * Verifies that the prompt edge renders with a truncated template preview,
 * poll interval badge, and that clicking the label triggers the callback.
 * Also tests the visual distinction between empty and customised prompts.
 */

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, expect, it, vi } from "vitest";
import type { PromptEdgeData } from "./PromptEdge";
import { PromptEdge } from "./PromptEdge";

// EdgeLabelRenderer uses a portal to a React Flow container div that does not
// exist in jsdom. Stub it to render children inline so tests can find elements.
vi.mock("@xyflow/react", async (importOriginal) => {
	const actual = await importOriginal<typeof import("@xyflow/react")>();
	return {
		...actual,
		EdgeLabelRenderer: ({ children }: { children: React.ReactNode }) => (
			<div data-testid="edge-label-renderer">{children}</div>
		),
	};
});

// React Flow requires ResizeObserver in jsdom
global.ResizeObserver = class ResizeObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderPromptEdge(data: Partial<PromptEdgeData> = {}) {
	const edgeData: PromptEdgeData = {
		promptTemplate: "",
		pollIntervalSecs: 300,
		enabled: true,
		...data,
	};

	return render(
		<ReactFlowProvider>
			<svg>
				<PromptEdge
					id="e1"
					source="n1"
					target="n2"
					sourceX={0}
					sourceY={0}
					targetX={200}
					targetY={0}
					sourcePosition={"right" as never}
					targetPosition={"left" as never}
					data={edgeData}
					selected={false}
					animated={false}
					type="prompt"
					interactionWidth={20}
				/>
			</svg>
		</ReactFlowProvider>,
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PromptEdge", () => {
	describe("rendering", () => {
		it("renders the edge label", () => {
			renderPromptEdge({ promptTemplate: "Fix this: {{title}}" });
			expect(screen.getByTestId("prompt-edge-label")).toBeInTheDocument();
		});

		it("renders the poll interval badge", () => {
			renderPromptEdge({ pollIntervalSecs: 300 });
			expect(screen.getByTestId("prompt-edge-interval")).toBeInTheDocument();
		});
	});

	describe("template preview", () => {
		it("shows 'No prompt set' for empty template", () => {
			renderPromptEdge({ promptTemplate: "" });
			expect(screen.getByTestId("prompt-edge-label")).toHaveTextContent(
				"No prompt set",
			);
		});

		it("shows first line of the template truncated to 40 chars", () => {
			const template = "A".repeat(60);
			renderPromptEdge({ promptTemplate: template });
			const label = screen.getByTestId("prompt-edge-label");
			expect(label.textContent).toContain("…");
			expect(label.textContent!.length).toBeLessThan(55);
		});

		it("shows short template fully when under 40 chars", () => {
			renderPromptEdge({ promptTemplate: "Fix: {{title}}" });
			expect(screen.getByTestId("prompt-edge-label")).toHaveTextContent(
				"Fix: {{title}}",
			);
		});

		it("shows only first line when template has multiple lines", () => {
			renderPromptEdge({
				promptTemplate: "First line\nSecond line\nThird line",
			});
			const label = screen.getByTestId("prompt-edge-label");
			expect(label.textContent).toContain("First line");
			expect(label.textContent).not.toContain("Second line");
		});
	});

	describe("poll interval badge", () => {
		it("shows seconds for sub-minute intervals", () => {
			renderPromptEdge({ pollIntervalSecs: 30 });
			expect(screen.getByTestId("prompt-edge-interval")).toHaveTextContent(
				"30s",
			);
		});

		it("shows minutes for minute-scale intervals", () => {
			renderPromptEdge({ pollIntervalSecs: 300 });
			expect(screen.getByTestId("prompt-edge-interval")).toHaveTextContent(
				"5m",
			);
		});

		it("shows hours for hour-scale intervals", () => {
			renderPromptEdge({ pollIntervalSecs: 3600 });
			expect(screen.getByTestId("prompt-edge-interval")).toHaveTextContent(
				"1h",
			);
		});
	});

	describe("click interaction", () => {
		it("calls onPromptChange when label is clicked", async () => {
			const user = userEvent.setup();
			const onPromptChange = vi.fn();

			renderPromptEdge({
				promptTemplate: "Fix: {{title}}",
				onPromptChange,
			});

			const label = screen.getByTestId("prompt-edge-label");
			await user.click(label.querySelector("button")!);
			expect(onPromptChange).toHaveBeenCalledOnce();
			expect(onPromptChange).toHaveBeenCalledWith("Fix: {{title}}");
		});
	});
});
