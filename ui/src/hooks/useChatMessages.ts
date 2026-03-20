/**
 * useChatMessages — hook for loading and managing chat message history.
 *
 * Provides:
 * - Initial load of N most-recent messages via REST
 * - Cursor-based pagination for loading older messages (scroll-up)
 * - Append new messages received via WebSocket
 * - Deduplication by message ID
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { communicateClient } from '@/services/communicate'
import { useToast, mapApiError } from '@/hooks/useToast'
import type { ChatMessage } from '@/types/communicate'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseChatMessagesOptions {
  /** The room to load messages for. Set to undefined to skip fetching. */
  roomId: string | undefined
  /** Number of messages to load initially and per page (default 50). */
  pageSize?: number
}

export interface UseChatMessagesResult {
  messages: ChatMessage[]
  loading: boolean
  loadingOlder: boolean
  hasMore: boolean
  error?: string
  /** Append a message received over WebSocket (deduplicates by id). */
  appendMessage: (msg: ChatMessage) => void
  /** Load the next page of older messages. */
  loadOlder: () => Promise<void>
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useChatMessages({
  roomId,
  pageSize = 50,
}: UseChatMessagesOptions): UseChatMessagesResult {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [loading, setLoading] = useState(false)
  const [loadingOlder, setLoadingOlder] = useState(false)
  const [hasMore, setHasMore] = useState(true)
  const [error, setError] = useState<string | undefined>()
  // The oldest message ID we've loaded — used as `before` cursor
  const [oldestId, setOldestId] = useState<string | undefined>()
  const toast = useToast()
  // Stable ref so toast is never a useEffect/useCallback dependency
  const toastRef = useRef(toast)
  toastRef.current = toast

  // -------------------------------------------------------------------------
  // Initial load
  // -------------------------------------------------------------------------

  useEffect(() => {
    if (!roomId) {
      setMessages([])
      setOldestId(undefined)
      setHasMore(true)
      return
    }

    let cancelled = false

    async function load() {
      setLoading(true)
      setError(undefined)

      try {
        const items = await communicateClient.getLatestMessages(roomId!, pageSize)
        if (!cancelled) {
          setMessages(items)
          setHasMore(items.length >= pageSize)
          setOldestId(items[0]?.id)
        }
      } catch (err) {
        if (!cancelled) {
          const msg = mapApiError(err)
          setError(msg)
          toastRef.current.error('Failed to load messages', msg)
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }

    load()
    return () => {
      cancelled = true
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [roomId, pageSize])

  // -------------------------------------------------------------------------
  // Load older (cursor pagination)
  // -------------------------------------------------------------------------

  const loadOlder = useCallback(async () => {
    if (!roomId || !hasMore || loadingOlder || !oldestId) return

    setLoadingOlder(true)
    try {
      const result = await communicateClient.listMessages(roomId, {
        limit: pageSize,
        before: oldestId,
      })
      if (result.items.length === 0) {
        setHasMore(false)
        return
      }
      setMessages((prev) => {
        const existingIds = new Set(prev.map((m) => m.id))
        const newItems = result.items.filter((m) => !existingIds.has(m.id))
        return [...newItems, ...prev]
      })
      setHasMore(result.items.length >= pageSize)
      setOldestId(result.items[0]?.id ?? oldestId)
    } catch (err) {
      toastRef.current.error('Failed to load older messages', mapApiError(err))
    } finally {
      setLoadingOlder(false)
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [roomId, hasMore, loadingOlder, oldestId, pageSize])

  // -------------------------------------------------------------------------
  // Append (WebSocket)
  // -------------------------------------------------------------------------

  const appendMessage = useCallback((msg: ChatMessage) => {
    setMessages((prev) => {
      if (prev.some((m) => m.id === msg.id)) return prev
      return [...prev, msg]
    })
  }, [])

  return { messages, loading, loadingOlder, hasMore, error, appendMessage, loadOlder }
}
