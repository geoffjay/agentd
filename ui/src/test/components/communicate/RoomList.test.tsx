/**
 * Tests for RoomList component.
 */

import { render, screen, fireEvent } from '@testing-library/react'
import { RoomList } from '@/components/communicate/RoomList'
import { makeRoomList } from '@/test/mocks/factories'

describe('RoomList', () => {
  const rooms = makeRoomList(4)

  it('renders a list of rooms', () => {
    render(
      <RoomList
        rooms={rooms}
        selectedRoomId={undefined}
        loading={false}
        onSelectRoom={vi.fn()}
      />,
    )

    rooms.forEach((room) => {
      expect(screen.getByText(room.name)).toBeInTheDocument()
    })
  })

  it('highlights the selected room', () => {
    const selected = rooms[1]
    render(
      <RoomList
        rooms={rooms}
        selectedRoomId={selected.id}
        loading={false}
        onSelectRoom={vi.fn()}
      />,
    )

    const btn = screen.getByText(selected.name).closest('button')
    expect(btn).toHaveClass('bg-primary-700')
  })

  it('calls onSelectRoom when a room is clicked', () => {
    const onSelect = vi.fn()
    render(
      <RoomList
        rooms={rooms}
        selectedRoomId={undefined}
        loading={false}
        onSelectRoom={onSelect}
      />,
    )

    fireEvent.click(screen.getByText(rooms[0].name))
    expect(onSelect).toHaveBeenCalledWith(rooms[0])
  })

  it('filters rooms by search input', () => {
    const named = [
      { ...rooms[0], name: 'alpha-ops' },
      { ...rooms[1], name: 'beta-chat' },
      { ...rooms[2], name: 'gamma-ops' },
    ]
    render(
      <RoomList
        rooms={named}
        selectedRoomId={undefined}
        loading={false}
        onSelectRoom={vi.fn()}
      />,
    )

    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'ops' } })

    expect(screen.getByText('alpha-ops')).toBeInTheDocument()
    expect(screen.getByText('gamma-ops')).toBeInTheDocument()
    expect(screen.queryByText('beta-chat')).not.toBeInTheDocument()
  })

  it('shows skeleton when loading', () => {
    render(
      <RoomList
        rooms={[]}
        selectedRoomId={undefined}
        loading={true}
        onSelectRoom={vi.fn()}
      />,
    )

    // No room names rendered, skeletons are aria-hidden
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('shows empty state when no rooms match search', () => {
    render(
      <RoomList
        rooms={rooms}
        selectedRoomId={undefined}
        loading={false}
        onSelectRoom={vi.fn()}
      />,
    )

    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'zzz-no-match' } })

    expect(screen.getByText(/no rooms match/i)).toBeInTheDocument()
  })

  it('shows topic when present', () => {
    const withTopic = [{ ...rooms[0], topic: 'Important discussions' }]
    render(
      <RoomList
        rooms={withTopic}
        selectedRoomId={undefined}
        loading={false}
        onSelectRoom={vi.fn()}
      />,
    )

    expect(screen.getByText('Important discussions')).toBeInTheDocument()
  })
})
