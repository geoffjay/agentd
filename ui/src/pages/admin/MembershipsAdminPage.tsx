import { useCallback } from "react";
import { type AdminColumn, AdminTable } from "@/components/admin/AdminTable";
import { useAdminResource } from "@/hooks/useAdminResource";
import { productAdminClient } from "@/services/productAdmin";
import type { AdminMembership } from "@/types/admin";

const COLUMNS: AdminColumn<AdminMembership>[] = [
	{
		header: "User ID",
		render: (m) => <span className="font-mono text-xs">{m.user_id}</span>,
	},
	{
		header: "Organization ID",
		render: (m) => (
			<span className="font-mono text-xs">{m.organization_id}</span>
		),
	},
	{ header: "Role", render: (m) => m.role },
	{ header: "Created", render: (m) => new Date(m.created_at).toLocaleString() },
];

export function MembershipsAdminPage() {
	const loader = useCallback(
		(p: { limit: number; offset: number }) =>
			productAdminClient.listMemberships(p),
		[],
	);
	const { items, total, offset, limit, loading, error, setOffset, refetch } =
		useAdminResource<AdminMembership>(loader);

	return (
		<AdminTable
			title="Memberships"
			description="All user-organization memberships across every organization."
			columns={COLUMNS}
			rows={items}
			rowKey={(m) => m.id}
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

export default MembershipsAdminPage;
