/**
 * Product-admin views — AdminLayout shell plus the four read-only entity pages
 * (Users, Organizations, Memberships, Sessions).
 *
 * Each page is driven through MSW so the real productAdminClient + useAdmin
 * Resource + AdminTable stack is exercised end to end. We assert the page
 * title, a representative cell from the mocked data, and (for AdminLayout) tab
 * navigation between sections.
 */

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HttpResponse, http } from "msw";
import { MemoryRouter, Navigate, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { AdminLayout } from "@/pages/admin/AdminLayout";
import { MembershipsAdminPage } from "@/pages/admin/MembershipsAdminPage";
import { OrganizationsAdminPage } from "@/pages/admin/OrganizationsAdminPage";
import { SessionsAdminPage } from "@/pages/admin/SessionsAdminPage";
import { UsersAdminPage } from "@/pages/admin/UsersAdminPage";
import { serviceConfig } from "@/services/config";
import { server } from "@/test/mocks/server";
import type {
	AdminMembership,
	AdminOrganization,
	AdminSession,
	AdminUser,
} from "@/types/admin";

const base = serviceConfig.coreServiceUrl;

function paginated<T>(items: T[]) {
	return { items, total: items.length, limit: 50, offset: 0 };
}

const USER: AdminUser = {
	id: "user-1",
	username: "alice",
	email: "alice@example.com",
	display_name: "Alice",
	role: "admin",
	is_superuser: true,
	active_organization_id: "org-1",
	created_at: "2026-01-01T00:00:00Z",
	updated_at: "2026-01-01T00:00:00Z",
};

const ORG: AdminOrganization = {
	id: "org-1",
	name: "Acme Inc",
	slug: "acme",
	created_at: "2026-01-01T00:00:00Z",
	updated_at: "2026-01-01T00:00:00Z",
};

const MEMBERSHIP: AdminMembership = {
	id: "mem-1",
	user_id: "user-1",
	organization_id: "org-1",
	role: "owner",
	created_at: "2026-01-01T00:00:00Z",
	updated_at: "2026-01-01T00:00:00Z",
};

const SESSION: AdminSession = {
	id: "sess-1",
	user_id: "user-1",
	expires_at: "2026-12-01T00:00:00Z",
	is_expired: false,
	created_at: "2026-01-01T00:00:00Z",
};

beforeEach(() => {
	// productAdminClient uses withAuth(), which reads the bearer token from
	// localStorage. The MSW handlers don't check it, but a token keeps the
	// client path realistic.
	localStorage.setItem("agentd_token", "session-token");
});

afterEach(() => {
	localStorage.clear();
});

describe("UsersAdminPage", () => {
	it("renders the users table from the admin API", async () => {
		const regular: AdminUser = {
			...USER,
			id: "user-2",
			username: "bob",
			email: "bob@example.com",
			display_name: null,
			is_superuser: false,
		};
		server.use(
			http.get(`${base}/api/v1/admin/users`, () =>
				HttpResponse.json(paginated([USER, regular])),
			),
		);
		render(<UsersAdminPage />);

		expect(await screen.findByText("alice@example.com")).toBeInTheDocument();
		expect(screen.getByText("bob@example.com")).toBeInTheDocument();
		expect(screen.getByRole("heading", { name: "Users" })).toBeInTheDocument();
		// is_superuser cell renders the badge for the superuser only; the
		// regular user (and bob's missing display name) render a "-" placeholder.
		expect(screen.getByText("superuser")).toBeInTheDocument();
		expect(screen.getAllByText("-").length).toBeGreaterThan(0);
	});

	it("renders the table error state when the API fails", async () => {
		server.use(
			http.get(`${base}/api/v1/admin/users`, () =>
				HttpResponse.json({ error: "forbidden" }, { status: 403 }),
			),
		);
		render(<UsersAdminPage />);

		const alert = await screen.findByRole("alert");
		expect(alert).toHaveTextContent("forbidden");
	});
});

describe("OrganizationsAdminPage", () => {
	it("renders the organizations table from the admin API", async () => {
		server.use(
			http.get(`${base}/api/v1/admin/organizations`, () =>
				HttpResponse.json(paginated([ORG])),
			),
		);
		render(<OrganizationsAdminPage />);

		expect(await screen.findByText("Acme Inc")).toBeInTheDocument();
		expect(screen.getByText("acme")).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "Organizations" }),
		).toBeInTheDocument();
	});
});

describe("MembershipsAdminPage", () => {
	it("renders the memberships table from the admin API", async () => {
		server.use(
			http.get(`${base}/api/v1/admin/memberships`, () =>
				HttpResponse.json(paginated([MEMBERSHIP])),
			),
		);
		render(<MembershipsAdminPage />);

		expect(await screen.findByText("owner")).toBeInTheDocument();
		expect(screen.getByText("user-1")).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "Memberships" }),
		).toBeInTheDocument();
	});
});

describe("SessionsAdminPage", () => {
	it("renders an active session from the admin API", async () => {
		server.use(
			http.get(`${base}/api/v1/admin/sessions`, () =>
				HttpResponse.json(paginated([SESSION])),
			),
		);
		render(<SessionsAdminPage />);

		expect(await screen.findByText("active")).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "Sessions" }),
		).toBeInTheDocument();
	});

	it("labels an expired session as expired", async () => {
		server.use(
			http.get(`${base}/api/v1/admin/sessions`, () =>
				HttpResponse.json(paginated([{ ...SESSION, is_expired: true }])),
			),
		);
		render(<SessionsAdminPage />);

		expect(await screen.findByText("expired")).toBeInTheDocument();
	});
});

describe("AdminLayout", () => {
	function renderLayout() {
		server.use(
			http.get(`${base}/api/v1/admin/users`, () =>
				HttpResponse.json(paginated([USER])),
			),
			http.get(`${base}/api/v1/admin/organizations`, () =>
				HttpResponse.json(paginated([ORG])),
			),
		);
		return render(
			<MemoryRouter initialEntries={["/admin/users"]}>
				<Routes>
					<Route path="/admin" element={<AdminLayout />}>
						<Route index element={<Navigate to="/admin/users" replace />} />
						<Route path="users" element={<UsersAdminPage />} />
						<Route path="organizations" element={<OrganizationsAdminPage />} />
					</Route>
				</Routes>
			</MemoryRouter>,
		);
	}

	it("renders the heading and section tabs", async () => {
		renderLayout();

		expect(
			screen.getByRole("heading", { name: /product admin/i }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("navigation", { name: /admin sections/i }),
		).toBeInTheDocument();
		// The default (users) view is rendered via the Outlet.
		expect(await screen.findByText("alice@example.com")).toBeInTheDocument();
	});

	it("switches the outlet content when a different tab is clicked", async () => {
		renderLayout();
		await screen.findByText("alice@example.com");

		await userEvent.click(screen.getByRole("link", { name: "Organizations" }));

		expect(await screen.findByText("Acme Inc")).toBeInTheDocument();
		await waitFor(() =>
			expect(screen.queryByText("alice@example.com")).not.toBeInTheDocument(),
		);
	});
});
