import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AgentLogView } from '@/components/agents/AgentLogView'
import type { LogLine } from '@/hooks/useAgentStream'

function makeLine(id: number, text: string): LogLine {
  return { id, text, timestamp: new Date().toISOString() }
}

describe('AgentLogView', () => {
  it('renders the log container', () => {
    render(<AgentLogView lines={[]} status="connected" onClear={vi.fn()} />)
    // The outer container is a <div> with aria-label (not a <section>), so use getByLabelText
    expect(screen.getByLabelText(/agent log output/i)).toBeInTheDocument()
  })

  it('shows connecting placeholder when status is connecting', () => {
    render(<AgentLogView lines={[]} status="connecting" onClear={vi.fn()} />)
    expect(screen.getByText(/connecting to agent stream/i)).toBeInTheDocument()
  })

  it('shows connected status badge', () => {
    render(<AgentLogView lines={[]} status="connected" onClear={vi.fn()} />)
    expect(screen.getByLabelText(/stream connected/i)).toBeInTheDocument()
  })

  it('shows disconnected status badge', () => {
    render(<AgentLogView lines={[]} status="disconnected" onClear={vi.fn()} />)
    expect(screen.getByLabelText(/stream disconnected/i)).toBeInTheDocument()
  })

  it('renders log lines', () => {
    const lines = [makeLine(1, 'First line'), makeLine(2, 'Second line')]
    render(<AgentLogView lines={lines} status="connected" onClear={vi.fn()} />)
    expect(screen.getByText('First line')).toBeInTheDocument()
    expect(screen.getByText('Second line')).toBeInTheDocument()
  })

  it('strips ANSI escape codes from log lines', () => {
    const lines = [makeLine(1, '\x1b[32mgreen text\x1b[0m')]
    render(<AgentLogView lines={lines} status="connected" onClear={vi.fn()} />)
    expect(screen.getByText('green text')).toBeInTheDocument()
  })

  it('calls onClear when Clear button is clicked', () => {
    const onClear = vi.fn()
    render(<AgentLogView lines={[makeLine(1, 'test')]} status="connected" onClear={onClear} />)
    fireEvent.click(screen.getByRole('button', { name: /clear log/i }))
    expect(onClear).toHaveBeenCalledOnce()
  })
})
