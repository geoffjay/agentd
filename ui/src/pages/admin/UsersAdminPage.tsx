import { useCallback } from "react";
import { type AdminColumn, AdminTable } from "@/components/admin/AdminTable";
import { useAdminResource } from "@/hooks/useAdminResource";
import { productAdminClient } from "@/services/productAdmin";
import type { AdminUser } from "@/types/admin";

const COLUMNS: AdminColumn<AdminUser>[] = [
	{ header: "Email", render: (u) => u.email },
	{ header: "Username", render: (u) => u.username ?? "-" },
	{ header: "Display name", render: (u) => u.display_name ?? "-" },
	{ header: "Role", render: (u) => u.role },
	{
		header: "Superuser",
		render: (u) =>
			u.is_superuser ? (
				<span className="rounded-full bg-th-accent/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-th-accent">
					superuser
				</span>
			) : (
				<span className="text-th-text-faint">-</span>
			),
	},
	{ header: "Created", render: (u) => new Date(u.created_at).toLocaleString() },
];

export function UsersAdminPage() {
	const loader = useCallback(
		(p: { limit: number; offset: number }) => productAdminClient.listUsers(p),
		[],
	);
	const { items, total, offset, limit, loading, error, setOffset, refetch } =
		useAdminResource<AdminUser>(loader);

	return (
		<AdminTable
			title="Users"
			description="All registered users across the product."
			columns={COLUMNS}
			rows={items}
			rowKey={(u) => u.id}
			loading={loading}
			error={error}
			total={total}
			offset={offset}
			limit={limit}
			onPage={setOffset}
			onRefresh={refetch}
		/>
	);
}

export default UsersAdminPage;
