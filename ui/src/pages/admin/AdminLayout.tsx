/**
 * AdminLayout — product-admin section shell.
 *
 * Renders the "Admin" heading and a tab bar linking to the four read-only
 * entity views, with an `<Outlet />` for the active view. Mounted under
 * RequireAuth → RequireSuperuser, so it is only ever reached by superusers.
 */

import { ShieldAlert } from "lucide-react";
import { NavLink, Outlet } from "react-router-dom";

const TABS = [
	{ label: "Users", path: "/admin/users" },
	{ label: "Organizations", path: "/admin/organizations" },
	{ label: "Memberships", path: "/admin/memberships" },
	{ label: "Sessions", path: "/admin/sessions" },
];

export function AdminLayout() {
	return (
		<div className="space-y-6">
			<div>
				<h1 className="flex items-center gap-2 text-2xl font-semibold text-th-text">
					<ShieldAlert size={22} className="text-th-accent" />
					Product Admin
				</h1>
				<p className="mt-1 text-sm text-th-text-muted">
					Product-wide, read-only views of core entities across every
					organization. Superuser only.
				</p>
			</div>

			<nav
				className="flex gap-1 border-b border-th-border"
				aria-label="Admin sections"
			>
				{TABS.map((tab) => (
					<NavLink
						key={tab.path}
						to={tab.path}
						className={({ isActive }) =>
							[
								"-mb-px border-b-2 px-4 py-2 text-sm font-medium transition-colors",
								isActive
									? "border-th-accent text-th-text"
									: "border-transparent text-th-text-muted hover:text-th-text-secondary",
							].join(" ")
						}
					>
						{tab.label}
					</NavLink>
				))}
			</nav>

			<Outlet />
		</div>
	);
}

export default AdminLayout;
