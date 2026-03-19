/**
 * TypeScript types for the Communicate service.
 * Mirrors the Rust types in crates/communicate/src/types.rs.
 */

// ---------------------------------------------------------------------------
// Enums / union types
// ---------------------------------------------------------------------------

/** The kind of conversation a room represents. */
export type RoomType = 'direct' | 'group' | 'broadcast'

/** Whether a participant is an autonomous agent or a human user. */
export type ParticipantKind = 'agent' | 'human'

/** The role a participant holds within a room. */
export type ParticipantRole = 'member' | 'admin' | 'observer'

/** Delivery state of a message. */
export type MessageStatus = 'sent' | 'delivered' | 'read'

// ---------------------------------------------------------------------------
// Domain models
// ---------------------------------------------------------------------------

/** A conversation channel that groups participants and messages. */
export interface Room {
  id: string
  name: string
  topic: string | null
  description: string | null
  room_type: RoomType
  created_by: string
  created_at: string
  updated_at: string
  /** Client-side only: participant count (from participant list endpoint). */
  participant_count?: number
}

/** An agent or human who is a member of a room. */
export interface Participant {
  id: string
  room_id: string
  identifier: string
  kind: ParticipantKind
  display_name: string
  role: ParticipantRole
  joined_at: string
  /** Client-side only: activity state from orchestrator stream (agents only). */
  activity_state?: 'idle' | 'busy'
}

/** A message sent within a room. */
export interface ChatMessage {
  id: string
  room_id: string
  sender_id: string
  sender_name: string
  sender_kind: ParticipantKind
  content: string
  metadata: Record<string, string>
  reply_to: string | null
  status: MessageStatus
  created_at: string
}

// ---------------------------------------------------------------------------
// WebSocket event types (from communicate service)
// ---------------------------------------------------------------------------

export interface WsMessageEvent {
  type: 'message'
  id: string
  room_id: string
  sender_id: string
  sender_name: string
  sender_kind: ParticipantKind
  content: string
  metadata: Record<string, string>
  reply_to: string | null
  status: MessageStatus
  created_at: string
}

export interface WsParticipantJoinedEvent {
  type: 'participant_joined'
  id: string
  room_id: string
  identifier: string
  kind: ParticipantKind
  display_name: string
  role: ParticipantRole
  joined_at: string
}

export interface WsParticipantLeftEvent {
  type: 'participant_left'
  room_id: string
  identifier: string
}

export type WsRoomEvent = WsMessageEvent | WsParticipantJoinedEvent | WsParticipantLeftEvent

// ---------------------------------------------------------------------------
// WebSocket client messages
// ---------------------------------------------------------------------------

export interface WsSubscribeCommand {
  type: 'subscribe'
  room_id: string
}

export interface WsUnsubscribeCommand {
  type: 'unsubscribe'
  room_id: string
}

export type WsClientCommand = WsSubscribeCommand | WsUnsubscribeCommand

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

export interface CreateRoomRequest {
  name: string
  topic?: string
  description?: string
  room_type?: RoomType
  created_by: string
}

export interface UpdateRoomRequest {
  topic?: string
  description?: string
}

export interface AddParticipantRequest {
  identifier: string
  kind: ParticipantKind
  display_name: string
  role?: ParticipantRole
}

export interface CreateMessageRequest {
  sender_id: string
  sender_name: string
  sender_kind: ParticipantKind
  content: string
  metadata?: Record<string, string>
  reply_to?: string
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

export interface ListRoomsParams {
  limit?: number
  offset?: number
  room_type?: RoomType
}

export interface ListMessagesParams {
  limit?: number
  offset?: number
  before?: string
  after?: string
}
