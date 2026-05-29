/**
 * SettingsPage — assembles all settings sections including service config,
 * UI preferences, about info, and data management actions.
 */

import { useRef, useState } from "react";
import { AboutSection } from "@/components/settings/AboutSection";
import { ServiceConfig } from "@/components/settings/ServiceConfig";
import { UIPreferences } from "@/components/settings/UIPreferences";
import { useSettings } from "@/hooks/useSettings";
import type { Settings } from "@/stores/settingsStore";
import { resetSettings } from "@/stores/settingsStore";

export function SettingsPage() {
	const { settings, updateServices, updateUI, reset } = useSettings();
	const [clearConfirmed, setClearConfirmed] = useState(false);
	const importInputRef = useRef<HTMLInputElement>(null);

	function handleClearAll() {
		if (!clearConfirmed) {
			setClearConfirmed(true);
			return;
		}
		resetSettings();
		reset();
		setClearConfirmed(false);
	}

	function handleExport() {
		const blob = new Blob([JSON.stringify(settings, null, 2)], {
			type: "application/json",
		});
		const url = URL.createObjectURL(blob);
		const a = document.createElement("a");
		a.href = url;
		a.download = "agentd-settings.json";
		a.click();
		URL.revokeObjectURL(url);
	}

	function handleImportClick() {
		importInputRef.current?.click();
	}

	function handleImportFile(e: React.ChangeEvent<HTMLInputElement>) {
		const file = e.target.files?.[0];
		if (!file) return;
		const reader = new FileReader();
		reader.onload = (event) => {
			try {
				const parsed = JSON.parse(
					event.target?.result as string,
				) as Partial<Settings>;
				if (parsed.services) updateServices(parsed.services);
				if (parsed.ui) updateUI(parsed.ui);
			} catch {
				// silently ignore malformed JSON
			}
		};
		reader.readAsText(file);
		// Reset input so the same file can be re-imported
		e.target.value = "";
	}

	return (
		<div className="space-y-8">
			<div>
				<h1 className="text-2xl font-semibold text-th-text">Settings</h1>
				<p className="mt-1 text-sm text-th-text-muted">
					Manage service connections, UI preferences, and application data.
				</p>
			</div>

			{/* Service Configuration */}
			<section className="rounded-lg border border-th-border bg-th-surface p-6 shadow-sm">
				<h2 className="mb-4 text-lg font-semibold text-th-text">
					Service Configuration
				</h2>
				<ServiceConfig services={settings.services} onSave={updateServices} />
			</section>

			{/* UI Preferences */}
			<section className="rounded-lg border border-th-border bg-th-surface p-6 shadow-sm">
				<h2 className="mb-4 text-lg font-semibold text-th-text">
					UI Preferences
				</h2>
				<UIPreferences ui={settings.ui} onSave={updateUI} />
			</section>

			{/* About */}
			<section className="rounded-lg border border-th-border bg-th-surface p-6 shadow-sm">
				<h2 className="mb-4 text-lg font-semibold text-th-text">About</h2>
				<AboutSection />
			</section>

			{/* Data Management */}
			<section className="rounded-lg border border-th-border bg-th-surface p-6 shadow-sm">
				<h2 className="mb-4 text-lg font-semibold text-th-text">
					Data Management
				</h2>
				<div className="flex flex-wrap gap-3">
					<button
						type="button"
						onClick={handleClearAll}
						className={`rounded-md px-4 py-2 text-sm font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-th-focus-ring-offset ${
							clearConfirmed
								? "bg-th-status-error-dot text-th-accent-text hover:opacity-90 focus:ring-th-status-error-dot"
								: "border border-th-status-error-border text-th-status-error-text hover:bg-th-status-error-bg focus:ring-th-status-error-dot"
						}`}
					>
						{clearConfirmed
							? "Confirm Clear All Settings"
							: "Clear All Settings"}
					</button>

					<button
						type="button"
						onClick={handleExport}
						className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary transition-colors hover:bg-th-surface-hover focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-2 focus:ring-offset-th-focus-ring-offset"
					>
						Export Settings
					</button>

					<button
						type="button"
						onClick={handleImportClick}
						className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary transition-colors hover:bg-th-surface-hover focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-2 focus:ring-offset-th-focus-ring-offset"
					>
						Import Settings
					</button>
					<input
						ref={importInputRef}
						type="file"
						accept=".json"
						className="hidden"
						onChange={handleImportFile}
						aria-label="Import settings file"
					/>
				</div>
			</section>
		</div>
	);
}

export default SettingsPage;
