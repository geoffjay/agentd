/**
 * themes.ts — theme color definitions.
 *
 * Exports:
 * - `lightColors` / `darkColors`: full color maps for each theme
 * - `lightNivoTheme` / `darkNivoTheme`: Nivo chart theme objects (PartialTheme)
 *
 * These can be used with the `useNivoTheme()` hook which returns the
 * appropriate Nivo theme based on the active theme.
 */

import type { PartialTheme as NivoTheme } from '@nivo/theming'

// ---------------------------------------------------------------------------
// Color palettes
// ---------------------------------------------------------------------------

export const lightColors = {
  // Backgrounds
  bgPrimary: '#f8fafc',      // secondary-50 — page background
  bgSecondary: '#f1f5f9',    // secondary-100 — card background
  bgTertiary: '#e2e8f0',     // secondary-200 — subtle surface

  // Text
  textPrimary: '#0f172a',    // secondary-900
  textSecondary: '#334155',  // secondary-700
  textMuted: '#64748b',      // secondary-500

  // Borders
  borderDefault: '#e2e8f0',  // secondary-200
  borderStrong: '#cbd5e1',   // secondary-300

  // Accent
  accentPrimary: '#3b82f6',  // primary-500
  accentSecondary: '#2563eb', // primary-600

  // Status
  success: '#22c55e',
  warning: '#f59e0b',
  error: '#ef4444',
  info: '#06b6d4',
} as const

export const darkColors = {
  // Backgrounds
  bgPrimary: '#020617',      // secondary-950 — page background
  bgSecondary: '#0f172a',    // secondary-900 — card background
  bgTertiary: '#1e293b',     // secondary-800 — subtle surface

  // Text
  textPrimary: '#f8fafc',    // secondary-50
  textSecondary: '#cbd5e1',  // secondary-300
  textMuted: '#94a3b8',      // secondary-400

  // Borders
  borderDefault: '#1e293b',  // secondary-800
  borderStrong: '#334155',   // secondary-700

  // Accent
  accentPrimary: '#60a5fa',  // primary-400
  accentSecondary: '#93c5fd', // primary-300

  // Status
  success: '#22c55e',
  warning: '#f59e0b',
  error: '#ef4444',
  info: '#06b6d4',
} as const

// ---------------------------------------------------------------------------
// Nivo chart themes
// ---------------------------------------------------------------------------

export const lightNivoTheme: NivoTheme = {
  background: lightColors.bgSecondary,
  axis: {
    domain: {
      line: { stroke: lightColors.borderStrong, strokeWidth: 1 },
    },
    ticks: {
      line: { stroke: lightColors.borderDefault, strokeWidth: 1 },
      text: { fill: lightColors.textMuted, fontSize: 11 },
    },
    legend: {
      text: { fill: lightColors.textSecondary, fontSize: 12, fontWeight: 500 },
    },
  },
  grid: {
    line: { stroke: lightColors.borderDefault, strokeWidth: 1 },
  },
  legends: {
    text: { fill: lightColors.textSecondary, fontSize: 12 },
  },
  tooltip: {
    container: {
      background: '#ffffff',
      color: lightColors.textPrimary,
      fontSize: 12,
      borderRadius: 6,
      boxShadow: '0 4px 6px -1px rgb(0 0 0 / 0.1)',
      border: `1px solid ${lightColors.borderDefault}`,
    },
  },
  labels: {
    text: { fill: lightColors.textSecondary, fontSize: 11 },
  },
}

export const darkNivoTheme: NivoTheme = {
  background: darkColors.bgSecondary,
  axis: {
    domain: {
      line: { stroke: darkColors.borderStrong, strokeWidth: 1 },
    },
    ticks: {
      line: { stroke: darkColors.borderDefault, strokeWidth: 1 },
      text: { fill: darkColors.textMuted, fontSize: 11 },
    },
    legend: {
      text: { fill: darkColors.textSecondary, fontSize: 12, fontWeight: 500 },
    },
  },
  grid: {
    line: { stroke: darkColors.borderDefault, strokeWidth: 1 },
  },
  legends: {
    text: { fill: darkColors.textSecondary, fontSize: 12 },
  },
  tooltip: {
    container: {
      background: darkColors.bgTertiary,
      color: darkColors.textPrimary,
      fontSize: 12,
      borderRadius: 6,
      boxShadow: '0 4px 6px -1px rgb(0 0 0 / 0.25)',
      border: `1px solid ${darkColors.borderStrong}`,
    },
  },
  labels: {
    text: { fill: darkColors.textSecondary, fontSize: 11 },
  },
}
