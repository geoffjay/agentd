import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { User } from "@/services/auth";
import { useAuthStore } from "@/stores/authStore";
import App from "../App";

function makeUser(overrides?: Partial<User>): User {
	return {
		id: "u1",
		username: "alice",
		email: "alice@example.com",
		display_name: "Alice",
		role: "user",
		is_superuser: false,
		active_organization_id: null,
		created_at: "2026-01-01T00:00:00Z",
		updated_at: "2026-01-01T00:00:00Z",
		...overrides,
	};
}

describe("App", () => {
	// The protected routes sit behind RequireAuth; seed an authenticated,
	// already-checked session so the app renders the dashboard rather than
	// redirecting to /login. sessionChecked: true prevents RequireAuth from
	// firing a checkSession() request that MSW would reject as unhandled.
	beforeEach(() => {
		useAuthStore.setState({
			token: "t",
			user: makeUser(),
			isAuthenticated: true,
			sessionChecked: true,
		});
	});

	it("renders the app root without crashing", () => {
		render(<App />);
		expect(document.getElementById("root") ?? document.body).toBeTruthy();
	});

	it("renders the header with the agentd logo link", () => {
		render(<App />);
		expect(
			screen.getByRole("link", { name: /agentd home/i }),
		).toBeInTheDocument();
	});

	it("renders the sidebar navigation", () => {
		render(<App />);
		expect(screen.getByRole("navigation")).toBeInTheDocument();
	});

	it("renders the main content area", () => {
		render(<App />);
		expect(screen.getByRole("main")).toBeInTheDocument();
	});

	it("renders the dashboard page by default", () => {
		render(<App />);
		expect(
			screen.getByRole("heading", { name: /dashboard/i }),
		).toBeInTheDocument();
	});
});
