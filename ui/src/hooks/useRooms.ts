/**
 * useRooms — hook for listing and managing communicate rooms.
 *
 * Provides:
 * - Paginated room list with optional type filter and name search
 * - Auto-refresh every 30 seconds (configurable)
 * - Error handling with toast notifications
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { communicateClient } from '@/services/communicate'
import { useToast, mapApiError } from '@/hooks/useToast'
import type { Room, RoomType } from '@/types/communicate'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseRoomsOptions {
  /** Filter by room type. */
  roomType?: RoomType
  /** Client-side name filter. */
  search?: string
  /** Pause auto-refresh. */
  paused?: boolean
  /** Auto-refresh interval (ms); default 30 000. */
  refreshInterval?: number
}

export interface UseRoomsResult {
  rooms: Room[]
  total: number
  loading: boolean
  refreshing: boolean
  error?: string
  refetch: () => void
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

const DEFAULT_REFRESH_INTERVAL = 30_000

export function useRooms({
  roomType,
  search = '',
  paused = false,
  refreshInterval = DEFAULT_REFRESH_INTERVAL,
}: UseRoomsOptions = {}): UseRoomsResult {
  const [allRooms, setAllRooms] = useState<Room[]>([])
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | undefined>()
  const toast = useToast()
  // Stable ref so toast is never a useCallback dependency
  const toastRef = useRef(toast)
  toastRef.current = toast
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetchRooms = useCallback(
    async (isBackground = false) => {
      if (isBackground) {
        setRefreshing(true)
      } else {
        setLoading(true)
        setError(undefined)
      }

      try {
        const result = await communicateClient.listRooms({
          limit: 200,
          ...(roomType ? { room_type: roomType } : {}),
        })
        setAllRooms(result.items)
        setError(undefined)
      } catch (err) {
        const msg = mapApiError(err)
        setError(msg)
        if (!isBackground) {
          toastRef.current.error('Failed to load rooms', msg)
        }
      } finally {
        setLoading(false)
        setRefreshing(false)
      }
    },
    [roomType],
  )

  useEffect(() => {
    fetchRooms(false)
  }, [fetchRooms])

  useEffect(() => {
    if (paused) {
      if (timerRef.current) clearInterval(timerRef.current)
      return
    }
    timerRef.current = setInterval(() => fetchRooms(true), refreshInterval)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [paused, refreshInterval, fetchRooms])

  // Client-side name filter
  const rooms = search
    ? allRooms.filter((r) => r.name.toLowerCase().includes(search.toLowerCase()))
    : allRooms

  const refetch = useCallback(() => fetchRooms(false), [fetchRooms])

  return { rooms, total: rooms.length, loading, refreshing, error, refetch }
}
