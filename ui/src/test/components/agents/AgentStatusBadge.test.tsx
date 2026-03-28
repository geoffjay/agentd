import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AgentStatusBadge } from "@/components/agents/AgentStatusBadge";

describe("AgentStatusBadge", () => {
	it("renders Running status", () => {
		render(<AgentStatusBadge status="running" />);
		expect(screen.getByText(/running/i)).toBeInTheDocument();
	});

	it("renders Pending status", () => {
		render(<AgentStatusBadge status="pending" />);
		expect(screen.getByText(/pending/i)).toBeInTheDocument();
	});

	it("renders Stopped status", () => {
		render(<AgentStatusBadge status="stopped" />);
		expect(screen.getByText(/stopped/i)).toBeInTheDocument();
	});

	it("renders Failed status", () => {
		render(<AgentStatusBadge status="failed" />);
		expect(screen.getByText(/failed/i)).toBeInTheDocument();
	});

	it("defaults to badge variant", () => {
		const { container } = render(<AgentStatusBadge status="running" />);
		// Badge variant renders a span with text; dot variant would render a circle without text
		expect(screen.getByText(/running/i)).toBeInTheDocument();
		expect(container.firstChild).toBeInTheDocument();
	});
});
