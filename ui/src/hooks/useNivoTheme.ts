/**
 * useNivoTheme -- derives a Nivo chart theme from the active ThemeTokens.
 *
 * Usage:
 *   const nivoTheme = useNivoTheme()
 *   <ResponsiveLine theme={nivoTheme} ... />
 */

import type { PartialTheme as NivoTheme } from "@nivo/theming";
import { useMemo } from "react";
import type { ThemeTokens } from "@/styles/theme-tokens";
import { useTheme } from "./useTheme";

/** Build a Nivo chart theme from ThemeTokens. */
export function buildNivoTheme(t: ThemeTokens): NivoTheme {
	return {
		background: t.surface,
		axis: {
			domain: {
				line: { stroke: t.borderStrong, strokeWidth: 1 },
			},
			ticks: {
				line: { stroke: t.border, strokeWidth: 1 },
				text: { fill: t.textMuted, fontSize: 11 },
			},
			legend: {
				text: { fill: t.textSecondary, fontSize: 12, fontWeight: 500 },
			},
		},
		grid: {
			line: { stroke: t.border, strokeWidth: 1 },
		},
		legends: {
			text: { fill: t.textSecondary, fontSize: 12 },
		},
		tooltip: {
			container: {
				background: t.surfaceRaised,
				color: t.text,
				fontSize: 12,
				borderRadius: 6,
				boxShadow: "0 4px 6px -1px rgb(0 0 0 / 0.1)",
				border: `1px solid ${t.border}`,
			},
		},
		labels: {
			text: { fill: t.textSecondary, fontSize: 11 },
		},
	};
}

export function useNivoTheme(): NivoTheme {
	const { theme } = useTheme();
	return useMemo(() => buildNivoTheme(theme), [theme]);
}
