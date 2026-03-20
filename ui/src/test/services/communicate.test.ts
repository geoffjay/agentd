/**
 * Tests for CommunicateClient service.
 */

import { http, HttpResponse } from 'msw'
import { server } from '@/test/mocks/server'
import { communicateClient } from '@/services/communicate'
import { makeRoom, makeRoomList, makeParticipantList, makeChatMessageList } from '@/test/mocks/factories'

const BASE = 'http://localhost:17010'

function paginated<T>(items: T[]) {
  return { items, total: items.length, limit: 50, offset: 0 }
}

describe('CommunicateClient', () => {
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  it('getHealth returns health response', async () => {
    const result = await communicateClient.getHealth()
    expect(result.status).toBe('ok')
    expect(result.service).toBe('communicate')
  })

  // -------------------------------------------------------------------------
  // Rooms
  // -------------------------------------------------------------------------

  it('listRooms returns paginated rooms', async () => {
    const rooms = makeRoomList(3)
    server.use(
      http.get(`${BASE}/rooms`, () => HttpResponse.json(paginated(rooms))),
    )

    const result = await communicateClient.listRooms()
    expect(result.items).toHaveLength(3)
    expect(result.total).toBe(3)
  })

  it('getRoom returns a single room', async () => {
    const room = makeRoom({ name: 'test-room' })
    server.use(
      http.get(`${BASE}/rooms/${room.id}`, () => HttpResponse.json(room)),
    )

    const result = await communicateClient.getRoom(room.id)
    expect(result.name).toBe('test-room')
  })

  it('createRoom posts and returns created room', async () => {
    const room = makeRoom({ name: 'new-room' })
    server.use(
      http.post(`${BASE}/rooms`, () => HttpResponse.json(room, { status: 201 })),
    )

    const result = await communicateClient.createRoom({
      name: 'new-room',
      created_by: 'human-ui',
    })
    expect(result.name).toBe('new-room')
  })

  it('updateRoom puts and returns updated room', async () => {
    const room = makeRoom({ topic: 'New topic' })
    server.use(
      http.put(`${BASE}/rooms/${room.id}`, () => HttpResponse.json(room)),
    )

    const result = await communicateClient.updateRoom(room.id, { topic: 'New topic' })
    expect(result.topic).toBe('New topic')
  })

  it('deleteRoom sends DELETE request', async () => {
    let deleted = false
    server.use(
      http.delete(`${BASE}/rooms/room-1`, () => {
        deleted = true
        return new HttpResponse(null, { status: 204 })
      }),
    )

    await communicateClient.deleteRoom('room-1')
    expect(deleted).toBe(true)
  })

  // -------------------------------------------------------------------------
  // Participants
  // -------------------------------------------------------------------------

  it('listParticipants returns paginated participants', async () => {
    const participants = makeParticipantList(4)
    server.use(
      http.get(`${BASE}/rooms/room-1/participants`, () =>
        HttpResponse.json(paginated(participants)),
      ),
    )

    const result = await communicateClient.listParticipants('room-1')
    expect(result.items).toHaveLength(4)
  })

  it('addParticipant posts and returns participant', async () => {
    const participants = makeParticipantList(1, { kind: 'human', display_name: 'Test Human' })
    server.use(
      http.post(`${BASE}/rooms/room-1/participants`, () =>
        HttpResponse.json(participants[0], { status: 201 }),
      ),
    )

    const result = await communicateClient.addParticipant('room-1', {
      identifier: 'human-test',
      kind: 'human',
      display_name: 'Test Human',
    })
    expect(result.display_name).toBe('Test Human')
  })

  // -------------------------------------------------------------------------
  // Messages
  // -------------------------------------------------------------------------

  it('getLatestMessages returns paginated messages', async () => {
    const messages = makeChatMessageList(5)
    server.use(
      http.get(`${BASE}/rooms/room-1/messages/latest`, () =>
        HttpResponse.json(paginated(messages)),
      ),
    )

    const result = await communicateClient.getLatestMessages('room-1')
    expect(result.items).toHaveLength(5)
  })

  it('listMessages returns paginated messages with params', async () => {
    const messages = makeChatMessageList(3)
    server.use(
      http.get(`${BASE}/rooms/room-1/messages`, () =>
        HttpResponse.json(paginated(messages)),
      ),
    )

    const result = await communicateClient.listMessages('room-1', { limit: 3 })
    expect(result.items).toHaveLength(3)
  })

  it('sendMessage posts and returns message', async () => {
    const message = makeChatMessageList(1, { content: 'Hello world' })[0]
    server.use(
      http.post(`${BASE}/rooms/room-1/messages`, () =>
        HttpResponse.json(message, { status: 201 }),
      ),
    )

    const result = await communicateClient.sendMessage('room-1', {
      sender_id: 'human-ui',
      sender_name: 'UI User',
      sender_kind: 'human',
      content: 'Hello world',
    })
    expect(result.content).toBe('Hello world')
  })
})
