/**
 * Tests for MessageInput component.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MessageInput } from '@/components/communicate/MessageInput'

describe('MessageInput', () => {
  it('renders textarea and send button when participant', () => {
    render(
      <MessageInput
        onSend={vi.fn()}
        isParticipant={true}
        onJoin={vi.fn()}
      />,
    )
    expect(screen.getByRole('textbox', { name: /message input/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /send message/i })).toBeInTheDocument()
  })

  it('shows join prompt when not a participant', () => {
    render(
      <MessageInput
        onSend={vi.fn()}
        isParticipant={false}
        onJoin={vi.fn()}
      />,
    )
    expect(screen.getByText(/not a participant/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /join room/i })).toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  })

  it('calls onJoin when Join Room button is clicked', () => {
    const onJoin = vi.fn()
    render(
      <MessageInput
        onSend={vi.fn()}
        isParticipant={false}
        onJoin={onJoin}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /join room/i }))
    expect(onJoin).toHaveBeenCalledOnce()
  })

  it('shows joining state on join button', () => {
    render(
      <MessageInput
        onSend={vi.fn()}
        isParticipant={false}
        onJoin={vi.fn()}
        joiningRoom={true}
      />,
    )
    // When joiningRoom=true the button text becomes "Joining…" and is disabled
    const btn = screen.getByRole('button', { name: /joining/i })
    expect(btn).toBeDisabled()
    expect(btn).toHaveTextContent(/joining/i)
  })

  it('calls onSend with trimmed text when send button is clicked', async () => {
    const onSend = vi.fn().mockResolvedValue(undefined)
    render(
      <MessageInput
        onSend={onSend}
        isParticipant={true}
        onJoin={vi.fn()}
      />,
    )
    const textarea = screen.getByRole('textbox', { name: /message input/i })
    fireEvent.change(textarea, { target: { value: '  hello world  ' } })
    fireEvent.click(screen.getByRole('button', { name: /send message/i }))
    await waitFor(() => expect(onSend).toHaveBeenCalledWith('hello world'))
  })

  it('calls onSend when Enter is pressed without Shift', async () => {
    const onSend = vi.fn().mockResolvedValue(undefined)
    render(
      <MessageInput
        onSend={onSend}
        isParticipant={true}
        onJoin={vi.fn()}
      />,
    )
    const textarea = screen.getByRole('textbox', { name: /message input/i })
    fireEvent.change(textarea, { target: { value: 'press enter' } })
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false })
    await waitFor(() => expect(onSend).toHaveBeenCalledWith('press enter'))
  })

  it('does not send when Shift+Enter is pressed', () => {
    const onSend = vi.fn()
    render(
      <MessageInput
        onSend={onSend}
        isParticipant={true}
        onJoin={vi.fn()}
      />,
    )
    const textarea = screen.getByRole('textbox', { name: /message input/i })
    fireEvent.change(textarea, { target: { value: 'multiline' } })
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: true })
    expect(onSend).not.toHaveBeenCalled()
  })

  it('send button is disabled when textarea is empty', () => {
    render(
      <MessageInput
        onSend={vi.fn()}
        isParticipant={true}
        onJoin={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: /send message/i })).toBeDisabled()
  })

  it('clears textarea after successful send', async () => {
    const onSend = vi.fn().mockResolvedValue(undefined)
    render(
      <MessageInput
        onSend={onSend}
        isParticipant={true}
        onJoin={vi.fn()}
      />,
    )
    const textarea = screen.getByRole('textbox', { name: /message input/i }) as HTMLTextAreaElement
    fireEvent.change(textarea, { target: { value: 'hello' } })
    fireEvent.click(screen.getByRole('button', { name: /send message/i }))
    await waitFor(() => expect(textarea.value).toBe(''))
  })
})
