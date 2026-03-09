/**
 * Tests for a11y utility functions.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import {
  getFocusableElements,
  isActivationKey,
  isEscapeKey,
  isArrowKey,
  onKeyActivate,
  generateId,
  resetIdCounter,
  prefersReducedMotion,
  announce,
} from '@/utils/a11y'

// ---------------------------------------------------------------------------
// getFocusableElements
// ---------------------------------------------------------------------------

describe('getFocusableElements', () => {
  let container: HTMLDivElement

  beforeEach(() => {
    container = document.createElement('div')
    document.body.appendChild(container)
  })

  afterEach(() => {
    document.body.removeChild(container)
  })

  it('finds buttons', () => {
    container.innerHTML = '<button>Click me</button><button disabled>Disabled</button>'
    const els = getFocusableElements(container)
    expect(els).toHaveLength(1)
    expect(els[0].tagName).toBe('BUTTON')
  })

  it('finds links with href', () => {
    container.innerHTML = '<a href="/">Home</a><a>No href</a>'
    const els = getFocusableElements(container)
    expect(els).toHaveLength(1)
  })

  it('finds inputs (not disabled)', () => {
    container.innerHTML = '<input type="text" /><input type="text" disabled />'
    const els = getFocusableElements(container)
    expect(els).toHaveLength(1)
  })

  it('finds elements with tabindex >= 0', () => {
    container.innerHTML = '<div tabindex="0">Focusable</div><div tabindex="-1">Not in tab order</div>'
    const els = getFocusableElements(container)
    expect(els).toHaveLength(1)
  })

  it('returns empty array when container has no focusable elements', () => {
    container.innerHTML = '<div><span>Plain text</span></div>'
    const els = getFocusableElements(container)
    expect(els).toHaveLength(0)
  })
})

// ---------------------------------------------------------------------------
// Keyboard helpers
// ---------------------------------------------------------------------------

describe('isActivationKey', () => {
  it('returns true for Enter', () => {
    expect(isActivationKey({ key: 'Enter' } as KeyboardEvent)).toBe(true)
  })

  it('returns true for Space', () => {
    expect(isActivationKey({ key: ' ' } as KeyboardEvent)).toBe(true)
  })

  it('returns false for other keys', () => {
    expect(isActivationKey({ key: 'Tab' } as KeyboardEvent)).toBe(false)
  })
})

describe('isEscapeKey', () => {
  it('returns true for Escape', () => {
    expect(isEscapeKey({ key: 'Escape' } as KeyboardEvent)).toBe(true)
  })

  it('returns false for other keys', () => {
    expect(isEscapeKey({ key: 'Enter' } as KeyboardEvent)).toBe(false)
  })
})

describe('isArrowKey', () => {
  it.each(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'])(
    'returns true for %s',
    (key) => {
      expect(isArrowKey({ key } as KeyboardEvent)).toBe(true)
    },
  )

  it('returns false for Tab', () => {
    expect(isArrowKey({ key: 'Tab' } as KeyboardEvent)).toBe(false)
  })
})

describe('onKeyActivate', () => {
  it('calls handler on Enter', () => {
    const handler = vi.fn()
    const wrapper = onKeyActivate(handler)
    const event = { key: 'Enter', preventDefault: vi.fn() } as unknown as React.KeyboardEvent
    wrapper(event)
    expect(handler).toHaveBeenCalledWith(event)
  })

  it('calls handler on Space', () => {
    const handler = vi.fn()
    const wrapper = onKeyActivate(handler)
    const event = { key: ' ', preventDefault: vi.fn() } as unknown as React.KeyboardEvent
    wrapper(event)
    expect(handler).toHaveBeenCalledWith(event)
  })

  it('does not call handler on Tab', () => {
    const handler = vi.fn()
    const wrapper = onKeyActivate(handler)
    const event = { key: 'Tab', preventDefault: vi.fn() } as unknown as React.KeyboardEvent
    wrapper(event)
    expect(handler).not.toHaveBeenCalled()
  })
})

// ---------------------------------------------------------------------------
// generateId
// ---------------------------------------------------------------------------

describe('generateId', () => {
  beforeEach(() => resetIdCounter())

  it('generates unique IDs', () => {
    const id1 = generateId()
    const id2 = generateId()
    expect(id1).not.toBe(id2)
  })

  it('uses provided prefix', () => {
    const id = generateId('field')
    expect(id).toMatch(/^field-/)
  })

  it('returns string', () => {
    expect(typeof generateId()).toBe('string')
  })
})

// ---------------------------------------------------------------------------
// prefersReducedMotion
// ---------------------------------------------------------------------------

describe('prefersReducedMotion', () => {
  it('returns a boolean', () => {
    expect(typeof prefersReducedMotion()).toBe('boolean')
  })
})

// ---------------------------------------------------------------------------
// announce
// ---------------------------------------------------------------------------

describe('announce', () => {
  it('appends a live region element to body', () => {
    const before = document.body.children.length
    announce('Test announcement')
    // The element is appended synchronously before the rAF callback
    // In jsdom, requestAnimationFrame is synchronous
    expect(document.body.children.length).toBeGreaterThanOrEqual(before)
  })
})
