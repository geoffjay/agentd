/**
 * useCommunicateSocket — WebSocket hook for the communicate service.
 *
 * Connects to `GET /ws` on the communicate service, subscribes to the
 * selected room, and dispatches typed room events to caller-provided handlers.
 *
 * Features:
 * - Auto-subscribe / re-subscribe when selectedRoomId changes
 * - Unsubscribes from previous room before subscribing to new one
 * - Reconnect with exponential backoff (via useWebSocket)
 * - Identifies the connected participant via participantId
 */

import { useEffect, useRef } from 'react'
import { useWebSocket } from '@/hooks/useWebSocket'
import { serviceConfig } from '@/services/config'
import type { ChatMessage, Participant, WsRoomEvent } from '@/types/communicate'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseCommunicateSocketOptions {
  /** The room to subscribe to. Undefined = no subscription. */
  selectedRoomId: string | undefined
  /** The local participant identifier (used to filter echo). */
  participantId: string
  /** Called when a new chat message arrives. */
  onMessage: (msg: ChatMessage) => void
  /** Called when a participant joins the selected room. */
  onParticipantJoined?: (participant: Participant) => void
  /** Called when a participant leaves the selected room. */
  onParticipantLeft?: (roomId: string, identifier: string) => void
  /** Whether to suppress the connection entirely. */
  paused?: boolean
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

function communicateWsUrl(): string {
  return serviceConfig.communicateServiceUrl.replace(/^http/, 'ws') + '/ws'
}

export function useCommunicateSocket({
  selectedRoomId,
  participantId,
  onMessage,
  onParticipantJoined,
  onParticipantLeft,
  paused = false,
}: UseCommunicateSocketOptions) {
  const wsUrl = communicateWsUrl()
  const { messages, connectionState, send } = useWebSocket(wsUrl, { paused })

  // Track the last subscribed room to handle unsubscribe/re-subscribe
  const subscribedRoomRef = useRef<string | undefined>(undefined)

  // Subscribe / unsubscribe when room or connection state changes
  useEffect(() => {
    if (connectionState !== 'Connected') return

    // Unsubscribe from previous room
    if (subscribedRoomRef.current && subscribedRoomRef.current !== selectedRoomId) {
      send(JSON.stringify({ type: 'unsubscribe', room_id: subscribedRoomRef.current }))
      subscribedRoomRef.current = undefined
    }

    // Subscribe to new room
    if (selectedRoomId && subscribedRoomRef.current !== selectedRoomId) {
      send(JSON.stringify({ type: 'subscribe', room_id: selectedRoomId }))
      subscribedRoomRef.current = selectedRoomId
    }
  }, [connectionState, selectedRoomId, send])

  // Dispatch incoming events to handlers
  useEffect(() => {
    if (messages.length === 0) return

    const lastEvent = messages[messages.length - 1]
    let event: WsRoomEvent
    try {
      event = JSON.parse(lastEvent.data as string) as WsRoomEvent
    } catch {
      return
    }

    if (event.type === 'message') {
      // Filter echo — skip messages sent by this participant
      if (event.sender_id === participantId) return
      onMessage({
        id: event.id,
        room_id: event.room_id,
        sender_id: event.sender_id,
        sender_name: event.sender_name,
        sender_kind: event.sender_kind,
        content: event.content,
        metadata: event.metadata,
        reply_to: event.reply_to,
        status: event.status,
        created_at: event.created_at,
      })
    } else if (event.type === 'participant_joined') {
      onParticipantJoined?.({
        id: event.id,
        room_id: event.room_id,
        identifier: event.identifier,
        kind: event.kind,
        display_name: event.display_name,
        role: event.role,
        joined_at: event.joined_at,
      })
    } else if (event.type === 'participant_left') {
      onParticipantLeft?.(event.room_id, event.identifier)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messages])

  return { connectionState }
}
