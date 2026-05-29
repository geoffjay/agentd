/**
 * themeStore -- pure DOM/localStorage theme application logic.
 *
 * Handles:
 * - Resolving "system" to a concrete theme ID via OS preference
 * - Applying a named theme by setting CSS custom properties on <html>
 * - Toggling data-theme attribute and color-scheme
 * - Reading/writing the theme preference to localStorage (via settingsStore key)
 *
 * These are plain functions (no React) so they can be called from:
 * - The anti-FOUC inline script in index.html (before React mounts)
 * - React hooks / event handlers
 */

import type { ThemeTokens } from "@/styles/theme-tokens";
import { TOKEN_CSS_MAP, TOKEN_KEYS } from "@/styles/theme-tokens";
import {
	DEFAULT_THEME_ID,
	SYSTEM_DARK_ID,
	SYSTEM_LIGHT_ID,
	THEME_REGISTRY,
} from "@/styles/themes/index";

/**
 * Resolve a theme preference to a concrete theme ID.
 * "system" checks prefers-color-scheme and maps to the default dark/light theme.
 * Unknown IDs fall back to DEFAULT_THEME_ID.
 */
export function resolveThemeId(preference: string): string {
	if (preference === "system") {
		try {
			return window.matchMedia("(prefers-color-scheme: dark)").matches
				? SYSTEM_DARK_ID
				: SYSTEM_LIGHT_ID;
		} catch {
			return DEFAULT_THEME_ID;
		}
	}
	if (preference in THEME_REGISTRY) return preference;
	return DEFAULT_THEME_ID;
}

/**
 * Get the ThemeTokens for a given theme ID.
 * Falls back to DEFAULT_THEME_ID if unknown.
 */
export function getTheme(id: string): ThemeTokens {
	return THEME_REGISTRY[id] ?? THEME_REGISTRY[DEFAULT_THEME_ID];
}

/**
 * Apply a theme to the DOM.
 * Sets CSS custom properties, data-theme, and color-scheme.
 */
export function applyTheme(preference: string): void {
	const id = resolveThemeId(preference);
	const theme = getTheme(id);
	const root = document.documentElement;

	// Set data-theme attribute
	root.setAttribute("data-theme", id);

	// Set color-scheme for native element theming (scrollbars, form controls)
	root.style.colorScheme = theme.family;

	// Apply all token CSS custom properties
	for (const key of TOKEN_KEYS) {
		const cssVar = TOKEN_CSS_MAP[key];
		const value = theme[key as keyof ThemeTokens] as string;
		if (cssVar && value) {
			root.style.setProperty(cssVar, value);
		}
	}
}

/**
 * Read the persisted theme preference from localStorage.
 * Migrates legacy "light"/"dark" values to named theme IDs.
 * Returns "system" if nothing is stored or parsing fails.
 */
export function readPersistedTheme(): string {
	try {
		const raw = localStorage.getItem("agentd:settings");
		if (!raw) return "system";
		const parsed = JSON.parse(raw) as { ui?: { theme?: string } };
		const pref = parsed?.ui?.theme;
		if (!pref) return "system";

		// Migrate legacy values
		if (pref === "light") return "agentd-light";
		if (pref === "dark") return "agentd-dark";

		return pref;
	} catch {
		return "system";
	}
}
