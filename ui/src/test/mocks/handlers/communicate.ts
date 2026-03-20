/**
 * MSW request handlers for the Communicate service (port 17010).
 *
 * Provides default responses for all communicate API endpoints.
 * Override per test with server.use().
 */

import { http, HttpResponse } from 'msw'
import {
  makeRoomList,
  makeParticipantList,
  makeChatMessageList,
  makeRoom,
  makeParticipant,
  makeChatMessage,
} from '../factories'
import type { PaginatedResponse } from '@/types/common'
import type { ChatMessage, Participant, Room } from '@/types/communicate'

const BASE = 'http://localhost:17010'

function paginated<T>(items: T[], total?: number): PaginatedResponse<T> {
  return { items, total: total ?? items.length, limit: 50, offset: 0 }
}

const DEFAULT_ROOMS = makeRoomList(3)
const DEFAULT_PARTICIPANTS = makeParticipantList(4)
const DEFAULT_MESSAGES = makeChatMessageList(5)

export const communicateHandlers = [
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  http.get(`${BASE}/health`, () =>
    HttpResponse.json({ status: 'ok', service: 'communicate', version: '0.1.0' }),
  ),

  // -------------------------------------------------------------------------
  // Rooms
  // -------------------------------------------------------------------------

  http.get(`${BASE}/rooms`, () =>
    HttpResponse.json(paginated<Room>(DEFAULT_ROOMS)),
  ),

  http.post(`${BASE}/rooms`, async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>
    const room = makeRoom({
      name: String(body.name ?? 'new-room'),
      room_type: (body.room_type as Room['room_type']) ?? 'group',
      created_by: String(body.created_by ?? 'human-ui'),
    })
    return HttpResponse.json(room, { status: 201 })
  }),

  http.get(`${BASE}/rooms/:id`, ({ params }) => {
    const room =
      DEFAULT_ROOMS.find((r) => r.id === params.id) ??
      makeRoom({ id: String(params.id) })
    return HttpResponse.json(room)
  }),

  http.put(`${BASE}/rooms/:id`, async ({ params, request }) => {
    const body = (await request.json()) as Record<string, unknown>
    const existing =
      DEFAULT_ROOMS.find((r) => r.id === params.id) ??
      makeRoom({ id: String(params.id) })
    return HttpResponse.json({
      ...existing,
      topic: (body.topic as string) ?? existing.topic,
      description: (body.description as string) ?? existing.description,
    })
  }),

  http.delete(`${BASE}/rooms/:id`, () => new HttpResponse(null, { status: 204 })),

  // -------------------------------------------------------------------------
  // Participants
  // -------------------------------------------------------------------------

  http.get(`${BASE}/rooms/:id/participants`, () =>
    HttpResponse.json(paginated<Participant>(DEFAULT_PARTICIPANTS)),
  ),

  http.post(`${BASE}/rooms/:id/participants`, async ({ params, request }) => {
    const body = (await request.json()) as Record<string, unknown>
    const participant = makeParticipant({
      room_id: String(params.id),
      identifier: String(body.identifier ?? 'participant-1'),
      kind: (body.kind as Participant['kind']) ?? 'human',
      display_name: String(body.display_name ?? 'New Participant'),
      role: (body.role as Participant['role']) ?? 'member',
    })
    return HttpResponse.json(participant, { status: 201 })
  }),

  http.delete(`${BASE}/rooms/:id/participants/:identifier`, () =>
    new HttpResponse(null, { status: 204 }),
  ),

  http.get(`${BASE}/participants/:identifier/rooms`, () =>
    HttpResponse.json(paginated<Room>(DEFAULT_ROOMS.slice(0, 2))),
  ),

  // -------------------------------------------------------------------------
  // Messages
  // -------------------------------------------------------------------------

  http.get(`${BASE}/rooms/:id/messages/latest`, () =>
    HttpResponse.json(paginated<ChatMessage>(DEFAULT_MESSAGES)),
  ),

  http.get(`${BASE}/rooms/:id/messages`, () =>
    HttpResponse.json(paginated<ChatMessage>(DEFAULT_MESSAGES)),
  ),

  http.post(`${BASE}/rooms/:id/messages`, async ({ params, request }) => {
    const body = (await request.json()) as Record<string, unknown>
    const message = makeChatMessage({
      room_id: String(params.id),
      sender_id: String(body.sender_id ?? 'human-ui'),
      sender_name: String(body.sender_name ?? 'UI User'),
      sender_kind: (body.sender_kind as ChatMessage['sender_kind']) ?? 'human',
      content: String(body.content ?? ''),
    })
    return HttpResponse.json(message, { status: 201 })
  }),

  http.delete(`${BASE}/messages/:id`, () => new HttpResponse(null, { status: 204 })),
]
