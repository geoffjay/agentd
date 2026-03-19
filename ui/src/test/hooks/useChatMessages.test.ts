/**
 * Tests for useChatMessages hook.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor, act } from '@testing-library/react'
import { useChatMessages } from '@/hooks/useChatMessages'
import { communicateClient } from '@/services/communicate'
import type { ChatMessage } from '@/types/communicate'

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

function makeMsg(overrides: Partial<ChatMessage> = {}): ChatMessage {
  const id = Math.random().toString(36).slice(2)
  return {
    id,
    room_id: 'room-1',
    sender_id: 'agent-1',
    sender_name: 'Agent One',
    sender_kind: 'agent',
    content: `Message ${id}`,
    metadata: {},
    reply_to: null,
    status: 'sent',
    created_at: new Date().toISOString(),
    ...overrides,
  }
}

function paginated(items: ChatMessage[]) {
  return { items, total: items.length, limit: 50, offset: 0 }
}

const ROOM_ID = 'room-test-123'

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useChatMessages', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('loads latest messages on mount', async () => {
    const messages = [makeMsg(), makeMsg(), makeMsg()]
    vi.spyOn(communicateClient, 'getLatestMessages').mockResolvedValue(paginated(messages))

    const { result } = renderHook(() => useChatMessages({ roomId: ROOM_ID }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.messages).toHaveLength(3)
    expect(result.current.error).toBeUndefined()
    expect(communicateClient.getLatestMessages).toHaveBeenCalledWith(ROOM_ID, 50)
  })

  it('does not fetch when roomId is undefined', async () => {
    const spy = vi.spyOn(communicateClient, 'getLatestMessages')

    const { result } = renderHook(() => useChatMessages({ roomId: undefined }))

    // Give it time to potentially fetch
    await new Promise((r) => setTimeout(r, 30))

    expect(spy).not.toHaveBeenCalled()
    expect(result.current.messages).toHaveLength(0)
    expect(result.current.loading).toBe(false)
  })

  it('resets messages when roomId changes', async () => {
    const msgs1 = [makeMsg({ room_id: 'room-1' }), makeMsg({ room_id: 'room-1' })]
    const msgs2 = [makeMsg({ room_id: 'room-2' })]

    vi.spyOn(communicateClient, 'getLatestMessages')
      .mockResolvedValueOnce(paginated(msgs1))
      .mockResolvedValueOnce(paginated(msgs2))

    const { result, rerender } = renderHook(
      ({ roomId }: { roomId: string }) => useChatMessages({ roomId }),
      { initialProps: { roomId: 'room-1' } },
    )

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.messages).toHaveLength(2)

    rerender({ roomId: 'room-2' })
    await waitFor(() => expect(result.current.messages).toHaveLength(1))
  })

  it('appendMessage deduplicates by id', async () => {
    const messages = [makeMsg(), makeMsg()]
    vi.spyOn(communicateClient, 'getLatestMessages').mockResolvedValue(paginated(messages))

    const { result } = renderHook(() => useChatMessages({ roomId: ROOM_ID }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    const newMsg = makeMsg()

    act(() => { result.current.appendMessage(newMsg) })
    expect(result.current.messages).toHaveLength(3)

    // Append same message again — should not duplicate
    act(() => { result.current.appendMessage(newMsg) })
    expect(result.current.messages).toHaveLength(3)
  })

  it('loadOlder fetches older page and prepends', async () => {
    const latest = [makeMsg(), makeMsg(), makeMsg(), makeMsg(), makeMsg()]
    const older = [makeMsg(), makeMsg()]

    vi.spyOn(communicateClient, 'getLatestMessages').mockResolvedValue(
      paginated(latest),
    )
    vi.spyOn(communicateClient, 'listMessages').mockResolvedValue(paginated(older))

    const { result } = renderHook(() => useChatMessages({ roomId: ROOM_ID, pageSize: 5 }))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.hasMore).toBe(true)

    await act(async () => {
      await result.current.loadOlder()
    })

    expect(result.current.messages).toHaveLength(7)
  })

  it('sets hasMore to false when fewer messages than pageSize are returned', async () => {
    const messages = [makeMsg(), makeMsg()]
    vi.spyOn(communicateClient, 'getLatestMessages').mockResolvedValue(paginated(messages))

    const { result } = renderHook(() => useChatMessages({ roomId: ROOM_ID, pageSize: 10 }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.hasMore).toBe(false)
  })

  it('sets error on fetch failure', async () => {
    vi.spyOn(communicateClient, 'getLatestMessages').mockRejectedValue(
      new Error('Network error'),
    )

    const { result } = renderHook(() => useChatMessages({ roomId: ROOM_ID }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.error).toBeDefined()
    expect(result.current.messages).toHaveLength(0)
  })
})
