/**
 * useNivoTheme — returns the appropriate Nivo chart theme for the active theme.
 *
 * Usage:
 *   const nivoTheme = useNivoTheme()
 *   <ResponsiveLine theme={nivoTheme} ... />
 */

import type { PartialTheme as NivoTheme } from "@nivo/theming";
import { darkNivoTheme, lightNivoTheme } from "@/styles/themes";
import { useTheme } from "./useTheme";

export function useNivoTheme(): NivoTheme {
	const { resolvedTheme } = useTheme();
	return resolvedTheme === "dark" ? darkNivoTheme : lightNivoTheme;
}
