/**
 * Tests for useHumanIdentity hook.
 */

import { describe, it, expect, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useHumanIdentity, IDENTITY_KEY } from '@/hooks/useHumanIdentity'

describe('useHumanIdentity', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('returns null identity when localStorage is empty', () => {
    const { result } = renderHook(() => useHumanIdentity())
    expect(result.current.identity).toBeNull()
    expect(result.current.isSetup).toBe(false)
  })

  it('reads identity from localStorage on mount', () => {
    localStorage.setItem(
      IDENTITY_KEY,
      JSON.stringify({ identifier: 'human-alice', displayName: 'Alice' }),
    )
    const { result } = renderHook(() => useHumanIdentity())
    expect(result.current.identity).toEqual({ identifier: 'human-alice', displayName: 'Alice' })
    expect(result.current.isSetup).toBe(true)
  })

  it('setup persists identity and updates state', () => {
    const { result } = renderHook(() => useHumanIdentity())

    act(() => {
      result.current.setup('human-bob', 'Bob')
    })

    expect(result.current.identity).toEqual({ identifier: 'human-bob', displayName: 'Bob' })
    expect(result.current.isSetup).toBe(true)
    expect(JSON.parse(localStorage.getItem(IDENTITY_KEY) ?? '{}')).toEqual({
      identifier: 'human-bob',
      displayName: 'Bob',
    })
  })

  it('clear removes identity from localStorage and resets state', () => {
    localStorage.setItem(
      IDENTITY_KEY,
      JSON.stringify({ identifier: 'human-alice', displayName: 'Alice' }),
    )
    const { result } = renderHook(() => useHumanIdentity())
    expect(result.current.isSetup).toBe(true)

    act(() => {
      result.current.clear()
    })

    expect(result.current.identity).toBeNull()
    expect(result.current.isSetup).toBe(false)
    expect(localStorage.getItem(IDENTITY_KEY)).toBeNull()
  })

  it('returns null for malformed localStorage data', () => {
    localStorage.setItem(IDENTITY_KEY, 'not-json')
    const { result } = renderHook(() => useHumanIdentity())
    expect(result.current.identity).toBeNull()
  })

  it('returns null when localStorage entry is missing required fields', () => {
    localStorage.setItem(IDENTITY_KEY, JSON.stringify({ identifier: 'only-id' }))
    const { result } = renderHook(() => useHumanIdentity())
    expect(result.current.identity).toBeNull()
  })
})
