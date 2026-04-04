/**
 * UIPreferences — UI preferences form for theme, sidebar, refresh interval,
 * notifications, and log view lines.
 *
 * Theme changes apply immediately to the DOM via ThemeContext so users
 * can preview the effect before saving.
 */

import { useState } from "react";
import { useTheme } from "@/hooks/useTheme";
import type { Settings } from "@/stores/settingsStore";
import { ThemePicker } from "./ThemePicker";

interface UIPreferencesProps {
	ui: Settings["ui"];
	onSave: (ui: Settings["ui"]) => void;
}

export function UIPreferences({ ui, onSave }: UIPreferencesProps) {
	const [localUI, setLocalUI] = useState<Settings["ui"]>(ui);
	const { setTheme } = useTheme();

	function handleSave() {
		onSave(localUI);
	}

	return (
		<div className="space-y-5">
			{/* Theme */}
			<div>
				<label className="mb-2 block text-sm font-medium text-th-text-secondary">
					Theme
				</label>
				<ThemePicker
					value={localUI.theme}
					onChange={(themeId) => {
						setLocalUI((prev) => ({ ...prev, theme: themeId }));
						setTheme(themeId);
					}}
				/>
			</div>

			{/* Sidebar default open */}
			<div className="flex items-center justify-between">
				<label
					htmlFor="ui-sidebar-open"
					className="text-sm font-medium text-th-text-secondary"
				>
					Sidebar
				</label>
				<label className="flex cursor-pointer items-center gap-2 text-sm text-th-text-muted">
					<input
						id="ui-sidebar-open"
						type="checkbox"
						checked={localUI.sidebarDefaultOpen}
						onChange={(e) =>
							setLocalUI((prev) => ({
								...prev,
								sidebarDefaultOpen: e.target.checked,
							}))
						}
						className="h-4 w-4 rounded border-th-border-input text-th-accent focus:ring-th-focus-ring"
					/>
					Open by default
				</label>
			</div>

			{/* Refresh interval */}
			<div className="flex items-center justify-between">
				<label
					htmlFor="ui-refresh-interval"
					className="text-sm font-medium text-th-text-secondary"
				>
					Refresh interval
				</label>
				<select
					id="ui-refresh-interval"
					value={localUI.refreshInterval}
					onChange={(e) =>
						setLocalUI((prev) => ({
							...prev,
							refreshInterval: Number(
								e.target.value,
							) as Settings["ui"]["refreshInterval"],
						}))
					}
					className="rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text focus:border-th-border-focus focus:outline-none focus:ring-1 focus:ring-th-focus-ring"
				>
					<option value={30}>30s</option>
					<option value={60}>60s</option>
					<option value={120}>2m</option>
					<option value={300}>5m</option>
				</select>
			</div>

			{/* Notifications */}
			<div className="flex items-center justify-between">
				<label
					htmlFor="ui-notifications"
					className="text-sm font-medium text-th-text-secondary"
				>
					Notifications
				</label>
				<label className="flex cursor-pointer items-center gap-2 text-sm text-th-text-muted">
					<input
						id="ui-notifications"
						type="checkbox"
						checked={localUI.notificationsEnabled}
						onChange={(e) =>
							setLocalUI((prev) => ({
								...prev,
								notificationsEnabled: e.target.checked,
							}))
						}
						className="h-4 w-4 rounded border-th-border-input text-th-accent focus:ring-th-focus-ring"
					/>
					Enable desktop notifications
				</label>
			</div>

			{/* Log view lines */}
			<div className="flex items-center justify-between">
				<label
					htmlFor="ui-log-lines"
					className="text-sm font-medium text-th-text-secondary"
				>
					Log view lines
				</label>
				<select
					id="ui-log-lines"
					value={localUI.logViewLines}
					onChange={(e) =>
						setLocalUI((prev) => ({
							...prev,
							logViewLines: Number(
								e.target.value,
							) as Settings["ui"]["logViewLines"],
						}))
					}
					className="rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text focus:border-th-border-focus focus:outline-none focus:ring-1 focus:ring-th-focus-ring"
				>
					<option value={100}>100</option>
					<option value={250}>250</option>
					<option value={500}>500</option>
					<option value={1000}>1000</option>
				</select>
			</div>

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

export default UIPreferences;
