/**
 * Tests for RoomSettingsPanel component.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { server } from '@/test/mocks/server'
import { RoomSettingsPanel } from '@/components/communicate/RoomSettingsPanel'
import { makeRoom, makeParticipantList } from '@/test/mocks/factories'

const BASE = 'http://localhost:17010'
const LOCAL_ID = 'human-alice'

function renderPanel(overrides?: Partial<Parameters<typeof RoomSettingsPanel>[0]>) {
  const room = makeRoom({ name: 'test-room', topic: 'Test topic', description: 'A description' })
  return render(
    <RoomSettingsPanel
      room={room}
      localIdentifier={LOCAL_ID}
      onClose={vi.fn()}
      onRoomDeleted={vi.fn()}
      onLeft={vi.fn()}
      onRoomUpdated={vi.fn()}
      {...overrides}
    />,
  )
}

describe('RoomSettingsPanel', () => {
  it('renders room info section', () => {
    renderPanel()
    expect(screen.getByText('Room Settings')).toBeInTheDocument()
    expect(screen.getByText('Info')).toBeInTheDocument()
  })

  it('pre-fills topic and description from room', () => {
    renderPanel()
    expect(screen.getByDisplayValue('Test topic')).toBeInTheDocument()
    expect(screen.getByDisplayValue('A description')).toBeInTheDocument()
  })

  it('calls onClose when close button is clicked', () => {
    const onClose = vi.fn()
    renderPanel({ onClose })
    fireEvent.click(screen.getByRole('button', { name: /close settings/i }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('calls onRoomUpdated after saving topic and description', async () => {
    const updated = makeRoom({ name: 'test-room', topic: 'New topic' })
    server.use(
      http.put(`${BASE}/rooms/:id`, () => HttpResponse.json(updated)),
    )
    const onRoomUpdated = vi.fn()
    renderPanel({ onRoomUpdated })

    const topicInput = screen.getByDisplayValue('Test topic')
    fireEvent.change(topicInput, { target: { value: 'New topic' } })
    fireEvent.click(screen.getByRole('button', { name: /save changes/i }))

    await waitFor(() => expect(onRoomUpdated).toHaveBeenCalledWith(updated))
  })

  it('loads and displays participants', async () => {
    const participants = makeParticipantList(2)
    server.use(
      http.get(`${BASE}/rooms/:id/participants`, () =>
        HttpResponse.json({ items: participants, total: 2, limit: 100, offset: 0 }),
      ),
    )
    renderPanel()
    await waitFor(() =>
      expect(screen.getByText(participants[0].display_name)).toBeInTheDocument(),
    )
    expect(screen.getByText(participants[1].display_name)).toBeInTheDocument()
  })

  it('shows delete room button in danger zone', () => {
    renderPanel()
    expect(screen.getByRole('button', { name: /delete room/i })).toBeInTheDocument()
  })

  it('opens confirm dialog when delete room is clicked', async () => {
    renderPanel()
    fireEvent.click(screen.getByRole('button', { name: /delete room/i }))
    await waitFor(() =>
      expect(screen.getByText(/permanently delete/i)).toBeInTheDocument(),
    )
  })

  it('calls onRoomDeleted after confirming delete', async () => {
    server.use(
      http.delete(`${BASE}/rooms/:id`, () => new HttpResponse(null, { status: 204 })),
    )
    const onRoomDeleted = vi.fn()
    renderPanel({ onRoomDeleted })

    // Open the confirm dialog
    fireEvent.click(screen.getByRole('button', { name: /delete room/i }))
    // Wait for the alertdialog to appear
    const dialog = await screen.findByRole('alertdialog')
    // Click the confirm button inside the alertdialog
    fireEvent.click(
      Array.from(dialog.querySelectorAll('button')).find((b) =>
        /delete room/i.test(b.textContent ?? ''),
      )!,
    )

    await waitFor(() => expect(onRoomDeleted).toHaveBeenCalledOnce())
  })
})
