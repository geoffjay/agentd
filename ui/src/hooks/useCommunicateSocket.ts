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

import { useEffect, useRef, useCallback } from 'react'
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

function communicateWsUrl(participantId: string, displayName: string): string {
  const base = serviceConfig.communicateServiceUrl.replace(/^http/, 'ws')
  const params = new URLSearchParams({
    identifier: participantId,
    kind: 'human',
    display_name: displayName,
  })
  return `${base}/ws?${params.toString()}`
}

export function useCommunicateSocket({
  selectedRoomId,
  participantId,
  onMessage,
  onParticipantJoined,
  onParticipantLeft,
  paused = false,
}: UseCommunicateSocketOptions) {
  // TODO: accept displayName from caller when user profiles are available
  const wsUrl = communicateWsUrl(participantId, participantId)
  const { messages, connectionState, send } = useWebSocket(wsUrl, { paused })

  // Track the last subscribed room to handle unsubscribe/re-subscribe
  const subscribedRoomRef = useRef<string | undefined>(undefined)
  // Track how many messages have been processed so we handle every new message,
  // not just the last one (important after reconnect bursts).
  const processedCountRef = useRef(0)

  // Stable refs for the caller-provided callbacks so they never appear in
  // dependency arrays. Callers MUST wrap these in useCallback (or equivalent
  // stable references) — if they don't, the ref will always hold the latest
  // version, which is correct, but stale closures in the caller will not be
  // re-evaluated here.
  const onMessageRef = useRef(onMessage)
  onMessageRef.current = onMessage
  const onParticipantJoinedRef = useRef(onParticipantJoined)
  onParticipantJoinedRef.current = onParticipantJoined
  const onParticipantLeftRef = useRef(onParticipantLeft)
  onParticipantLeftRef.current = onParticipantLeft
  const participantIdRef = useRef(participantId)
  participantIdRef.current = participantId

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

  // Dispatch handler — extracted so it can reference the stable callback refs
  const dispatchEvent = useCallback((raw: MessageEvent) => {
    let event: WsRoomEvent
    try {
      event = JSON.parse(raw.data as string) as WsRoomEvent
    } catch {
      return
    }

    if (event.type === 'message') {
      if (event.sender_id === participantIdRef.current) return
      onMessageRef.current({
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
      onParticipantJoinedRef.current?.({
        id: event.id,
        room_id: event.room_id,
        identifier: event.identifier,
        kind: event.kind,
        display_name: event.display_name,
        role: event.role,
        joined_at: event.joined_at,
      })
    } else if (event.type === 'participant_left') {
      onParticipantLeftRef.current?.(event.room_id, event.identifier)
    }
  }, [])

  // Process all messages added since the last render to avoid dropping events
  // that arrive in the same state flush (e.g. after a reconnect burst).
  useEffect(() => {
    const newMessages = messages.slice(processedCountRef.current)
    if (newMessages.length === 0) return
    newMessages.forEach(dispatchEvent)
    processedCountRef.current = messages.length
  }, [messages, dispatchEvent])

  return { connectionState }
}
