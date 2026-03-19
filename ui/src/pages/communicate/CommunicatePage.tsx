/**
 * CommunicatePage — full-screen chat UI for inter-agent communication.
 *
 * Layout:
 *   ┌──────────────┬───────────────────────────┬────────────────┐
 *   │  Room list   │  Chat message area         │  Participants  │
 *   │  (240px)     │  (flex-1)                  │  (220px)       │
 *   └──────────────┴───────────────────────────┴────────────────┘
 *
 * - Room list sidebar: filterable list of rooms
 * - Chat area: message history + real-time WebSocket updates
 * - Participant panel: members of the selected room
 * - Connection status indicator in the header area
 */

import { useState, useCallback } from 'react'
import { MessageSquare, Wifi, WifiOff, Loader2 } from 'lucide-react'
import { RoomList, ChatMessageView, ParticipantPanel, markRoomAsRead } from '@/components/communicate'
import { useRooms } from '@/hooks/useRooms'
import { useChatMessages } from '@/hooks/useChatMessages'
import { useCommunicateSocket } from '@/hooks/useCommunicateSocket'
import type { ChatMessage, Participant, Room } from '@/types/communicate'
import type { ConnectionState } from '@/hooks/useWebSocket'

// ---------------------------------------------------------------------------
// Connection status indicator
// ---------------------------------------------------------------------------

function ConnectionIndicator({ state }: { state: ConnectionState }) {
  if (state === 'Connected') {
    return (
      <span className="flex items-center gap-1.5 text-xs text-green-400">
        <Wifi size={14} />
        Live
      </span>
    )
  }
  if (state === 'Connecting' || state === 'Reconnecting') {
    return (
      <span className="flex items-center gap-1.5 text-xs text-yellow-400">
        <Loader2 size={14} className="animate-spin" />
        {state === 'Reconnecting' ? 'Reconnecting…' : 'Connecting…'}
      </span>
    )
  }
  return (
    <span className="flex items-center gap-1.5 text-xs text-gray-500">
      <WifiOff size={14} />
      Offline
    </span>
  )
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

function NoRoomSelected() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
      <MessageSquare size={40} className="text-gray-600" />
      <div>
        <p className="text-sm font-medium text-gray-300">No room selected</p>
        <p className="mt-1 text-xs text-gray-500">Pick a room from the sidebar to start chatting.</p>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// CommunicatePage
// ---------------------------------------------------------------------------

// This is a placeholder participant ID — in a real deployment this would come
// from an auth context or settings store.
const LOCAL_PARTICIPANT_ID = 'human-ui'

export function CommunicatePage() {
  const [selectedRoom, setSelectedRoom] = useState<Room | undefined>()
  const [realtimeParticipants, setRealtimeParticipants] = useState<Participant[]>([])
  const [leftIdentifiers, setLeftIdentifiers] = useState<string[]>([])

  // Room list
  const { rooms, loading: roomsLoading } = useRooms()

  // Message history for selected room
  const { messages, loading: messagesLoading, loadingOlder, hasMore, appendMessage, loadOlder } =
    useChatMessages({ roomId: selectedRoom?.id })

  // Handle new message from WebSocket
  const handleMessage = useCallback(
    (msg: ChatMessage) => {
      appendMessage(msg)
    },
    [appendMessage],
  )

  // Handle participant joined via WebSocket
  const handleParticipantJoined = useCallback((participant: Participant) => {
    setRealtimeParticipants((prev) => {
      if (prev.some((p) => p.identifier === participant.identifier)) return prev
      return [...prev, participant]
    })
  }, [])

  // Handle participant left via WebSocket
  const handleParticipantLeft = useCallback((_roomId: string, identifier: string) => {
    setLeftIdentifiers((prev) => (prev.includes(identifier) ? prev : [...prev, identifier]))
  }, [])

  // WebSocket connection
  const { connectionState } = useCommunicateSocket({
    selectedRoomId: selectedRoom?.id,
    participantId: LOCAL_PARTICIPANT_ID,
    onMessage: handleMessage,
    onParticipantJoined: handleParticipantJoined,
    onParticipantLeft: handleParticipantLeft,
  })

  const handleSelectRoom = useCallback((room: Room) => {
    setSelectedRoom(room)
    setRealtimeParticipants([])
    setLeftIdentifiers([])
    markRoomAsRead(room.id)
  }, [])

  return (
    <div className="flex h-full overflow-hidden">
      {/* Room list sidebar */}
      <aside
        className="flex w-60 shrink-0 flex-col border-r border-gray-700 bg-gray-800"
        aria-label="Rooms sidebar"
      >
        <div className="flex h-12 shrink-0 items-center justify-between border-b border-gray-700 px-4">
          <h2 className="text-sm font-semibold text-white">Rooms</h2>
          <ConnectionIndicator state={connectionState} />
        </div>
        <RoomList
          rooms={rooms}
          selectedRoomId={selectedRoom?.id}
          loading={roomsLoading}
          onSelectRoom={handleSelectRoom}
        />
      </aside>

      {/* Main chat area */}
      <main className="flex flex-1 flex-col overflow-hidden">
        {selectedRoom ? (
          <>
            {/* Room header */}
            <div className="flex h-12 shrink-0 items-center gap-3 border-b border-gray-700 px-4">
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-sm font-semibold text-white">{selectedRoom.name}</h2>
                {selectedRoom.topic && (
                  <p className="truncate text-xs text-gray-400">{selectedRoom.topic}</p>
                )}
              </div>
            </div>

            {/* Messages */}
            <ChatMessageView
              messages={messages}
              loading={messagesLoading}
              loadingOlder={loadingOlder}
              hasMore={hasMore}
              onLoadOlder={loadOlder}
            />
          </>
        ) : (
          <NoRoomSelected />
        )}
      </main>

      {/* Participant panel */}
      <aside
        className="hidden w-56 shrink-0 flex-col border-l border-gray-700 bg-gray-800 lg:flex"
        aria-label="Participants"
      >
        <div className="flex h-12 shrink-0 items-center border-b border-gray-700 px-4">
          <h2 className="text-sm font-semibold text-white">Participants</h2>
        </div>
        <ParticipantPanel
          roomId={selectedRoom?.id}
          realtimeParticipants={realtimeParticipants}
          leftIdentifiers={leftIdentifiers}
        />
      </aside>
    </div>
  )
}
