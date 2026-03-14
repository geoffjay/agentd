/**
 * Tests for useMemories hook.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor, act } from '@testing-library/react'
import { useMemories } from '@/hooks/useMemories'
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
// useMemories
// ---------------------------------------------------------------------------

describe('useMemories', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    mockApiError.mockClear()
  })

  it('fetches and returns memories', async () => {
    const mem = makeMemory()
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [mem],
      total: 1,
      limit: 200,
      offset: 0,
    })

    const { result } = renderHook(() => useMemories())
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.memories).toHaveLength(1)
    expect(result.current.memories[0].content).toBe('Test memory content')
    expect(result.current.total).toBe(1)
  })

  it('sets error on fetch failure', async () => {
    vi.spyOn(memoryClient, 'listMemories').mockRejectedValue(
      new Error('Service unavailable'),
    )

    const { result } = renderHook(() => useMemories())
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.error).toBe('Service unavailable')
    expect(result.current.memories).toHaveLength(0)
  })

  it('filters by client-side content search', async () => {
    const m1 = makeMemory({ id: 'm1', content: 'Deploy the API' })
    const m2 = makeMemory({ id: 'm2', content: 'Fix the bug' })
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [m1, m2],
      total: 2,
      limit: 200,
      offset: 0,
    })

    const { result } = renderHook(() => useMemories({ search: 'deploy' }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.memories).toHaveLength(1)
    expect(result.current.memories[0].content).toBe('Deploy the API')
  })

  it('passes server-side filters to listMemories', async () => {
    const spy = vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [],
      total: 0,
      limit: 200,
      offset: 0,
    })

    const { result } = renderHook(() =>
      useMemories({
        filters: { type: 'question', visibility: 'private' },
      }),
    )
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(spy).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'question',
        visibility: 'private',
        limit: 200,
      }),
    )
  })

  it('sorts by created_at descending by default', async () => {
    const m1 = makeMemory({ id: 'm1', created_at: '2024-01-10T00:00:00Z' })
    const m2 = makeMemory({ id: 'm2', created_at: '2024-01-20T00:00:00Z' })
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [m1, m2],
      total: 2,
      limit: 200,
      offset: 0,
    })

    const { result } = renderHook(() => useMemories())
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.memories[0].id).toBe('m2')
    expect(result.current.memories[1].id).toBe('m1')
  })

  it('paginates results', async () => {
    const items = Array.from({ length: 5 }, (_, i) =>
      makeMemory({ id: `m${i}`, created_at: `2024-01-0${i + 1}T00:00:00Z` }),
    )
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items,
      total: 5,
      limit: 200,
      offset: 0,
    })

    const { result } = renderHook(() => useMemories({ page: 2, pageSize: 2 }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    // Sorted desc: m4, m3, m2, m1, m0 → page 2 (size 2) = m2, m1
    expect(result.current.memories).toHaveLength(2)
    expect(result.current.total).toBe(5)
  })

  it('createMemory calls client and refreshes', async () => {
    const existing = makeMemory()
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [existing],
      total: 1,
      limit: 200,
      offset: 0,
    })
    const created = makeMemory({ id: 'new-mem', content: 'New memory' })
    vi.spyOn(memoryClient, 'createMemory').mockResolvedValue(created)

    const { result } = renderHook(() => useMemories())
    await waitFor(() => expect(result.current.loading).toBe(false))

    const returned = await act(() =>
      result.current.createMemory({ content: 'New memory', created_by: 'user-1' }),
    )
    expect(returned.id).toBe('new-mem')
  })

  it('deleteMemory removes memory from state', async () => {
    const mem = makeMemory()
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [mem],
      total: 1,
      limit: 200,
      offset: 0,
    })
    vi.spyOn(memoryClient, 'deleteMemory').mockResolvedValue({ deleted: true })

    const { result } = renderHook(() => useMemories())
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.memories).toHaveLength(1)

    await act(() => result.current.deleteMemory(mem.id))
    await waitFor(() => expect(result.current.memories).toHaveLength(0))
  })

  it('updateVisibility updates memory in state', async () => {
    const mem = makeMemory({ visibility: 'public' })
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [mem],
      total: 1,
      limit: 200,
      offset: 0,
    })
    const updated = { ...mem, visibility: 'private' as const }
    vi.spyOn(memoryClient, 'updateVisibility').mockResolvedValue(updated)

    const { result } = renderHook(() => useMemories())
    await waitFor(() => expect(result.current.loading).toBe(false))

    await act(() =>
      result.current.updateVisibility(mem.id, { visibility: 'private' }),
    )
    await waitFor(() =>
      expect(result.current.memories[0].visibility).toBe('private'),
    )
  })

  it('shows toast on createMemory failure', async () => {
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [],
      total: 0,
      limit: 200,
      offset: 0,
    })
    vi.spyOn(memoryClient, 'createMemory').mockRejectedValue(
      new Error('Bad request'),
    )

    const { result } = renderHook(() => useMemories())
    await waitFor(() => expect(result.current.loading).toBe(false))

    await expect(
      act(() =>
        result.current.createMemory({ content: 'test', created_by: 'user-1' }),
      ),
    ).rejects.toThrow('Bad request')
    expect(mockApiError).toHaveBeenCalledWith(
      expect.any(Error),
      'Failed to create memory',
    )
  })
})
