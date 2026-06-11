/**
 * ThemePicker -- visual theme selector for the settings page.
 *
 * Displays a grid of theme cards with color swatches.
 * Grouped by family (dark / light) with a "System (auto)" option.
 */

import { Check, Monitor } from "lucide-react";
import { THEME_LIST, THEME_REGISTRY } from "@/styles/themes/index";

interface ThemePickerProps {
	value: string;
	onChange: (themeId: string) => void;
}

function SwatchRow({ themeId }: { themeId: string }) {
	const t = THEME_REGISTRY[themeId];
	if (!t) return null;
	const swatches = [t.page, t.surface, t.accent, t.text, t.textMuted];
	return (
		<div className="flex gap-1">
			{swatches.map((color, i) => (
				<span
					key={i}
					className="h-3 w-3 rounded-full border border-black/10"
					style={{ backgroundColor: color }}
				/>
			))}
		</div>
	);
}

export function ThemePicker({ value, onChange }: ThemePickerProps) {
	const darkThemes = THEME_LIST.filter((t) => t.family === "dark");
	const lightThemes = THEME_LIST.filter((t) => t.family === "light");

	const isSystem = value === "system";

	return (
		<div className="space-y-4">
			{/* System option */}
			<button
				type="button"
				onClick={() => onChange("system")}
				className={[
					"flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left text-sm transition-colors",
					isSystem
						? "border-th-accent bg-th-accent-subtle text-th-text"
						: "border-th-border bg-th-surface text-th-text-secondary hover:border-th-border-strong",
				].join(" ")}
			>
				<Monitor size={16} className="shrink-0" />
				<div className="flex-1">
					<span className="font-medium">System</span>
					<span className="ml-2 text-th-text-muted text-xs">
						follows OS preference
					</span>
				</div>
				{isSystem && <Check size={16} className="shrink-0 text-th-accent" />}
			</button>

			{/* Dark themes */}
			<div>
				<h4 className="mb-2 text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Dark
				</h4>
				<div className="grid grid-cols-2 gap-2">
					{darkThemes.map((t) => (
						<ThemeCard
							key={t.id}
							id={t.id}
							name={t.name}
							selected={value === t.id}
							onClick={() => onChange(t.id)}
						/>
					))}
				</div>
			</div>

			{/* Light themes */}
			<div>
				<h4 className="mb-2 text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Light
				</h4>
				<div className="grid grid-cols-2 gap-2">
					{lightThemes.map((t) => (
						<ThemeCard
							key={t.id}
							id={t.id}
							name={t.name}
							selected={value === t.id}
							onClick={() => onChange(t.id)}
						/>
					))}
				</div>
			</div>
		</div>
	);
}

interface ThemeCardProps {
	id: string;
	name: string;
	selected: boolean;
	onClick: () => void;
}

function ThemeCard({ id, name, selected, onClick }: ThemeCardProps) {
	return (
		<button
			type="button"
			onClick={onClick}
			className={[
				"flex items-center justify-between rounded-lg border px-3 py-2 text-left text-sm transition-colors",
				selected
					? "border-th-accent bg-th-accent-subtle text-th-text"
					: "border-th-border bg-th-surface text-th-text-secondary hover:border-th-border-strong",
			].join(" ")}
		>
			<div className="space-y-1">
				<span className="font-medium">{name}</span>
				<SwatchRow themeId={id} />
			</div>
			{selected && <Check size={14} className="shrink-0 text-th-accent" />}
		</button>
	);
}

export default ThemePicker;
