/**
 * Tests for useHooks, useHookEvents, and useHookServiceStatus stubs.
 */

import { describe, it, expect } from 'vitest'
import { renderHook } from '@testing-library/react'
import {
  useHooks,
  useHookEvents,
  useHookServiceStatus,
  HOOK_SERVICE_PORT,
} from '@/hooks/useHooks'

describe('HOOK_SERVICE_PORT', () => {
  it('is 17002', () => {
    expect(HOOK_SERVICE_PORT).toBe(17002)
  })
})

describe('useHooks', () => {
  it('returns empty hooks list', () => {
    const { result } = renderHook(() => useHooks())
    expect(result.current.hooks).toEqual([])
  })

  it('returns total of 0', () => {
    const { result } = renderHook(() => useHooks())
    expect(result.current.total).toBe(0)
  })

  it('is not loading', () => {
    const { result } = renderHook(() => useHooks())
    expect(result.current.loading).toBe(false)
  })

  it('has no error', () => {
    const { result } = renderHook(() => useHooks())
    expect(result.current.error).toBeUndefined()
  })

  it('exposes a refetch function', () => {
    const { result } = renderHook(() => useHooks())
    expect(typeof result.current.refetch).toBe('function')
  })

  it('refetch does not throw', () => {
    const { result } = renderHook(() => useHooks())
    expect(() => result.current.refetch()).not.toThrow()
  })
})

describe('useHookEvents', () => {
  it('returns empty events list', () => {
    const { result } = renderHook(() => useHookEvents())
    expect(result.current.events).toEqual([])
  })

  it('returns total of 0', () => {
    const { result } = renderHook(() => useHookEvents())
    expect(result.current.total).toBe(0)
  })

  it('is not loading', () => {
    const { result } = renderHook(() => useHookEvents())
    expect(result.current.loading).toBe(false)
  })

  it('has no error', () => {
    const { result } = renderHook(() => useHookEvents())
    expect(result.current.error).toBeUndefined()
  })

  it('exposes a refetch function', () => {
    const { result } = renderHook(() => useHookEvents())
    expect(typeof result.current.refetch).toBe('function')
  })
})

describe('useHookServiceStatus', () => {
  it('reports service as unreachable', () => {
    const { result } = renderHook(() => useHookServiceStatus())
    expect(result.current.reachable).toBe(false)
  })

  it('is not checking', () => {
    const { result } = renderHook(() => useHookServiceStatus())
    expect(result.current.checking).toBe(false)
  })

  it('reports correct port', () => {
    const { result } = renderHook(() => useHookServiceStatus())
    expect(result.current.port).toBe(HOOK_SERVICE_PORT)
  })
})
