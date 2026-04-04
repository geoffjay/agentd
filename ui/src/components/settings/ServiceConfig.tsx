/**
 * ServiceConfig — per-service URL configuration with health test buttons.
 */

import { useState } from "react";
import type { Settings } from "@/stores/settingsStore";

interface ServiceRow {
	key: keyof Settings["services"];
	label: string;
	port: number;
}

const SERVICE_ROWS: ServiceRow[] = [
	{ key: "orchestratorUrl", label: "Orchestrator", port: 17006 },
	{ key: "notifyUrl", label: "Notify", port: 17004 },
	{ key: "askUrl", label: "Ask", port: 17001 },
];

type TestStatus = "idle" | "loading" | "success" | "error";

interface ServiceConfigProps {
	services: Settings["services"];
	onSave: (services: Settings["services"]) => void;
}

export function ServiceConfig({ services, onSave }: ServiceConfigProps) {
	const [localServices, setLocalServices] =
		useState<Settings["services"]>(services);
	const [testStatuses, setTestStatuses] = useState<
		Record<keyof Settings["services"], TestStatus>
	>({
		orchestratorUrl: "idle",
		notifyUrl: "idle",
		askUrl: "idle",
	});

	function handleUrlChange(key: keyof Settings["services"], value: string) {
		setLocalServices((prev) => ({ ...prev, [key]: value }));
		setTestStatuses((prev) => ({ ...prev, [key]: "idle" }));
	}

	async function handleTest(key: keyof Settings["services"]) {
		const url = localServices[key];
		setTestStatuses((prev) => ({ ...prev, [key]: "loading" }));
		try {
			const response = await fetch(`${url}/health`);
			setTestStatuses((prev) => ({
				...prev,
				[key]: response.ok ? "success" : "error",
			}));
		} catch {
			setTestStatuses((prev) => ({ ...prev, [key]: "error" }));
		}
	}

	function handleSave() {
		onSave(localServices);
	}

	return (
		<div className="space-y-4">
			{SERVICE_ROWS.map((row) => {
				const status = testStatuses[row.key];
				return (
					<div key={row.key} className="flex items-center gap-3">
						<label
							htmlFor={`service-url-${row.key}`}
							className="w-32 shrink-0 text-sm font-medium text-th-text-secondary"
						>
							{row.label}
							<span className="block text-xs font-normal text-th-text-muted">
								Port {row.port}
							</span>
						</label>
						<input
							id={`service-url-${row.key}`}
							type="text"
							value={localServices[row.key]}
							onChange={(e) => handleUrlChange(row.key, e.target.value)}
							className="min-w-0 flex-1 rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text placeholder:text-th-text-faint focus:border-th-focus-ring focus:outline-none focus:ring-1 focus:ring-th-focus-ring"
							placeholder={`http://localhost:${row.port}`}
						/>
						<button
							type="button"
							onClick={() => void handleTest(row.key)}
							disabled={status === "loading"}
							aria-label={`Test ${row.label} connection`}
							className="inline-flex items-center gap-1.5 rounded-md border border-th-border-strong bg-th-surface px-3 py-2 text-sm font-medium text-th-text-secondary transition-colors hover:bg-th-surface-hover disabled:cursor-not-allowed disabled:opacity-60"
						>
							{status === "loading" ? (
								<span
									className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-th-border border-t-th-text-secondary"
									aria-hidden="true"
								/>
							) : status === "success" ? (
								<span
									className="text-th-status-success-text"
									aria-hidden="true"
								>
									✓
								</span>
							) : status === "error" ? (
								<span
									className="text-th-status-error-text"
									aria-hidden="true"
								>
									✗
								</span>
							) : null}
							Test
						</button>
					</div>
				);
			})}

			<div className="pt-2">
				<button
					type="button"
					onClick={handleSave}
					className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text transition-colors hover:bg-th-accent-hover focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-2 focus:ring-offset-th-focus-ring-offset"
				>
					Save
				</button>
			</div>
		</div>
	);
}

export default ServiceConfig;
