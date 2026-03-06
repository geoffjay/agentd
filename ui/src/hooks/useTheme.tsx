/**
 * useTheme — React hook + context for theme management.
 *
 * Provides:
 * - `theme`: the stored preference ('light' | 'dark' | 'system')
 * - `resolvedTheme`: the concrete theme after resolving 'system' ('light' | 'dark')
 * - `setTheme`: update theme preference (persists + applies to DOM)
 *
 * Usage: wrap your app with <ThemeProvider> and consume via useTheme().
 */

import { createContext, useCallback, useContext, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { loadSettings, saveSettings } from '@/stores/settingsStore'
import { applyTheme, resolveTheme } from '@/stores/themeStore'
import type { ThemeMode } from '@/stores/themeStore'

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

export interface ThemeContextValue {
  /** Stored theme preference */
  theme: ThemeMode
  /** Resolved (concrete) theme — never 'system' */
  resolvedTheme: 'light' | 'dark'
  /** Update the theme preference */
  setTheme: (mode: ThemeMode) => void
}

export const ThemeContext = createContext<ThemeContextValue | null>(null)

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

interface ThemeProviderProps {
  children: ReactNode
}

export function ThemeProvider({ children }: ThemeProviderProps) {
  const [theme, setThemeState] = useState<ThemeMode>(() => {
    return loadSettings().ui.theme
  })

  const resolvedTheme = resolveTheme(theme)

  // Apply theme to DOM on every theme change
  useEffect(() => {
    applyTheme(theme)
  }, [theme])

  // Watch for OS preference changes when using 'system' mode
  useEffect(() => {
    if (theme !== 'system') return

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')

    function handleChange() {
      // Re-apply — resolveTheme() will pick up the new OS value
      applyTheme('system')
      // Force a re-render so resolvedTheme updates
      setThemeState('system')
    }

    mediaQuery.addEventListener('change', handleChange)
    return () => mediaQuery.removeEventListener('change', handleChange)
  }, [theme])

  const setTheme = useCallback((mode: ThemeMode) => {
    // Persist to settings store
    const current = loadSettings()
    saveSettings({ ...current, ui: { ...current.ui, theme: mode } })
    // Apply immediately
    applyTheme(mode)
    setThemeState(mode)
  }, [])

  return (
    <ThemeContext.Provider value={{ theme, resolvedTheme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/** Consume the theme context — must be used within <ThemeProvider> */
export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext)
  if (!ctx) {
    throw new Error('useTheme must be used within a ThemeProvider')
  }
  return ctx
}
