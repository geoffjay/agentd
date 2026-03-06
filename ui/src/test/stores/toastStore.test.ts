/**
 * Tests for toastStore.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { toastStore } from '@/stores/toastStore'

beforeEach(() => {
  toastStore.clear()
})

describe('toastStore', () => {
  it('starts with an empty list', () => {
    expect(toastStore.getToasts()).toHaveLength(0)
  })

  it('adds a toast and returns its id', () => {
    const id = toastStore.add({ type: 'info', title: 'Hello', duration: 5000 })
    expect(id).toBeTruthy()
    const toasts = toastStore.getToasts()
    expect(toasts).toHaveLength(1)
    expect(toasts[0].title).toBe('Hello')
    expect(toasts[0].id).toBe(id)
  })

  it('convenience success() method sets correct type', () => {
    toastStore.success('Done!')
    const toasts = toastStore.getToasts()
    expect(toasts[0].type).toBe('success')
  })

  it('convenience error() method sets correct type', () => {
    toastStore.error('Oops!')
    const toasts = toastStore.getToasts()
    expect(toasts[0].type).toBe('error')
  })

  it('convenience warning() method sets correct type', () => {
    toastStore.warning('Watch out')
    const toasts = toastStore.getToasts()
    expect(toasts[0].type).toBe('warning')
  })

  it('convenience info() method sets correct type', () => {
    toastStore.info('FYI')
    const toasts = toastStore.getToasts()
    expect(toasts[0].type).toBe('info')
  })

  it('dismisses a toast by id', () => {
    const id = toastStore.success('Toast 1')
    toastStore.info('Toast 2')
    toastStore.dismiss(id)
    const toasts = toastStore.getToasts()
    expect(toasts).toHaveLength(1)
    expect(toasts[0].title).toBe('Toast 2')
  })

  it('clear() removes all toasts', () => {
    toastStore.success('A')
    toastStore.error('B')
    toastStore.clear()
    expect(toastStore.getToasts()).toHaveLength(0)
  })

  it('notifies subscribers when a toast is added', () => {
    const listener = vi.fn()
    const unsub = toastStore.subscribe(listener)
    toastStore.info('Notify me')
    expect(listener).toHaveBeenCalledTimes(1)
    const [toasts] = listener.mock.calls[0] as [unknown[]]
    expect(Array.isArray(toasts)).toBe(true)
    unsub()
  })

  it('notifies subscribers when a toast is dismissed', () => {
    const id = toastStore.info('Hi')
    const listener = vi.fn()
    const unsub = toastStore.subscribe(listener)
    toastStore.dismiss(id)
    expect(listener).toHaveBeenCalled()
    unsub()
  })

  it('unsubscribes correctly', () => {
    const listener = vi.fn()
    const unsub = toastStore.subscribe(listener)
    unsub()
    toastStore.success('After unsub')
    expect(listener).not.toHaveBeenCalled()
  })

  it('stacks multiple toasts', () => {
    toastStore.success('A')
    toastStore.error('B')
    toastStore.warning('C')
    expect(toastStore.getToasts()).toHaveLength(3)
  })

  it('errors default to 8s duration', () => {
    toastStore.error('Oops')
    expect(toastStore.getToasts()[0].duration).toBe(8_000)
  })

  it('success defaults to 5s duration', () => {
    toastStore.success('Yay')
    expect(toastStore.getToasts()[0].duration).toBe(5_000)
  })
})
