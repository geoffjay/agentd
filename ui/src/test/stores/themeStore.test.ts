/**
 * Tests for themeStore pure functions.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { resolveTheme, applyTheme, readPersistedTheme } from '@/stores/themeStore'

function mockMatchMedia(prefersDark: boolean) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: prefersDark && query === '(prefers-color-scheme: dark)',
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  })
}

describe('resolveTheme', () => {
  afterEach(() => vi.restoreAllMocks())

  it('returns light for light mode', () => {
    expect(resolveTheme('light')).toBe('light')
  })

  it('returns dark for dark mode', () => {
    expect(resolveTheme('dark')).toBe('dark')
  })

  it('returns dark when system prefers dark', () => {
    mockMatchMedia(true)
    expect(resolveTheme('system')).toBe('dark')
  })

  it('returns light when system prefers light', () => {
    mockMatchMedia(false)
    expect(resolveTheme('system')).toBe('light')
  })
})

describe('applyTheme', () => {
  beforeEach(() => {
    document.documentElement.classList.remove('dark')
  })

  afterEach(() => {
    document.documentElement.classList.remove('dark')
    vi.restoreAllMocks()
  })

  it('adds dark class for dark mode', () => {
    applyTheme('dark')
    expect(document.documentElement.classList.contains('dark')).toBe(true)
  })

  it('removes dark class for light mode', () => {
    document.documentElement.classList.add('dark')
    applyTheme('light')
    expect(document.documentElement.classList.contains('dark')).toBe(false)
  })

  it('adds dark class when system prefers dark', () => {
    mockMatchMedia(true)
    applyTheme('system')
    expect(document.documentElement.classList.contains('dark')).toBe(true)
  })

  it('removes dark class when system prefers light', () => {
    document.documentElement.classList.add('dark')
    mockMatchMedia(false)
    applyTheme('system')
    expect(document.documentElement.classList.contains('dark')).toBe(false)
  })
})

describe('readPersistedTheme', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('returns system when nothing is stored', () => {
    expect(readPersistedTheme()).toBe('system')
  })

  it('returns light from stored settings', () => {
    localStorage.setItem(
      'agentd:settings',
      JSON.stringify({ ui: { theme: 'light' } }),
    )
    expect(readPersistedTheme()).toBe('light')
  })

  it('returns dark from stored settings', () => {
    localStorage.setItem(
      'agentd:settings',
      JSON.stringify({ ui: { theme: 'dark' } }),
    )
    expect(readPersistedTheme()).toBe('dark')
  })

  it('returns system for invalid stored value', () => {
    localStorage.setItem(
      'agentd:settings',
      JSON.stringify({ ui: { theme: 'rainbow' } }),
    )
    expect(readPersistedTheme()).toBe('system')
  })

  it('returns system on malformed JSON', () => {
    localStorage.setItem('agentd:settings', 'not json {{{')
    expect(readPersistedTheme()).toBe('system')
  })
})
