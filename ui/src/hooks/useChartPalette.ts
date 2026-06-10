/**
 * useChartPalette -- theme-aware chart colors derived from ThemeTokens.
 *
 * Charts should use muted (alpha-blended) fills via `withAlpha` so they sit
 * quietly on the page; full-strength token colors are reserved for legend
 * dots, small text accents, and thin (1px) arc/series borders.
 *
 * Falls back to the default theme tokens when rendered outside a
 * <ThemeProvider> (e.g. isolated component tests) instead of throwing.
 */

import { useContext, useMemo } from "react";
import { ThemeContext } from "@/hooks/useTheme";
import { getTheme } from "@/stores/themeStore";
import type { ThemeTokens } from "@/styles/theme-tokens";
import { DEFAULT_THEME_ID } from "@/styles/themes/index";

// ---------------------------------------------------------------------------
// withAlpha
// ---------------------------------------------------------------------------

/**
 * Apply an alpha channel to a CSS color, returning an `rgba()` string.
 *
 * Supported inputs: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
 * `rgb(r, g, b)`, `rgba(r, g, b, a)` (comma or space separated).
 * Unparseable values are returned unchanged.
 */
export function withAlpha(color: string, alpha: number): string {
	const a = Math.min(1, Math.max(0, alpha));
	const trimmed = color.trim();

	// Hex forms
	const hexMatch = trimmed.match(/^#([0-9a-f]{3,8})$/i);
	if (hexMatch) {
		const hex = hexMatch[1];
		let r: number;
		let g: number;
		let b: number;
		if (hex.length === 3 || hex.length === 4) {
			r = Number.parseInt(hex[0] + hex[0], 16);
			g = Number.parseInt(hex[1] + hex[1], 16);
			b = Number.parseInt(hex[2] + hex[2], 16);
		} else if (hex.length === 6 || hex.length === 8) {
			r = Number.parseInt(hex.slice(0, 2), 16);
			g = Number.parseInt(hex.slice(2, 4), 16);
			b = Number.parseInt(hex.slice(4, 6), 16);
		} else {
			return color;
		}
		return `rgba(${r}, ${g}, ${b}, ${a})`;
	}

	// rgb(...) / rgba(...) forms -- comma, space, or slash separated
	const rgbMatch = trimmed.match(
		/^rgba?\(\s*([\d.]+%?)\s*[, ]\s*([\d.]+%?)\s*[, ]\s*([\d.]+%?)\s*(?:[,/]\s*[\d.%]+\s*)?\)$/i,
	);
	if (rgbMatch) {
		return `rgba(${rgbMatch[1]}, ${rgbMatch[2]}, ${rgbMatch[3]}, ${a})`;
	}

	return color;
}

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

export interface ChartPalette {
	/** Semantic colors (full strength -- mute with withAlpha for fills) */
	success: string;
	warning: string;
	error: string;
	info: string;
	neutral: string;
	accent: string;
	/** Surface color, used for subtle arc/segment borders */
	surface: string;
	/** Categorical series colors derived from theme tokens */
	series: string[];
	/** Helper re-exported for convenience in chart components */
	withAlpha: typeof withAlpha;
}

/** Build a chart palette from a full token set. */
export function buildChartPalette(t: ThemeTokens): ChartPalette {
	return {
		success: t.statusSuccessDot,
		warning: t.statusWarningDot,
		error: t.statusErrorDot,
		info: t.statusInfoDot,
		neutral: t.textMuted,
		accent: t.accent,
		surface: t.surface,
		series: [
			t.accent,
			t.statusInfoDot,
			t.statusSuccessDot,
			t.statusWarningDot,
			t.accentSecondary,
			t.statusErrorDot,
		],
		withAlpha,
	};
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

/**
 * Read the current ThemeTokens, falling back to the default theme when no
 * ThemeProvider is mounted (keeps chart components safe in isolation).
 */
export function useThemeTokens(): ThemeTokens {
	const ctx = useContext(ThemeContext);
	return ctx?.theme ?? getTheme(DEFAULT_THEME_ID);
}

/** Theme-aware chart palette for the active theme. */
export function useChartPalette(): ChartPalette {
	const tokens = useThemeTokens();
	return useMemo(() => buildChartPalette(tokens), [tokens]);
}
