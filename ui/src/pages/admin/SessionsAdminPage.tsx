import { useCallback } from "react";
import { type AdminColumn, AdminTable } from "@/components/admin/AdminTable";
import { useAdminResource } from "@/hooks/useAdminResource";
import { productAdminClient } from "@/services/productAdmin";
import type { AdminSession } from "@/types/admin";

const COLUMNS: AdminColumn<AdminSession>[] = [
	{
		header: "User ID",
		render: (s) => <span className="font-mono text-xs">{s.user_id}</span>,
	},
	{
		header: "Status",
		render: (s) =>
			s.is_expired ? (
				<span className="text-th-text-faint">expired</span>
			) : (
				<span className="text-th-status-success-text">active</span>
			),
	},
	{ header: "Expires", render: (s) => new Date(s.expires_at).toLocaleString() },
	{ header: "Created", render: (s) => new Date(s.created_at).toLocaleString() },
];

export function SessionsAdminPage() {
	const loader = useCallback(
		(p: { limit: number; offset: number }) =>
			productAdminClient.listSessions(p),
		[],
	);
	const { items, total, offset, limit, loading, error, setOffset, refetch } =
		useAdminResource<AdminSession>(loader);

	return (
		<AdminTable
			title="Sessions"
			description="All active and expired sessions. Token values are never exposed."
			columns={COLUMNS}
			rows={items}
			rowKey={(s) => s.id}
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

export default SessionsAdminPage;
