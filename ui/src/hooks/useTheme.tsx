/**
 * useTheme -- React hook + context for named theme management.
 *
 * Provides:
 * - `themeId`: the stored preference (could be "system" or a concrete ID)
 * - `resolvedThemeId`: the concrete theme ID after resolving "system"
 * - `theme`: the full ThemeTokens object for the resolved theme
 * - `setTheme`: update theme preference (persists + applies to DOM)
 *
 * Usage: wrap your app with <ThemeProvider> and consume via useTheme().
 */

import type { ReactNode } from "react";
import {
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import { loadSettings, saveSettings } from "@/stores/settingsStore";
import type { ThemeTokens } from "@/styles/theme-tokens";
import { applyTheme, getTheme, resolveThemeId } from "@/stores/themeStore";

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

export interface ThemeContextValue {
	/** Stored theme preference (may be "system") */
	themeId: string;
	/** Resolved concrete theme ID -- never "system" */
	resolvedThemeId: string;
	/** Full token set for the resolved theme */
	theme: ThemeTokens;
	/** Update the theme preference */
	setTheme: (id: string) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

interface ThemeProviderProps {
	children: ReactNode;
}

export function ThemeProvider({ children }: ThemeProviderProps) {
	const [themeId, setThemeIdState] = useState<string>(() => {
		const settings = loadSettings();
		return settings.ui.theme;
	});

	const resolved = resolveThemeId(themeId);
	const theme = useMemo(() => getTheme(resolved), [resolved]);

	// Apply theme to DOM on every theme change
	useEffect(() => {
		applyTheme(themeId);
	}, [themeId]);

	// Watch for OS preference changes when using "system" mode
	useEffect(() => {
		if (themeId !== "system") return;

		const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

		function handleChange() {
			applyTheme("system");
			// Force re-render so resolvedThemeId updates
			setThemeIdState("system");
		}

		mediaQuery.addEventListener("change", handleChange);
		return () => mediaQuery.removeEventListener("change", handleChange);
	}, [themeId]);

	const setTheme = useCallback((id: string) => {
		const current = loadSettings();
		saveSettings({ ...current, ui: { ...current.ui, theme: id } });
		applyTheme(id);
		setThemeIdState(id);
	}, []);

	const value = useMemo<ThemeContextValue>(
		() => ({ themeId, resolvedThemeId: resolved, theme, setTheme }),
		[themeId, resolved, theme, setTheme],
	);

	return (
		<ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
	);
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/** Consume the theme context -- must be used within <ThemeProvider>. */
export function useTheme(): ThemeContextValue {
	const ctx = useContext(ThemeContext);
	if (!ctx) {
		throw new Error("useTheme must be used within a ThemeProvider");
	}
	return ctx;
}
