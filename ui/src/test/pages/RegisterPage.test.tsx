/**
 * RegisterPage — verifies account creation stores the session and redirects,
 * surfaces server errors, disables the button while in flight, and links back
 * to the login page. The register call goes through MSW so the real auth.ts
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
import { RegisterPage } from "@/pages/RegisterPage";
import type { AuthResponse } from "@/services/auth";
import { serviceConfig } from "@/services/config";
import { useAuthStore } from "@/stores/authStore";
import { server } from "@/test/mocks/server";

const REGISTER_URL = `${serviceConfig.coreServiceUrl}/auth/register`;

function authResponse(): AuthResponse {
	return {
		token: "new-session-456",
		user: {
			id: "u2",
			username: "bob",
			email: "bob@example.com",
			display_name: null,
			role: "user",
			is_superuser: false,
			active_organization_id: null,
			created_at: "2026-01-01T00:00:00Z",
			updated_at: "2026-01-01T00:00:00Z",
		},
		active_organization: null,
	};
}

function renderRegister(initialEntries: InitialEntry[] = ["/register"]) {
	return render(
		<MemoryRouter initialEntries={initialEntries}>
			<Routes>
				<Route path="/register" element={<RegisterPage />} />
				<Route path="/" element={<div>dashboard page</div>} />
				<Route path="/login" element={<div>login page</div>} />
			</Routes>
		</MemoryRouter>,
	);
}

describe("RegisterPage", () => {
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

	it("registers, stores the session, and redirects to the dashboard", async () => {
		server.use(
			http.post(REGISTER_URL, () => HttpResponse.json(authResponse())),
		);

		renderRegister();

		await userEvent.type(screen.getByLabelText(/username/i), "bob");
		await userEvent.type(screen.getByLabelText(/email/i), "bob@example.com");
		await userEvent.type(screen.getByLabelText(/password/i), "hunter2");
		await userEvent.click(
			screen.getByRole("button", { name: /create account/i }),
		);

		expect(await screen.findByText("dashboard page")).toBeInTheDocument();
		const state = useAuthStore.getState();
		expect(state.isAuthenticated).toBe(true);
		expect(state.token).toBe("new-session-456");
		expect(state.user?.username).toBe("bob");
	});

	it("redirects back to the originally-requested page after registering", async () => {
		server.use(
			http.post(REGISTER_URL, () => HttpResponse.json(authResponse())),
		);

		renderRegister([
			{ pathname: "/register", state: { from: { pathname: "/" } } },
		]);

		await userEvent.type(screen.getByLabelText(/username/i), "bob");
		await userEvent.type(screen.getByLabelText(/email/i), "bob@example.com");
		await userEvent.type(screen.getByLabelText(/password/i), "hunter2");
		await userEvent.click(
			screen.getByRole("button", { name: /create account/i }),
		);

		expect(await screen.findByText("dashboard page")).toBeInTheDocument();
	});

	it("shows the server error message and does not authenticate on failure", async () => {
		server.use(
			http.post(REGISTER_URL, () =>
				HttpResponse.json({ error: "Username already taken" }, { status: 409 }),
			),
		);

		renderRegister();

		await userEvent.type(screen.getByLabelText(/username/i), "bob");
		await userEvent.type(screen.getByLabelText(/email/i), "bob@example.com");
		await userEvent.type(screen.getByLabelText(/password/i), "hunter2");
		await userEvent.click(
			screen.getByRole("button", { name: /create account/i }),
		);

		expect(
			await screen.findByText("Username already taken"),
		).toBeInTheDocument();
		expect(useAuthStore.getState().isAuthenticated).toBe(false);
	});

	it("disables the submit button and shows a pending label while registering", async () => {
		let resolve: (() => void) | undefined;
		const gate = new Promise<void>((r) => {
			resolve = r;
		});
		server.use(
			http.post(REGISTER_URL, async () => {
				await gate;
				return HttpResponse.json(authResponse());
			}),
		);

		renderRegister();

		await userEvent.type(screen.getByLabelText(/username/i), "bob");
		await userEvent.type(screen.getByLabelText(/email/i), "bob@example.com");
		await userEvent.type(screen.getByLabelText(/password/i), "hunter2");
		await userEvent.click(
			screen.getByRole("button", { name: /create account/i }),
		);

		const button = await screen.findByRole("button", {
			name: /creating account/i,
		});
		expect(button).toBeDisabled();

		resolve?.();
		expect(await screen.findByText("dashboard page")).toBeInTheDocument();
	});

	it("links to the login page", async () => {
		renderRegister();

		await userEvent.click(screen.getByRole("link", { name: /sign in/i }));
		await waitFor(() =>
			expect(screen.getByText("login page")).toBeInTheDocument(),
		);
	});
});
