/**
 * PolicyDisplay — read-only view of an agent's tool policy.
 *
 * Shown in the Tool Policy section when the user is not actively editing.
 * Pairs with AgentPolicyEditor which is shown during editing.
 */

import { useState } from "react";
import { ChevronDown, ChevronRight, ShieldAlert } from "lucide-react";
import type { ToolPolicy } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PolicyDisplayProps {
	policy: ToolPolicy;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const MODE_LABELS: Record<ToolPolicy["mode"], string> = {
	allow_all: "Allow All",
	deny_all: "Deny All",
	allow_list: "Allow List",
	deny_list: "Deny List",
	require_approval: "Require Approval",
};

// ---------------------------------------------------------------------------
// PolicyDisplay
// ---------------------------------------------------------------------------

export function PolicyDisplay({ policy }: PolicyDisplayProps) {
	const [bypassExpanded, setBypassExpanded] = useState(false);

	const label = MODE_LABELS[policy.mode];
	const tools =
		policy.mode === "allow_list" || policy.mode === "deny_list"
			? policy.tools
			: [];
	const sandboxBypass = policy.sandbox_bypass ?? [];

	return (
		<dl className="flex flex-col gap-2 text-sm">
			<div className="flex items-center gap-2">
				<dt className="text-xs font-medium text-th-text-muted w-24 shrink-0">
					Policy type
				</dt>
				<dd className="font-medium text-th-text">{label}</dd>
			</div>

			{tools.length > 0 && (
				<div className="flex items-start gap-2">
					<dt className="text-xs font-medium text-th-text-muted w-24 shrink-0 pt-0.5">
						Tools
					</dt>
					<dd className="flex flex-wrap gap-1">
						{tools.map((tool) => (
							<span
								key={tool}
								className="inline-flex items-center rounded bg-th-surface-sunken px-2 py-0.5 text-xs font-mono text-th-text-secondary"
							>
								{tool}
							</span>
						))}
					</dd>
				</div>
			)}

			{tools.length === 0 &&
				policy.mode !== "allow_all" &&
				policy.mode !== "deny_all" && (
					<div className="flex items-center gap-2">
						<dt className="text-xs font-medium text-th-text-muted w-24 shrink-0">
							Tools
						</dt>
						<dd className="text-th-text-faint italic">
							None configured
						</dd>
					</div>
				)}

			{sandboxBypass.length > 0 && (
				<div className="flex items-start gap-2">
					<dt className="text-xs font-medium text-th-text-muted w-24 shrink-0 pt-0.5">
						{/* spacer — label is in the collapsible header */}
					</dt>
					<dd className="flex-1 min-w-0">
						<button
							type="button"
							onClick={() => setBypassExpanded((v) => !v)}
							className="flex items-center gap-1.5 text-amber-600 dark:text-amber-400 hover:text-amber-700 dark:hover:text-amber-300 transition-colors"
							aria-expanded={bypassExpanded}
						>
							<ShieldAlert className="w-3.5 h-3.5 shrink-0" />
							<span className="text-xs font-medium">
								Sandbox bypass
							</span>
							<span className="ml-1 inline-flex items-center rounded-full bg-amber-100 dark:bg-amber-900/40 px-1.5 py-0.5 text-xs font-medium text-amber-700 dark:text-amber-300">
								{sandboxBypass.length}
							</span>
							{bypassExpanded ? (
								<ChevronDown className="w-3 h-3 ml-0.5" />
							) : (
								<ChevronRight className="w-3 h-3 ml-0.5" />
							)}
						</button>

						{bypassExpanded && (
							<ul className="mt-1.5 flex flex-col gap-1">
								{sandboxBypass.map((glob) => (
									<li key={glob}>
										<span className="inline-flex items-center rounded bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 px-2 py-0.5 text-xs font-mono text-amber-800 dark:text-amber-300">
											{glob}
										</span>
									</li>
								))}
							</ul>
						)}
					</dd>
				</div>
			)}
		</dl>
	);
}

export default PolicyDisplay;
