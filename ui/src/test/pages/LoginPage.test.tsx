/**
 * LoginPage — verifies a successful login stores the session and redirects to
 * the originally-requested page, surfaces server errors, disables the button
 * while in flight, and links to the register page.
 *
 * The login call goes through MSW (not a mocked client) so the real auth.ts
 * client and authStore wiring are exercised end to end.
 */

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import {
	type InitialEntry,
	MemoryRouter,
	Route,
	Routes,
} from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { LoginPage } from "@/pages/LoginPage";
import type { AuthResponse } from "@/services/auth";
import { serviceConfig } from "@/services/config";
import { useAuthStore } from "@/stores/authStore";
import { server } from "@/test/mocks/server";

const LOGIN_URL = `${serviceConfig.coreServiceUrl}/auth/login`;

function authResponse(): AuthResponse {
	return {
		token: "session-token-123",
		user: {
			id: "u1",
			username: "alice",
			email: "alice@example.com",
			display_name: "Alice",
			role: "user",
			is_superuser: false,
			active_organization_id: null,
			created_at: "2026-01-01T00:00:00Z",
			updated_at: "2026-01-01T00:00:00Z",
		},
		active_organization: null,
	};
}

/**
 * Render LoginPage at /login with sibling routes so we can assert where a
 * successful login (or the register link) navigates to.
 */
function renderLogin(initialEntries: InitialEntry[] = ["/login"]) {
	return render(
		<MemoryRouter initialEntries={initialEntries}>
			<Routes>
				<Route path="/login" element={<LoginPage />} />
				<Route path="/" element={<div>dashboard page</div>} />
				<Route path="/agents" element={<div>agents page</div>} />
				<Route path="/register" element={<div>register page</div>} />
			</Routes>
		</MemoryRouter>,
	);
}

describe("LoginPage", () => {
	beforeEach(() => {
		localStorage.clear();
		useAuthStore.setState({
			token: null,
			user: null,
			isAuthenticated: false,
			sessionChecked: false,
		});
	});

	afterEach(() => {
		localStorage.clear();
	});

	it("logs in, stores the session, and redirects to the requested page", async () => {
		server.use(http.post(LOGIN_URL, () => HttpResponse.json(authResponse())));

		renderLogin([
			{ pathname: "/login", state: { from: { pathname: "/agents" } } },
		]);

		await userEvent.type(screen.getByLabelText(/username/i), "alice");
		await userEvent.type(screen.getByLabelText(/password/i), "hunter2");
		await userEvent.click(screen.getByRole("button", { name: /sign in/i }));

		expect(await screen.findByText("agents page")).toBeInTheDocument();
		const state = useAuthStore.getState();
		expect(state.isAuthenticated).toBe(true);
		expect(state.token).toBe("session-token-123");
		expect(state.user?.username).toBe("alice");
		expect(localStorage.getItem("agentd_token")).toBe("session-token-123");
	});

	it("defaults the post-login redirect to the dashboard when no origin is set", async () => {
		server.use(http.post(LOGIN_URL, () => HttpResponse.json(authResponse())));

		renderLogin();

		await userEvent.type(screen.getByLabelText(/username/i), "alice");
		await userEvent.type(screen.getByLabelText(/password/i), "hunter2");
		await userEvent.click(screen.getByRole("button", { name: /sign in/i }));

		expect(await screen.findByText("dashboard page")).toBeInTheDocument();
	});

	it("surfaces an error and does not authenticate when credentials are rejected", async () => {
		// The core service returns 401 for bad credentials. The shared ApiClient
		// special-cases 401 (clears the token and surfaces "Session expired"), so
		// that is the message the form renders — the important guarantee here is
		// that no session is established on a failed login.
		server.use(
			http.post(LOGIN_URL, () =>
				HttpResponse.json({ error: "invalid credentials" }, { status: 401 }),
			),
		);

		renderLogin();

		await userEvent.type(screen.getByLabelText(/username/i), "alice");
		await userEvent.type(screen.getByLabelText(/password/i), "wrong");
		await userEvent.click(screen.getByRole("button", { name: /sign in/i }));

		expect(await screen.findByText(/session expired/i)).toBeInTheDocument();
		expect(useAuthStore.getState().isAuthenticated).toBe(false);
		expect(localStorage.getItem("agentd_token")).toBeNull();
		// Still on the login form — no client-side redirect occurred.
		expect(
			screen.getByRole("button", { name: /sign in/i }),
		).toBeInTheDocument();
	});

	it("surfaces a non-401 server error message verbatim", async () => {
		server.use(
			http.post(LOGIN_URL, () =>
				HttpResponse.json({ error: "account is locked" }, { status: 403 }),
			),
		);

		renderLogin();

		await userEvent.type(screen.getByLabelText(/username/i), "alice");
		await userEvent.type(screen.getByLabelText(/password/i), "hunter2");
		await userEvent.click(screen.getByRole("button", { name: /sign in/i }));

		expect(await screen.findByText("account is locked")).toBeInTheDocument();
		expect(useAuthStore.getState().isAuthenticated).toBe(false);
	});

	it("disables the submit button and shows a pending label while logging in", async () => {
		let resolve: (() => void) | undefined;
		const gate = new Promise<void>((r) => {
			resolve = r;
		});
		server.use(
			http.post(LOGIN_URL, async () => {
				await gate;
				return HttpResponse.json(authResponse());
			}),
		);

		renderLogin();

		await userEvent.type(screen.getByLabelText(/username/i), "alice");
		await userEvent.type(screen.getByLabelText(/password/i), "hunter2");
		await userEvent.click(screen.getByRole("button", { name: /sign in/i }));

		const button = await screen.findByRole("button", { name: /signing in/i });
		expect(button).toBeDisabled();

		resolve?.();
		expect(await screen.findByText("dashboard page")).toBeInTheDocument();
	});

	it("links to the register page", async () => {
		renderLogin();

		await userEvent.click(screen.getByRole("link", { name: /register/i }));
		await waitFor(() =>
			expect(screen.getByText("register page")).toBeInTheDocument(),
		);
	});
});
