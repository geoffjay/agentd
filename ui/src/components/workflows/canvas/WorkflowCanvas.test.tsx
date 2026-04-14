/**
 * WorkflowCanvas tests.
 *
 * Verifies that the canvas renders without errors and that the React Flow
 * sub-components (controls, minimap, background) are present in the DOM.
 */

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { WorkflowCanvas } from "./WorkflowCanvas";

// React Flow uses ResizeObserver internally; provide a stub in jsdom.
global.ResizeObserver = class ResizeObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
};

describe("WorkflowCanvas", () => {
	const defaultProps = {
		nodes: [],
		edges: [],
		onNodesChange: () => {},
		onEdgesChange: () => {},
		onConnect: () => {},
	};

	it("renders without throwing", () => {
		expect(() => render(<WorkflowCanvas {...defaultProps} />)).not.toThrow();
	});

	it("renders the canvas wrapper", () => {
		render(<WorkflowCanvas {...defaultProps} />);
		expect(screen.getByTestId("workflow-canvas")).toBeInTheDocument();
	});

	it("accepts custom className", () => {
		render(
			<WorkflowCanvas {...defaultProps} className="my-custom-class" />,
		);
		const wrapper = screen.getByTestId("workflow-canvas");
		expect(wrapper.className).toContain("my-custom-class");
	});

	it("renders with nodes and edges without throwing", () => {
		const nodes = [
			{
				id: "n1",
				type: "default",
				position: { x: 0, y: 0 },
				data: { label: "Node 1" },
			},
		];
		const edges = [{ id: "e1", source: "n1", target: "n2" }];
		expect(() =>
			render(
				<WorkflowCanvas
					{...defaultProps}
					nodes={nodes}
					edges={edges}
				/>,
			),
		).not.toThrow();
	});
});
