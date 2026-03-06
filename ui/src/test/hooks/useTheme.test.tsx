/**
 * Tests for useTheme hook and ThemeProvider.
 *
 * These are unit tests that verify:
 * - ThemeProvider reads initial theme from settingsStore
 * - setTheme updates DOM class and persists to localStorage
 * - System mode responds to prefers-color-scheme changes
 * - resolvedTheme correctly reflects the active theme
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import type { ReactNode } from 'react'
import { ThemeProvider, useTheme } from '@/hooks/useTheme'

// ---------------------------------------------------------------------------
// Test wrapper
// ---------------------------------------------------------------------------

function wrapper({ children }: { children: ReactNode }) {
  return <ThemeProvider>{children}</ThemeProvider>
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function setStoredTheme(theme: 'light' | 'dark' | 'system') {
  localStorage.setItem(
    'agentd:settings',
    JSON.stringify({ version: 1, ui: { theme } }),
  )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useTheme', () => {
  beforeEach(() => {
    localStorage.clear()
    document.documentElement.classList.remove('dark')
  })

  afterEach(() => {
    vi.restoreAllMocks()
    document.documentElement.classList.remove('dark')
  })

  it('defaults to system theme when no preference is stored', () => {
    const { result } = renderHook(() => useTheme(), { wrapper })
    expect(result.current.theme).toBe('system')
  })

  it('reads stored light preference', () => {
    setStoredTheme('light')
    const { result } = renderHook(() => useTheme(), { wrapper })
    expect(result.current.theme).toBe('light')
  })

  it('reads stored dark preference', () => {
    setStoredTheme('dark')
    const { result } = renderHook(() => useTheme(), { wrapper })
    expect(result.current.theme).toBe('dark')
  })

  it('adds dark class to documentElement when theme is dark', () => {
    setStoredTheme('dark')
    renderHook(() => useTheme(), { wrapper })
    expect(document.documentElement.classList.contains('dark')).toBe(true)
  })

  it('removes dark class when theme is light', () => {
    document.documentElement.classList.add('dark')
    setStoredTheme('light')
    renderHook(() => useTheme(), { wrapper })
    expect(document.documentElement.classList.contains('dark')).toBe(false)
  })

  it('setTheme updates the theme and applies to DOM', () => {
    const { result } = renderHook(() => useTheme(), { wrapper })

    act(() => {
      result.current.setTheme('dark')
    })

    expect(result.current.theme).toBe('dark')
    expect(document.documentElement.classList.contains('dark')).toBe(true)
  })

  it('setTheme light removes dark class', () => {
    document.documentElement.classList.add('dark')
    const { result } = renderHook(() => useTheme(), { wrapper })

    act(() => {
      result.current.setTheme('light')
    })

    expect(result.current.theme).toBe('light')
    expect(document.documentElement.classList.contains('dark')).toBe(false)
  })

  it('setTheme persists to localStorage', () => {
    const { result } = renderHook(() => useTheme(), { wrapper })

    act(() => {
      result.current.setTheme('dark')
    })

    const stored = JSON.parse(localStorage.getItem('agentd:settings') ?? '{}')
    expect(stored.ui.theme).toBe('dark')
  })

  it('resolvedTheme is light when theme is light', () => {
    setStoredTheme('light')
    const { result } = renderHook(() => useTheme(), { wrapper })
    expect(result.current.resolvedTheme).toBe('light')
  })

  it('resolvedTheme is dark when theme is dark', () => {
    setStoredTheme('dark')
    const { result } = renderHook(() => useTheme(), { wrapper })
    expect(result.current.resolvedTheme).toBe('dark')
  })

  it('resolvedTheme follows system preference in system mode', () => {
    // Mock matchMedia to report dark preference
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: query === '(prefers-color-scheme: dark)',
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    })

    const { result } = renderHook(() => useTheme(), { wrapper })
    // system mode → dark OS preference → resolvedTheme = dark
    expect(result.current.resolvedTheme).toBe('dark')
  })

  it('throws when used outside ThemeProvider', () => {
    // Suppress the error boundary console output
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(() => renderHook(() => useTheme())).toThrow(
      'useTheme must be used within a ThemeProvider',
    )
    spy.mockRestore()
  })
})
