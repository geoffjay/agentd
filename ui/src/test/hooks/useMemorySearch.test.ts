/**
 * Tests for useMemorySearch hook.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor, act } from '@testing-library/react'
import { useMemorySearch } from '@/hooks/useMemorySearch'
import { memoryClient } from '@/services/memory'
import type { Memory } from '@/types/memory'

// ---------------------------------------------------------------------------
// Mock useToast
// ---------------------------------------------------------------------------

const mockApiError = vi.fn().mockReturnValue('toast-id')
vi.mock('@/hooks/useToast', () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
    dismiss: vi.fn(),
    clear: vi.fn(),
    apiError: mockApiError,
  }),
  mapApiError: (err: unknown) =>
    err instanceof Error ? err.message : String(err),
}))

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

function makeMemory(overrides: Partial<Memory> = {}): Memory {
  return {
    id: 'mem_1234567890_abcdef12',
    content: 'Test memory content',
    type: 'information',
    tags: ['test'],
    created_by: 'agent-1',
    owner: undefined,
    created_at: '2024-01-15T10:00:00Z',
    updated_at: '2024-01-15T10:00:00Z',
    visibility: 'public',
    shared_with: [],
    references: [],
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// useMemorySearch
// ---------------------------------------------------------------------------

describe('useMemorySearch', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockApiError.mockClear()
  })

  it('starts with empty results', () => {
    const { result } = renderHook(() => useMemorySearch())

    expect(result.current.results).toHaveLength(0)
    expect(result.current.total).toBe(0)
    expect(result.current.searching).toBe(false)
    expect(result.current.error).toBeUndefined()
  })

  it('executes search and returns results', async () => {
    const mem = makeMemory({ content: 'Deployment procedure' })
    vi.spyOn(memoryClient, 'searchMemories').mockResolvedValue({
      memories: [mem],
      total: 1,
    })

    const { result } = renderHook(() => useMemorySearch())

    await act(() => result.current.search({ query: 'deployment' }))

    await waitFor(() => expect(result.current.searching).toBe(false))
    expect(result.current.results).toHaveLength(1)
    expect(result.current.results[0].content).toBe('Deployment procedure')
    expect(result.current.total).toBe(1)
  })

  it('passes all search parameters to the client', async () => {
    const spy = vi.spyOn(memoryClient, 'searchMemories').mockResolvedValue({
      memories: [],
      total: 0,
    })

    const { result } = renderHook(() => useMemorySearch())

    await act(() =>
      result.current.search({
        query: 'test',
        as_actor: 'user-1',
        type: 'question',
        tags: ['important'],
        limit: 5,
      }),
    )

    expect(spy).toHaveBeenCalledWith({
      query: 'test',
      as_actor: 'user-1',
      type: 'question',
      tags: ['important'],
      limit: 5,
    })
  })

  it('sets error on search failure', async () => {
    vi.spyOn(memoryClient, 'searchMemories').mockRejectedValue(
      new Error('Vector search unavailable'),
    )

    const { result } = renderHook(() => useMemorySearch())

    await act(() => result.current.search({ query: 'test' }))

    await waitFor(() => expect(result.current.searching).toBe(false))
    expect(result.current.error).toBe('Vector search unavailable')
    expect(result.current.results).toHaveLength(0)
    expect(mockApiError).toHaveBeenCalledWith(
      expect.any(Error),
      'Memory search failed',
    )
  })

  it('clears results and error', async () => {
    const mem = makeMemory()
    vi.spyOn(memoryClient, 'searchMemories').mockResolvedValue({
      memories: [mem],
      total: 1,
    })

    const { result } = renderHook(() => useMemorySearch())

    // Search first
    await act(() => result.current.search({ query: 'test' }))
    await waitFor(() => expect(result.current.results).toHaveLength(1))

    // Then clear
    act(() => result.current.clear())

    expect(result.current.results).toHaveLength(0)
    expect(result.current.total).toBe(0)
    expect(result.current.error).toBeUndefined()
  })

  it('clears previous error on new search', async () => {
    vi.spyOn(memoryClient, 'searchMemories')
      .mockRejectedValueOnce(new Error('First failure'))
      .mockResolvedValueOnce({ memories: [], total: 0 })

    const { result } = renderHook(() => useMemorySearch())

    // First search fails
    await act(() => result.current.search({ query: 'bad' }))
    await waitFor(() => expect(result.current.error).toBe('First failure'))

    // Second search succeeds — error should be cleared
    await act(() => result.current.search({ query: 'good' }))
    await waitFor(() => expect(result.current.searching).toBe(false))
    expect(result.current.error).toBeUndefined()
  })
})
