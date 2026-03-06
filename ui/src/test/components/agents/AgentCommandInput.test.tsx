import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { AgentCommandInput } from '@/components/agents/AgentCommandInput'

describe('AgentCommandInput', () => {
  const defaultProps = {
    agentId: 'agent-1',
    enabled: true,
    onSend: vi.fn().mockResolvedValue(undefined),
  }

  it('renders the input field', () => {
    render(<AgentCommandInput {...defaultProps} />)
    expect(screen.getByRole('textbox', { name: /send message/i })).toBeInTheDocument()
  })

  it('disables input when enabled=false', () => {
    render(
      <AgentCommandInput
        {...defaultProps}
        enabled={false}
        disabledReason="Agent is not running"
      />,
    )
    expect(screen.getByRole('textbox', { name: /send message/i })).toBeDisabled()
  })

  it('calls onSend when Enter is pressed', async () => {
    const onSend = vi.fn().mockResolvedValue(undefined)
    render(<AgentCommandInput {...defaultProps} onSend={onSend} />)

    const input = screen.getByRole('textbox', { name: /send message/i })
    fireEvent.change(input, { target: { value: 'hello world' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    await waitFor(() => expect(onSend).toHaveBeenCalledWith('hello world'))
  })

  it('calls onSend when Send button is clicked', async () => {
    const onSend = vi.fn().mockResolvedValue(undefined)
    render(<AgentCommandInput {...defaultProps} onSend={onSend} />)

    fireEvent.change(screen.getByRole('textbox', { name: /send message/i }), {
      target: { value: 'hello' },
    })
    fireEvent.click(screen.getByRole('button', { name: /send message/i }))

    await waitFor(() => expect(onSend).toHaveBeenCalledWith('hello'))
  })

  it('clears the input after sending', async () => {
    const onSend = vi.fn().mockResolvedValue(undefined)
    render(<AgentCommandInput {...defaultProps} onSend={onSend} />)

    const input = screen.getByRole('textbox', { name: /send message/i })
    fireEvent.change(input, { target: { value: 'test message' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    await waitFor(() => expect(input).toHaveValue(''))
  })

  it('does not send empty or whitespace-only messages', async () => {
    const onSend = vi.fn().mockResolvedValue(undefined)
    render(<AgentCommandInput {...defaultProps} onSend={onSend} />)

    const input = screen.getByRole('textbox', { name: /send message/i })
    fireEvent.change(input, { target: { value: '   ' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    await new Promise(r => setTimeout(r, 50))
    expect(onSend).not.toHaveBeenCalled()
  })

  it('shows error message when send fails', async () => {
    const onSend = vi.fn().mockRejectedValue(new Error('Network error'))
    render(<AgentCommandInput {...defaultProps} onSend={onSend} />)

    fireEvent.change(screen.getByRole('textbox', { name: /send message/i }), {
      target: { value: 'test' },
    })
    fireEvent.keyDown(screen.getByRole('textbox', { name: /send message/i }), {
      key: 'Enter',
    })

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('Network error'),
    )
  })
})
