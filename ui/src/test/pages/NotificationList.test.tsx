/**
 * NotificationList — smoke tests for the notifications page.
 */

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/hooks/useNotifications", () => ({
	useNotifications: () => ({
		notifications: [
			{
				id: "n-1",
				title: "Agent stalled",
				message: "Agent abc is unresponsive",
				status: "pending",
				priority: "high",
				source: { type: "system" },
				created_at: new Date().toISOString(),
			},
			{
				id: "n-2",
				title: "Build complete",
				message: "Build finished",
				status: "viewed",
				priority: "low",
				source: { type: "agent_hook" },
				created_at: new Date().toISOString(),
			},
		],
		loading: false,
		error: undefined,
		busyIds: new Set<string>(),
		refetch: vi.fn(),
		markViewed: vi.fn(),
		respond: vi.fn(),
		dismiss: vi.fn(),
		remove: vi.fn(),
		bulkDismiss: vi.fn(),
		bulkDelete: vi.fn(),
		markAllViewed: vi.fn(),
	}),
}));

import { NotificationList } from "@/pages/notifications/NotificationList";

function renderPage() {
	return render(
		<MemoryRouter>
			<NotificationList />
		</MemoryRouter>,
	);
}

describe("NotificationList", () => {
	it("renders the page heading", () => {
		renderPage();
		expect(screen.getByText("Notifications")).toBeInTheDocument();
	});

	it("renders tab navigation", () => {
		renderPage();
		expect(screen.getByRole("tab", { name: "All" })).toBeInTheDocument();
		expect(screen.getByRole("tab", { name: "Actionable" })).toBeInTheDocument();
		expect(screen.getByRole("tab", { name: "History" })).toBeInTheDocument();
	});

	it("renders notification titles in the table", () => {
		renderPage();
		expect(screen.getByText("Agent stalled")).toBeInTheDocument();
		expect(screen.getByText("Build complete")).toBeInTheDocument();
	});

	it("renders refresh button", () => {
		renderPage();
		expect(
			screen.getByRole("button", { name: /refresh notifications/i }),
		).toBeInTheDocument();
	});

	it("shows mark-all-viewed button when pending notifications exist", () => {
		renderPage();
		expect(screen.getByText("Mark all viewed")).toBeInTheDocument();
	});
});
