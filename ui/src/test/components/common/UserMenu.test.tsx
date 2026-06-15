/**
 * UserMenu — header avatar dropdown for the signed-in user.
 *
 * Verifies the avatar initials, open/close behaviour (toggle, outside click,
 * Escape), the superuser badge, the Settings link, and the logout flow which
 * clears the session both server-side (via MSW) and locally before routing to
 * /login.
 */

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { UserMenu } from "@/components/common/UserMenu";
import type { User } from "@/services/auth";
import { serviceConfig } from "@/services/config";
import { useAuthStore } from "@/stores/authStore";
import { server } from "@/test/mocks/server";

const LOGOUT_URL = `${serviceConfig.coreServiceUrl}/auth/logout`;

function makeUser(overrides?: Partial<User>): User {
	return {
		id: "u1",
		username: "alice",
		email: "alice@example.com",
		display_name: "Alice Anderson",
		role: "user",
		is_superuser: false,
		active_organization_id: null,
		created_at: "2026-01-01T00:00:00Z",
		updated_at: "2026-01-01T00:00:00Z",
		...overrides,
	};
}

function renderMenu() {
	return render(
		<MemoryRouter initialEntries={["/"]}>
			<Routes>
				<Route path="/" element={<UserMenu />} />
				<Route path="/login" element={<div>login page</div>} />
				<Route path="/settings" element={<div>settings page</div>} />
			</Routes>
		</MemoryRouter>,
	);
}

describe("UserMenu", () => {
	beforeEach(() => {
		localStorage.clear();
		useAuthStore.setState({
			token: "session-token",
			user: makeUser(),
			isAuthenticated: true,
			sessionChecked: true,
		});
		localStorage.setItem("agentd_token", "session-token");
	});

	afterEach(() => {
		localStorage.clear();
	});

	it("renders two-letter initials derived from the display name", () => {
		renderMenu();
		expect(
			screen.getByRole("button", { name: /user menu/i }),
		).toHaveTextContent("AA");
	});

	it("opens the menu and shows the user's name and email", async () => {
		renderMenu();

		expect(screen.queryByText("alice@example.com")).not.toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: /user menu/i }));

		expect(screen.getByText("Alice Anderson")).toBeInTheDocument();
		expect(screen.getByText("alice@example.com")).toBeInTheDocument();
	});

	it("shows the superuser badge only for superusers", async () => {
		useAuthStore.setState({ user: makeUser({ is_superuser: true }) });
		renderMenu();

		await userEvent.click(screen.getByRole("button", { name: /user menu/i }));
		expect(screen.getByText("superuser")).toBeInTheDocument();
	});

	it("does not show the superuser badge for a regular user", async () => {
		renderMenu();
		await userEvent.click(screen.getByRole("button", { name: /user menu/i }));
		expect(screen.queryByText("superuser")).not.toBeInTheDocument();
	});

	it("closes the menu when Escape is pressed", async () => {
		renderMenu();
		await userEvent.click(screen.getByRole("button", { name: /user menu/i }));
		expect(screen.getByText("Alice Anderson")).toBeInTheDocument();

		await userEvent.keyboard("{Escape}");
		await waitFor(() =>
			expect(screen.queryByText("Alice Anderson")).not.toBeInTheDocument(),
		);
	});

	it("closes the menu on an outside click", async () => {
		render(
			<MemoryRouter initialEntries={["/"]}>
				<Routes>
					<Route
						path="/"
						element={
							<div>
								<UserMenu />
								<button type="button">outside</button>
							</div>
						}
					/>
				</Routes>
			</MemoryRouter>,
		);

		await userEvent.click(screen.getByRole("button", { name: /user menu/i }));
		expect(screen.getByText("Alice Anderson")).toBeInTheDocument();

		await userEvent.click(screen.getByRole("button", { name: "outside" }));
		await waitFor(() =>
			expect(screen.queryByText("Alice Anderson")).not.toBeInTheDocument(),
		);
	});

	it("navigates to settings via the menu link", async () => {
		renderMenu();
		await userEvent.click(screen.getByRole("button", { name: /user menu/i }));
		await userEvent.click(screen.getByRole("link", { name: /settings/i }));

		expect(await screen.findByText("settings page")).toBeInTheDocument();
	});

	it("logs out: clears the session server-side and locally, then routes to /login", async () => {
		let logoutHit = false;
		server.use(
			http.post(LOGOUT_URL, () => {
				logoutHit = true;
				return new HttpResponse(null, { status: 204 });
			}),
		);

		renderMenu();
		await userEvent.click(screen.getByRole("button", { name: /user menu/i }));
		await userEvent.click(screen.getByRole("button", { name: /log out/i }));

		expect(await screen.findByText("login page")).toBeInTheDocument();
		expect(logoutHit).toBe(true);
		expect(useAuthStore.getState().isAuthenticated).toBe(false);
		expect(localStorage.getItem("agentd_token")).toBeNull();
	});

	it("logs out locally even when the server logout call fails", async () => {
		server.use(http.post(LOGOUT_URL, () => HttpResponse.error()));

		renderMenu();
		await userEvent.click(screen.getByRole("button", { name: /user menu/i }));
		await userEvent.click(screen.getByRole("button", { name: /log out/i }));

		expect(await screen.findByText("login page")).toBeInTheDocument();
		expect(useAuthStore.getState().isAuthenticated).toBe(false);
	});

	it("falls back to the generic user icon when no name is available", () => {
		useAuthStore.setState({
			user: makeUser({ display_name: null, username: null }),
		});
		renderMenu();

		// No initials text — the button renders the lucide user icon instead.
		const button = screen.getByRole("button", { name: /user menu/i });
		expect(button).toHaveTextContent("");
	});
});
