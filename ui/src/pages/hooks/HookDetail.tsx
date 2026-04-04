/**
 * HookDetail — placeholder for the individual hook detail page.
 *
 * This component will display the full configuration and event log for a
 * single hook once the hook service (port 17002) is implemented. For now
 * it renders a minimal placeholder that is ready to be wired up.
 */

import { ArrowLeft, Zap } from "lucide-react";
import { Link, useParams } from "react-router-dom";

export function HookDetail() {
	const { id } = useParams<{ id: string }>();

	return (
		<div id="main-content" className="space-y-6">
			{/* Back nav */}
			<Link
				to="/hooks"
				className="inline-flex items-center gap-2 text-sm text-th-text-muted hover:text-th-text transition-colors focus-visible:outline-none focus-visible:ring-2 focus:ring-th-focus-ring rounded"
			>
				<ArrowLeft size={16} />
				Back to Hooks
			</Link>

			{/* Header */}
			<div className="flex items-center gap-3">
				<div className="flex h-10 w-10 items-center justify-center rounded-lg bg-th-accent-subtle">
					<Zap size={22} className="text-th-text-link" />
				</div>
				<div>
					<h1 className="text-2xl font-semibold text-th-text">
						Hook {id ?? "—"}
					</h1>
					<p className="text-sm text-th-text-muted">
						Hook detail — coming soon
					</p>
				</div>
			</div>

			{/* Placeholder body */}
			<div className="rounded-lg border border-dashed border-th-border bg-th-surface-sunken p-10 text-center">
				<p className="text-th-text-faint">
					Hook detail view will be available once the hook service is
					implemented.
				</p>
			</div>
		</div>
	);
}

export default HookDetail;
