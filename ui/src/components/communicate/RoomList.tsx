/**
 * RoomList — sidebar component for browsing communicate rooms.
 *
 * Features:
 * - Displays room name, type badge, and topic
 * - Highlights the selected room
 * - Search/filter by name
 * - Unread indicator based on last-read timestamp stored in localStorage
 * - Loading skeleton while fetching
 */

import { useState } from 'react'
import { Hash, Lock, Radio, Search } from 'lucide-react'
import type { Room, RoomType } from '@/types/communicate'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const LAST_READ_KEY = 'agentd:communicate:last-read'

function getLastRead(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(LAST_READ_KEY) ?? '{}')
  } catch {
    return {}
  }
}

export function markRoomAsRead(roomId: string): void {
  const lastRead = getLastRead()
  lastRead[roomId] = new Date().toISOString()
  localStorage.setItem(LAST_READ_KEY, JSON.stringify(lastRead))
}

function isUnread(room: Room): boolean {
  const lastRead = getLastRead()
  const readAt = lastRead[room.id]
  if (!readAt) return false
  return new Date(room.updated_at) > new Date(readAt)
}

// ---------------------------------------------------------------------------
// Room type icon
// ---------------------------------------------------------------------------

function RoomTypeIcon({ type }: { type: RoomType }) {
  switch (type) {
    case 'direct':
      return <Lock size={14} className="shrink-0 text-gray-400" />
    case 'broadcast':
      return <Radio size={14} className="shrink-0 text-yellow-400" />
    default:
      return <Hash size={14} className="shrink-0 text-gray-400" />
  }
}

// ---------------------------------------------------------------------------
// Single room item
// ---------------------------------------------------------------------------

interface RoomItemProps {
  room: Room
  selected: boolean
  onClick: () => void
}

function RoomItem({ room, selected, onClick }: RoomItemProps) {
  const unread = isUnread(room)

  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        'w-full flex items-start gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors',
        selected
          ? 'bg-primary-700 text-white'
          : 'text-gray-300 hover:bg-gray-700 hover:text-white',
      ].join(' ')}
    >
      <span className="mt-0.5">
        <RoomTypeIcon type={room.room_type} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-1.5">
          <span className="truncate font-medium">{room.name}</span>
          {unread && !selected && (
            <span className="h-2 w-2 shrink-0 rounded-full bg-primary-400" aria-label="Unread messages" />
          )}
        </span>
        {room.topic && (
          <span className="block truncate text-xs text-gray-400 mt-0.5">{room.topic}</span>
        )}
      </span>
    </button>
  )
}

// ---------------------------------------------------------------------------
// RoomList
// ---------------------------------------------------------------------------

interface RoomListProps {
  rooms: Room[]
  selectedRoomId: string | undefined
  loading: boolean
  onSelectRoom: (room: Room) => void
}

export function RoomList({ rooms, selectedRoomId, loading, onSelectRoom }: RoomListProps) {
  const [search, setSearch] = useState('')

  const filtered = search
    ? rooms.filter((r) => r.name.toLowerCase().includes(search.toLowerCase()))
    : rooms

  return (
    <div className="flex h-full flex-col">
      {/* Search */}
      <div className="px-3 py-2">
        <div className="relative">
          <Search
            size={14}
            className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400 pointer-events-none"
          />
          <input
            type="search"
            placeholder="Find a room…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full rounded-md bg-gray-700 pl-8 pr-3 py-1.5 text-sm text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary-500"
            aria-label="Search rooms"
          />
        </div>
      </div>

      {/* List */}
      <nav
        aria-label="Rooms"
        className="flex-1 overflow-y-auto px-2 py-1 space-y-0.5"
      >
        {loading ? (
          // Skeleton
          Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-10 rounded-md bg-gray-700 animate-pulse mx-1"
              aria-hidden="true"
            />
          ))
        ) : filtered.length === 0 ? (
          <p className="px-3 py-4 text-center text-xs text-gray-500">
            {search ? 'No rooms match your search.' : 'No rooms yet.'}
          </p>
        ) : (
          filtered.map((room) => (
            <RoomItem
              key={room.id}
              room={room}
              selected={room.id === selectedRoomId}
              onClick={() => onSelectRoom(room)}
            />
          ))
        )}
      </nav>
    </div>
  )
}
