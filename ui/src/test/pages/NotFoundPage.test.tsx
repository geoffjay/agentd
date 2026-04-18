/**
 * NotFoundPage + simple wrapper pages — smoke tests.
 */

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

// Mock hooks used by wrapper pages' children
vi.mock("@/hooks/useAgents", () => ({
	useAgents: () => ({
		agents: [],
		allAgents: [],
		total: 0,
		loading: false,
		refreshing: false,
		error: undefined,
		refetch: vi.fn(),
		deleteAgent: vi.fn(),
		restartAgent: vi.fn(),
		terminateAgent: vi.fn(),
	}),
}));

vi.mock("@/hooks/useWorkflows", () => ({
	useWorkflows: () => ({
		workflows: [],
		total: 0,
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

import { NotFoundPage } from "@/pages/NotFoundPage";

describe("NotFoundPage", () => {
	it("renders 404 and a link home", () => {
		render(
			<MemoryRouter>
				<NotFoundPage />
			</MemoryRouter>,
		);
		expect(screen.getByText("404")).toBeInTheDocument();
		expect(screen.getByText("Page not found")).toBeInTheDocument();
		expect(screen.getByText("Return to dashboard")).toBeInTheDocument();
	});
});
