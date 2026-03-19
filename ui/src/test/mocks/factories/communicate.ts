/**
 * Test factories for communicate service types.
 */

import type {
  ChatMessage,
  Participant,
  ParticipantKind,
  ParticipantRole,
  Room,
  RoomType,
} from '@/types/communicate'

let _seq = 0
function seq(): number {
  return ++_seq
}

function uuid(): string {
  return `xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0
    return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16)
  })
}

function isoNow(offsetMs = 0): string {
  return new Date(Date.now() + offsetMs).toISOString()
}

// ---------------------------------------------------------------------------
// Room factory
// ---------------------------------------------------------------------------

export function makeRoom(overrides: Partial<Room> = {}): Room {
  const n = seq()
  return {
    id: uuid(),
    name: `room-${n}`,
    topic: `Topic for room ${n}`,
    description: null,
    room_type: 'group' as RoomType,
    created_by: 'agent-system',
    created_at: isoNow(-n * 60_000),
    updated_at: isoNow(-n * 1_000),
    ...overrides,
  }
}

export function makeRoomList(count = 3, overrides: Partial<Room> = {}): Room[] {
  return Array.from({ length: count }, () => makeRoom(overrides))
}

// ---------------------------------------------------------------------------
// Participant factory
// ---------------------------------------------------------------------------

export function makeParticipant(overrides: Partial<Participant> = {}): Participant {
  const n = seq()
  const kind: ParticipantKind = n % 2 === 0 ? 'agent' : 'human'
  return {
    id: uuid(),
    room_id: uuid(),
    identifier: `${kind}-${n}`,
    kind,
    display_name: `${kind === 'agent' ? 'Agent' : 'Human'} ${n}`,
    role: 'member' as ParticipantRole,
    joined_at: isoNow(-n * 5_000),
    ...overrides,
  }
}

export function makeParticipantList(count = 3, overrides: Partial<Participant> = {}): Participant[] {
  return Array.from({ length: count }, () => makeParticipant(overrides))
}

// ---------------------------------------------------------------------------
// ChatMessage factory
// ---------------------------------------------------------------------------

export function makeChatMessage(overrides: Partial<ChatMessage> = {}): ChatMessage {
  const n = seq()
  const kind: ParticipantKind = n % 2 === 0 ? 'agent' : 'human'
  return {
    id: uuid(),
    room_id: uuid(),
    sender_id: `${kind}-${n}`,
    sender_name: `${kind === 'agent' ? 'Agent' : 'Human'} ${n}`,
    sender_kind: kind,
    content: `Message number ${n}`,
    metadata: {},
    reply_to: null,
    status: 'sent',
    created_at: isoNow(-n * 2_000),
    ...overrides,
  }
}

export function makeChatMessageList(count = 5, overrides: Partial<ChatMessage> = {}): ChatMessage[] {
  return Array.from({ length: count }, () => makeChatMessage(overrides))
}
