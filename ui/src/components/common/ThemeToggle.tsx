/**
 * ThemeToggle — quick theme toggle for the header bar.
 *
 * Cycles through: system → light → dark → system
 * Displays an icon representing the *current* preference:
 *   - system → Monitor icon
 *   - light  → Sun icon
 *   - dark   → Moon icon
 */

import { Monitor, Moon, Sun } from 'lucide-react'
import { useTheme } from '@/hooks/useTheme'
import type { ThemeMode } from '@/stores/themeStore'

const NEXT_THEME: Record<ThemeMode, ThemeMode> = {
  system: 'light',
  light: 'dark',
  dark: 'system',
}

const LABELS: Record<ThemeMode, string> = {
  system: 'Theme: System (switch to Light)',
  light: 'Theme: Light (switch to Dark)',
  dark: 'Theme: Dark (switch to System)',
}

function ThemeIcon({ mode }: { mode: ThemeMode }) {
  const size = 18
  if (mode === 'dark') return <Moon size={size} aria-hidden="true" />
  if (mode === 'light') return <Sun size={size} aria-hidden="true" />
  return <Monitor size={size} aria-hidden="true" />
}

export interface ThemeToggleProps {
  /** Additional class names to merge onto the button */
  className?: string
}

export function ThemeToggle({ className = '' }: ThemeToggleProps) {
  const { theme, setTheme } = useTheme()

  function handleClick() {
    setTheme(NEXT_THEME[theme])
  }

  return (
    <button
      type="button"
      aria-label={LABELS[theme]}
      onClick={handleClick}
      className={[
        'rounded-md p-2 text-gray-400 transition-colors',
        'hover:bg-gray-700 hover:text-white',
        'dark:text-gray-400 dark:hover:bg-gray-700 dark:hover:text-white',
        'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
        'dark:focus:ring-offset-gray-900',
        className,
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <ThemeIcon mode={theme} />
    </button>
  )
}

export default ThemeToggle
