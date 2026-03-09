/**
 * themeStore — pure DOM/localStorage theme application logic.
 *
 * Handles:
 * - Resolving 'system' mode against OS preference
 * - Toggling the `dark` class on <html>
 * - Reading/writing the theme preference to localStorage (via settingsStore key)
 *
 * These are plain functions (no React) so they can be called from:
 * - The anti-FOUC inline script in index.html (before React mounts)
 * - React hooks / event handlers
 */

export type ThemeMode = 'light' | 'dark' | 'system'

/**
 * Resolve the effective (concrete) theme from a mode value.
 * 'system' → checks prefers-color-scheme media query.
 */
export function resolveTheme(mode: ThemeMode): 'light' | 'dark' {
  if (mode === 'system') {
    try {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
    } catch {
      return 'light'
    }
  }
  return mode
}

/**
 * Apply the given theme mode to the DOM by toggling the `dark` class
 * on `document.documentElement`.
 */
export function applyTheme(mode: ThemeMode): void {
  const resolved = resolveTheme(mode)
  document.documentElement.classList.toggle('dark', resolved === 'dark')
}

/**
 * Read the persisted theme preference from localStorage.
 * Returns 'system' if nothing is stored or parsing fails.
 */
export function readPersistedTheme(): ThemeMode {
  try {
    const raw = localStorage.getItem('agentd:settings')
    if (!raw) return 'system'
    const parsed = JSON.parse(raw) as { ui?: { theme?: ThemeMode } }
    const mode = parsed?.ui?.theme
    if (mode === 'light' || mode === 'dark' || mode === 'system') return mode
    return 'system'
  } catch {
    return 'system'
  }
}
