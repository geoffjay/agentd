/**
 * Tests for useToast hook and mapApiError utility.
 */

import { describe, it, expect, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useToast, mapApiError } from '@/hooks/useToast'
import { toastStore } from '@/stores/toastStore'
import { ApiError } from '@/types/common'

beforeEach(() => {
  toastStore.clear()
})

describe('mapApiError', () => {
  it('maps status 0 to service unavailable message', () => {
    const err = new ApiError(0, 'Network error')
    expect(mapApiError(err)).toContain('unavailable')
  })

  it('maps status 404 to resource not found', () => {
    const err = new ApiError(404, 'Not found')
    expect(mapApiError(err)).toBe('Resource not found')
  })

  it('maps status 409 to conflict message', () => {
    const err = new ApiError(409, 'Conflict')
    expect(mapApiError(err)).toContain('Conflict')
  })

  it('maps status 500 to server error message', () => {
    const err = new ApiError(500, 'Internal error')
    expect(mapApiError(err)).toContain('Server error')
  })

  it('maps status 400 with custom message', () => {
    const err = new ApiError(400, 'Name is required')
    expect(mapApiError(err)).toBe('Name is required')
  })

  it('returns message from plain Error', () => {
    expect(mapApiError(new Error('boom'))).toBe('boom')
  })

  it('stringifies unknown error', () => {
    expect(mapApiError('oops')).toBe('oops')
  })
})

describe('useToast', () => {
  it('success() adds a success toast', () => {
    const { result } = renderHook(() => useToast())
    act(() => {
      result.current.success('Great!')
    })
    const toasts = toastStore.getToasts()
    expect(toasts).toHaveLength(1)
    expect(toasts[0].type).toBe('success')
    expect(toasts[0].title).toBe('Great!')
  })

  it('error() adds an error toast', () => {
    const { result } = renderHook(() => useToast())
    act(() => {
      result.current.error('Failed!')
    })
    expect(toastStore.getToasts()[0].type).toBe('error')
  })

  it('apiError() maps ApiError to friendly message', () => {
    const { result } = renderHook(() => useToast())
    act(() => {
      result.current.apiError(new ApiError(404, 'Not found'))
    })
    const toasts = toastStore.getToasts()
    expect(toasts[0].type).toBe('error')
    expect(toasts[0].message).toBe('Resource not found')
  })

  it('dismiss() removes a specific toast', () => {
    const { result } = renderHook(() => useToast())
    let id = ''
    act(() => {
      id = result.current.info('Hello')
    })
    act(() => {
      result.current.dismiss(id)
    })
    expect(toastStore.getToasts()).toHaveLength(0)
  })

  it('clear() removes all toasts', () => {
    const { result } = renderHook(() => useToast())
    act(() => {
      result.current.success('A')
      result.current.error('B')
    })
    act(() => {
      result.current.clear()
    })
    expect(toastStore.getToasts()).toHaveLength(0)
  })
})
