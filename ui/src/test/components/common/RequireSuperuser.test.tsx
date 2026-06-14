/**
 * RequireSuperuser guard — verifies superusers reach gated routes, non-superusers
 * are redirected, and the loading state shows before the session is checked.
 *
 * Note: this guard is a UX convenience only — the core service independently
 * enforces superuser access on every /api/v1/admin/* endpoint.
 */

import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";
import { RequireSuperuser } from "@/components/common/RequireSuperuser";
import type { User } from "@/services/auth";
import { useAuthStore } from "@/stores/authStore";

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

function renderGuard() {
	return render(
		<MemoryRouter initialEntries={["/admin"]}>
			<Routes>
				<Route path="/" element={<div>home page</div>} />
				<Route element={<RequireSuperuser />}>
					<Route path="/admin" element={<div>admin area</div>} />
				</Route>
			</Routes>
		</MemoryRouter>,
	);
}

describe("RequireSuperuser", () => {
	beforeEach(() => {
		useAuthStore.setState({
			token: "t",
			user: null,
			isAuthenticated: true,
			sessionChecked: false,
		});
	});

	it("shows a loading state until the session has been checked", () => {
		useAuthStore.setState({ sessionChecked: false });
		renderGuard();
		expect(screen.getByText(/checking access/i)).toBeInTheDocument();
		expect(screen.queryByText("admin area")).not.toBeInTheDocument();
	});

	it("renders the gated route for a superuser", () => {
		useAuthStore.setState({
			sessionChecked: true,
			user: makeUser({ is_superuser: true }),
		});
		renderGuard();
		expect(screen.getByText("admin area")).toBeInTheDocument();
	});

	it("redirects a non-superuser away from the gated route", () => {
		useAuthStore.setState({
			sessionChecked: true,
			user: makeUser({ is_superuser: false }),
		});
		renderGuard();
		expect(screen.getByText("home page")).toBeInTheDocument();
		expect(screen.queryByText("admin area")).not.toBeInTheDocument();
	});
});
