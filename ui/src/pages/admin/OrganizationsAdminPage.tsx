import { useCallback } from "react";
import { type AdminColumn, AdminTable } from "@/components/admin/AdminTable";
import { useAdminResource } from "@/hooks/useAdminResource";
import { productAdminClient } from "@/services/productAdmin";
import type { AdminOrganization } from "@/types/admin";

const COLUMNS: AdminColumn<AdminOrganization>[] = [
	{ header: "Name", render: (o) => o.name },
	{ header: "Slug", render: (o) => o.slug },
	{
		header: "ID",
		render: (o) => <span className="font-mono text-xs">{o.id}</span>,
	},
	{ header: "Created", render: (o) => new Date(o.created_at).toLocaleString() },
];

export function OrganizationsAdminPage() {
	const loader = useCallback(
		(p: { limit: number; offset: number }) =>
			productAdminClient.listOrganizations(p),
		[],
	);
	const { items, total, offset, limit, loading, error, setOffset, refetch } =
		useAdminResource<AdminOrganization>(loader);

	return (
		<AdminTable
			title="Organizations"
			description="All organizations across the product."
			columns={COLUMNS}
			rows={items}
			rowKey={(o) => o.id}
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

export default OrganizationsAdminPage;
