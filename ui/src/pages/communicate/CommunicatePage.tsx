/**
 * CommunicatePage — full-screen chat UI for inter-agent communication.
 *
 * Layout:
 *   ┌──────────────┬───────────────────────────┬────────────────┐
 *   │  Room list   │  Chat message area         │  Participants  │
 *   │  (240px)     │  (flex-1)                  │  or Settings   │
 *   └──────────────┴───────────────────────────┴────────────────┘
 *
 * - Room list sidebar: filterable list of rooms with "Create" button
 * - Chat area: message history + message input + real-time WebSocket updates
 * - Right panel: participant list or room settings (toggled)
 * - Human identity setup modal on first visit
 * - Join/leave room support
 */

import { useState, useCallback, useEffect, useRef } from 'react'
import { useLayout } from '@/layouts/context'
import { MessageSquare, Wifi, WifiOff, Loader2, Plus, Settings } from 'lucide-react'
import {
  RoomList,
  ChatMessageView,
  ParticipantPanel,
  MessageInput,
  CreateRoomDialog,
  RoomSettingsPanel,
  HumanIdentitySetup,
  markRoomAsRead,
} from '@/components/communicate'
import { useRooms } from '@/hooks/useRooms'
import { useChatMessages } from '@/hooks/useChatMessages'
import { useCommunicateSocket } from '@/hooks/useCommunicateSocket'
import { useHumanIdentity } from '@/hooks/useHumanIdentity'
import { useToast } from '@/hooks/useToast'
import { communicateClient } from '@/services/communicate'
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

export function CommunicatePage() {
  const { sidebarOpen } = useLayout()
  const { identity, isSetup, setup } = useHumanIdentity()
  const toast = useToast()
  const toastRef = useRef(toast)
  toastRef.current = toast

  const [selectedRoom, setSelectedRoom] = useState<Room | undefined>()
  const [realtimeParticipants, setRealtimeParticipants] = useState<Participant[]>([])
  const [leftIdentifiers, setLeftIdentifiers] = useState<string[]>([])

  // Right panel state: 'participants' | 'settings'
  const [rightPanel, setRightPanel] = useState<'participants' | 'settings'>('participants')

  // Dialogs
  const [showIdentitySetup, setShowIdentitySetup] = useState(!isSetup)
  const [showCreateRoom, setShowCreateRoom] = useState(false)

  // Membership tracking
  const [isParticipant, setIsParticipant] = useState(false)
  const [joiningRoom, setJoiningRoom] = useState(false)

  // Room list
  const { rooms, loading: roomsLoading, refetch: refetchRooms } = useRooms()

  // Message history for selected room
  const {
    messages,
    loading: messagesLoading,
    loadingOlder,
    hasMore,
    appendMessage,
    loadOlder,
  } = useChatMessages({ roomId: selectedRoom?.id })

  // Check if local human is a participant when room changes.
  // Paginates through all participants to handle rooms with >100 members.
  useEffect(() => {
    if (!selectedRoom || !identity) {
      setIsParticipant(false)
      return
    }
    const roomId = selectedRoom.id
    const identifier = identity.identifier
    let cancelled = false

    async function checkMembership() {
      const limit = 100
      let offset = 0
      while (!cancelled) {
        const res = await communicateClient.listParticipants(roomId, { limit, offset })
        if (res.items.some((p) => p.identifier === identifier)) {
          if (!cancelled) setIsParticipant(true)
          return
        }
        if (res.items.length < limit) break // no more pages
        offset += limit
      }
      if (!cancelled) setIsParticipant(false)
    }

    checkMembership().catch(() => { if (!cancelled) setIsParticipant(false) })
    return () => { cancelled = true }
  }, [selectedRoom, identity])

  // Handle new message from WebSocket — skip if it's from us (we already appended optimistically)
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
    participantId: identity?.identifier ?? '',
    onMessage: handleMessage,
    onParticipantJoined: handleParticipantJoined,
    onParticipantLeft: handleParticipantLeft,
    paused: !identity,
  })

  const handleSelectRoom = useCallback((room: Room) => {
    setSelectedRoom(room)
    setRealtimeParticipants([])
    setLeftIdentifiers([])
    setRightPanel('participants')
    markRoomAsRead(room.id)
  }, [])

  // Send message (human)
  const handleSendMessage = useCallback(
    async (content: string) => {
      if (!selectedRoom || !identity) return
      try {
        const msg = await communicateClient.sendMessage(selectedRoom.id, {
          sender_id: identity.identifier,
          sender_name: identity.displayName,
          sender_kind: 'human',
          content,
        })
        appendMessage(msg)
      } catch (err) {
        toastRef.current.apiError(err, 'Failed to send message')
      }
    },
    [selectedRoom, identity, appendMessage],
  )

  // Join room
  const handleJoinRoom = useCallback(async () => {
    if (!selectedRoom || !identity) return
    setJoiningRoom(true)
    try {
      await communicateClient.addParticipant(selectedRoom.id, {
        identifier: identity.identifier,
        kind: 'human',
        display_name: identity.displayName,
      })
      setIsParticipant(true)
    } catch (err) {
      toastRef.current.apiError(err, 'Failed to join room')
    } finally {
      setJoiningRoom(false)
    }
  }, [selectedRoom, identity])

  // Room created
  const handleRoomCreated = useCallback(
    (room: Room) => {
      setShowCreateRoom(false)
      refetchRooms()
      setSelectedRoom(room)
      markRoomAsRead(room.id)
    },
    [refetchRooms],
  )

  // Room deleted
  const handleRoomDeleted = useCallback(() => {
    setSelectedRoom(undefined)
    setRightPanel('participants')
    refetchRooms()
  }, [refetchRooms])

  // Left room
  const handleLeft = useCallback(() => {
    setIsParticipant(false)
    setRightPanel('participants')
  }, [])

  // Room updated (topic/description changed)
  const handleRoomUpdated = useCallback((updated: Room) => {
    setSelectedRoom(updated)
  }, [])

  return (
    <div className={`fixed inset-0 top-16 flex overflow-hidden transition-all duration-300 ease-in-out ${sidebarOpen ? 'lg:left-60' : 'lg:left-16'}`}>
      {/* Identity setup modal — dismissible only when editing an existing identity */}
      <HumanIdentitySetup
        open={showIdentitySetup}
        onSave={(identifier, displayName) => {
          setup(identifier, displayName)
          setShowIdentitySetup(false)
        }}
        onClose={isSetup ? () => setShowIdentitySetup(false) : undefined}
      />

      {/* Create room dialog */}
      <CreateRoomDialog
        open={showCreateRoom}
        createdBy={identity?.identifier ?? 'human-ui'}
        onCreated={handleRoomCreated}
        onClose={() => setShowCreateRoom(false)}
      />

      {/* Room list sidebar */}
      <aside
        className="flex w-60 shrink-0 flex-col border-r border-gray-700 bg-gray-800"
        aria-label="Rooms sidebar"
      >
        <div className="flex h-12 shrink-0 items-center justify-between border-b border-gray-700 px-4">
          <h2 className="text-sm font-semibold text-white">Rooms</h2>
          <div className="flex items-center gap-2">
            <ConnectionIndicator state={connectionState} />
            <button
              type="button"
              onClick={() => setShowCreateRoom(true)}
              aria-label="Create room"
              className="rounded p-1 text-gray-400 hover:bg-gray-700 hover:text-white transition-colors"
            >
              <Plus size={16} />
            </button>
          </div>
        </div>
        <RoomList
          rooms={rooms}
          selectedRoomId={selectedRoom?.id}
          loading={roomsLoading}
          onSelectRoom={handleSelectRoom}
        />
        {/* Identity footer */}
        {identity && (
          <div className="shrink-0 border-t border-gray-700 px-3 py-2">
            <button
              type="button"
              onClick={() => setShowIdentitySetup(true)}
              className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-gray-400 hover:bg-gray-700 hover:text-white transition-colors"
              aria-label="Edit identity"
            >
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-emerald-700 text-white text-[10px] font-bold">
                {identity.displayName.charAt(0).toUpperCase()}
              </span>
              <span className="min-w-0">
                <span className="block truncate font-medium text-gray-300">{identity.displayName}</span>
                <span className="block truncate text-gray-500">{identity.identifier}</span>
              </span>
            </button>
          </div>
        )}
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
              <button
                type="button"
                onClick={() =>
                  setRightPanel((p) => (p === 'settings' ? 'participants' : 'settings'))
                }
                aria-label="Room settings"
                aria-pressed={rightPanel === 'settings'}
                className={[
                  'rounded p-1.5 transition-colors',
                  rightPanel === 'settings'
                    ? 'bg-gray-700 text-white'
                    : 'text-gray-400 hover:bg-gray-700 hover:text-white',
                ].join(' ')}
              >
                <Settings size={16} />
              </button>
            </div>

            {/* Messages */}
            <ChatMessageView
              messages={messages}
              loading={messagesLoading}
              loadingOlder={loadingOlder}
              hasMore={hasMore}
              onLoadOlder={loadOlder}
            />

            {/* Message input */}
            {identity && (
              <MessageInput
                onSend={handleSendMessage}
                isParticipant={isParticipant}
                onJoin={() => void handleJoinRoom()}
                joiningRoom={joiningRoom}
              />
            )}
          </>
        ) : (
          <NoRoomSelected />
        )}
      </main>

      {/* Right panel: Participants or Settings */}
      <aside
        className="hidden w-56 shrink-0 flex-col border-l border-gray-700 bg-gray-800 lg:flex"
        aria-label={rightPanel === 'settings' ? 'Room settings' : 'Participants'}
      >
        {rightPanel === 'settings' && selectedRoom && identity ? (
          <RoomSettingsPanel
            room={selectedRoom}
            localIdentifier={identity.identifier}
            onClose={() => setRightPanel('participants')}
            onRoomDeleted={handleRoomDeleted}
            onLeft={handleLeft}
            onRoomUpdated={handleRoomUpdated}
          />
        ) : (
          <>
            <div className="flex h-12 shrink-0 items-center border-b border-gray-700 px-4">
              <h2 className="text-sm font-semibold text-white">Participants</h2>
            </div>
            <ParticipantPanel
              roomId={selectedRoom?.id}
              realtimeParticipants={realtimeParticipants}
              leftIdentifiers={leftIdentifiers}
            />
          </>
        )}
      </aside>
    </div>
  )
}
