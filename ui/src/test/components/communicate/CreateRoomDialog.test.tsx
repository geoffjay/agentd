/**
 * Tests for CreateRoomDialog component.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { server } from '@/test/mocks/server'
import { CreateRoomDialog } from '@/components/communicate/CreateRoomDialog'
import { makeRoom } from '@/test/mocks/factories'

const BASE = 'http://localhost:17010'

describe('CreateRoomDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = render(
      <CreateRoomDialog
        open={false}
        createdBy="human-alice"
        onCreated={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('renders the dialog when open', () => {
    render(
      <CreateRoomDialog
        open={true}
        createdBy="human-alice"
        onCreated={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: /create room/i })).toBeInTheDocument()
  })

  it('shows name, topic, and description inputs', () => {
    render(
      <CreateRoomDialog
        open={true}
        createdBy="human-alice"
        onCreated={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByPlaceholderText(/e\.g\. general/i)).toBeInTheDocument()
    expect(screen.getByPlaceholderText(/project discussions/i)).toBeInTheDocument()
    expect(screen.getByPlaceholderText(/what is this room for/i)).toBeInTheDocument()
  })

  it('shows validation error when name is empty', async () => {
    render(
      <CreateRoomDialog
        open={true}
        createdBy="human-alice"
        onCreated={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /create room/i }))
    expect(await screen.findByText(/room name is required/i)).toBeInTheDocument()
  })

  it('creates room and calls onCreated on success', async () => {
    const created = makeRoom({ name: 'test-room', room_type: 'group' })
    server.use(
      http.post(`${BASE}/rooms`, () => HttpResponse.json(created, { status: 201 })),
    )
    const onCreated = vi.fn()
    render(
      <CreateRoomDialog
        open={true}
        createdBy="human-alice"
        onCreated={onCreated}
        onClose={vi.fn()}
      />,
    )

    fireEvent.change(screen.getByPlaceholderText(/e\.g\. general/i), {
      target: { value: 'test-room' },
    })
    fireEvent.click(screen.getByRole('button', { name: /create room/i }))

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(created))
  })

  it('shows error message on API failure', async () => {
    server.use(
      http.post(`${BASE}/rooms`, () =>
        HttpResponse.json({ error: 'Name already taken' }, { status: 409 }),
      ),
    )
    render(
      <CreateRoomDialog
        open={true}
        createdBy="human-alice"
        onCreated={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    fireEvent.change(screen.getByPlaceholderText(/e\.g\. general/i), {
      target: { value: 'dup-room' },
    })
    fireEvent.click(screen.getByRole('button', { name: /create room/i }))

    // mapApiError maps 409 → "Conflict — resource already exists"
    await waitFor(() =>
      expect(screen.getByText(/conflict/i)).toBeInTheDocument(),
    )
  })

  it('calls onClose when cancel is clicked', () => {
    const onClose = vi.fn()
    render(
      <CreateRoomDialog
        open={true}
        createdBy="human-alice"
        onCreated={vi.fn()}
        onClose={onClose}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('allows selecting room type', () => {
    render(
      <CreateRoomDialog
        open={true}
        createdBy="human-alice"
        onCreated={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    // Click "Direct" type button
    fireEvent.click(screen.getByText('Direct'))
    // The button should have the selected style
    const directBtn = screen.getByText('Direct').closest('button')
    expect(directBtn).toHaveClass('border-primary-500')
  })
})
