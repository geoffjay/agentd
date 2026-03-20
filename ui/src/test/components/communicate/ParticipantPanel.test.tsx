/**
 * Tests for ParticipantPanel component.
 */

import { render, screen, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { server } from '@/test/mocks/server'
import { ParticipantPanel } from '@/components/communicate/ParticipantPanel'
import { makeParticipantList } from '@/test/mocks/factories'

const ROOM_ID = 'room-abc-123'

describe('ParticipantPanel', () => {
  it('shows placeholder when no room is selected', () => {
    render(<ParticipantPanel roomId={undefined} />)

    expect(screen.getByText(/select a room/i)).toBeInTheDocument()
  })

  it('loads and renders participants', async () => {
    const agents = makeParticipantList(2, { room_id: ROOM_ID, kind: 'agent' })
    const humans = makeParticipantList(1, { room_id: ROOM_ID, kind: 'human' })
    const all = [...agents, ...humans]

    server.use(
      http.get(`http://localhost:17010/rooms/${ROOM_ID}/participants`, () =>
        HttpResponse.json({ items: all, total: 3, limit: 100, offset: 0 }),
      ),
    )

    render(<ParticipantPanel roomId={ROOM_ID} />)

    await waitFor(() => {
      expect(screen.getByText(/agents/i)).toBeInTheDocument()
      expect(screen.getByText(/humans/i)).toBeInTheDocument()
    })

    all.forEach((p) => {
      expect(screen.getByText(p.display_name)).toBeInTheDocument()
    })
  })

  it('shows error when fetch fails', async () => {
    server.use(
      http.get(`http://localhost:17010/rooms/${ROOM_ID}/participants`, () =>
        HttpResponse.json({ error: 'Not found' }, { status: 404 }),
      ),
    )

    render(<ParticipantPanel roomId={ROOM_ID} />)

    await waitFor(() => {
      expect(screen.getByText(/not found/i)).toBeInTheDocument()
    })
  })

  it('merges realtime joined participants', async () => {
    const existing = makeParticipantList(1, { room_id: ROOM_ID, kind: 'agent' })

    server.use(
      http.get(`http://localhost:17010/rooms/${ROOM_ID}/participants`, () =>
        HttpResponse.json({ items: existing, total: 1, limit: 100, offset: 0 }),
      ),
    )

    const newJoiner = makeParticipantList(1, {
      room_id: ROOM_ID,
      kind: 'human',
      display_name: 'Late Joiner',
    })[0]

    render(<ParticipantPanel roomId={ROOM_ID} realtimeParticipants={[newJoiner]} />)

    await waitFor(() => {
      expect(screen.getByText('Late Joiner')).toBeInTheDocument()
    })
  })

  it('hides participants who left', async () => {
    const participants = makeParticipantList(2, { room_id: ROOM_ID, kind: 'human' })

    server.use(
      http.get(`http://localhost:17010/rooms/${ROOM_ID}/participants`, () =>
        HttpResponse.json({ items: participants, total: 2, limit: 100, offset: 0 }),
      ),
    )

    render(
      <ParticipantPanel
        roomId={ROOM_ID}
        leftIdentifiers={[participants[0].identifier]}
      />,
    )

    await waitFor(() => {
      expect(screen.getByText(participants[1].display_name)).toBeInTheDocument()
    })

    expect(screen.queryByText(participants[0].display_name)).not.toBeInTheDocument()
  })
})
