/**
 * Tests for useRooms hook.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { useRooms } from '@/hooks/useRooms'
import { communicateClient } from '@/services/communicate'
import type { Room } from '@/types/communicate'

// ---------------------------------------------------------------------------
// Mock useToast
// ---------------------------------------------------------------------------

vi.mock('@/hooks/useToast', () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
    dismiss: vi.fn(),
    clear: vi.fn(),
    apiError: vi.fn(),
  }),
  mapApiError: (err: unknown) => (err instanceof Error ? err.message : String(err)),
}))

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeRoom(overrides: Partial<Room> = {}): Room {
  const id = Math.random().toString(36).slice(2)
  return {
    id,
    name: `room-${id}`,
    topic: `Topic ${id}`,
    description: null,
    room_type: 'group',
    created_by: 'agent-system',
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    ...overrides,
  }
}

function paginated(items: Room[]) {
  return { items, total: items.length, limit: 200, offset: 0 }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useRooms', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('loads rooms on mount', async () => {
    const rooms = [makeRoom(), makeRoom(), makeRoom()]
    vi.spyOn(communicateClient, 'listRooms').mockResolvedValue(paginated(rooms))

    const { result } = renderHook(() => useRooms())

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.rooms).toHaveLength(3)
    expect(result.current.rooms[0].name).toBe(rooms[0].name)
    expect(result.current.error).toBeUndefined()
  })

  it('filters rooms by name search client-side', async () => {
    const rooms = [
      makeRoom({ name: 'alpha-channel' }),
      makeRoom({ name: 'beta-channel' }),
      makeRoom({ name: 'gamma-ops' }),
    ]
    vi.spyOn(communicateClient, 'listRooms').mockResolvedValue(paginated(rooms))

    const { result } = renderHook(() => useRooms({ search: 'channel' }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.rooms).toHaveLength(2)
    expect(result.current.rooms.every((r) => r.name.includes('channel'))).toBe(true)
  })

  it('sets error on fetch failure', async () => {
    vi.spyOn(communicateClient, 'listRooms').mockRejectedValue(new Error('Network error'))

    const { result } = renderHook(() => useRooms({ refreshInterval: 0 }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.error).toBeDefined()
    expect(result.current.rooms).toHaveLength(0)
  })

  it('refetch re-loads rooms', async () => {
    const rooms = [makeRoom(), makeRoom()]
    const spy = vi.spyOn(communicateClient, 'listRooms').mockResolvedValue(paginated(rooms))

    const { result } = renderHook(() => useRooms())
    await waitFor(() => expect(result.current.loading).toBe(false))

    const before = spy.mock.calls.length
    result.current.refetch()
    await waitFor(() => expect(spy.mock.calls.length).toBeGreaterThan(before))
  })

  it('passes room_type filter to the API', async () => {
    const rooms = [makeRoom({ room_type: 'direct' })]
    const spy = vi.spyOn(communicateClient, 'listRooms').mockResolvedValue(paginated(rooms))

    const { result } = renderHook(() => useRooms({ roomType: 'direct' }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(spy).toHaveBeenCalledWith(expect.objectContaining({ room_type: 'direct' }))
  })
})
