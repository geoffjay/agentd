/**
 * useNivoTheme — returns the appropriate Nivo chart theme for the active theme.
 *
 * Usage:
 *   const nivoTheme = useNivoTheme()
 *   <ResponsiveLine theme={nivoTheme} ... />
 */

import { useTheme } from './useTheme'
import { lightNivoTheme, darkNivoTheme } from '@/styles/themes'
import type { PartialTheme as NivoTheme } from '@nivo/theming'

export function useNivoTheme(): NivoTheme {
  const { resolvedTheme } = useTheme()
  return resolvedTheme === 'dark' ? darkNivoTheme : lightNivoTheme
}
