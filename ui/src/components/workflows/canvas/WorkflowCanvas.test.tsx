/**
 * WorkflowCanvas tests.
 *
 * Verifies that the canvas renders without errors, that the React Flow
 * sub-components are present in the DOM, and that readOnly mode shows the
 * view-mode badge and sets the correct data attribute.
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

	describe("basic rendering", () => {
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

	describe("readOnly mode", () => {
		it("does not show view-mode badge by default", () => {
			render(<WorkflowCanvas {...defaultProps} />);
			expect(screen.queryByTestId("readonly-badge")).not.toBeInTheDocument();
		});

		it("shows view-mode badge when readOnly is true", () => {
			render(<WorkflowCanvas {...defaultProps} readOnly />);
			expect(screen.getByTestId("readonly-badge")).toBeInTheDocument();
		});

		it("displays 'View mode' text in the badge", () => {
			render(<WorkflowCanvas {...defaultProps} readOnly />);
			expect(screen.getByText("View mode")).toBeInTheDocument();
		});

		it("sets data-readonly=false by default", () => {
			render(<WorkflowCanvas {...defaultProps} />);
			expect(screen.getByTestId("workflow-canvas")).toHaveAttribute(
				"data-readonly",
				"false",
			);
		});

		it("sets data-readonly=true when readOnly prop is set", () => {
			render(<WorkflowCanvas {...defaultProps} readOnly />);
			expect(screen.getByTestId("workflow-canvas")).toHaveAttribute(
				"data-readonly",
				"true",
			);
		});
	});
});
