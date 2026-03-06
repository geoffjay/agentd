import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { AgentStatusBadge } from '@/components/agents/AgentStatusBadge'

describe('AgentStatusBadge', () => {
  it('renders Running status', () => {
    render(<AgentStatusBadge status="Running" />)
    expect(screen.getByText(/running/i)).toBeInTheDocument()
  })

  it('renders Pending status', () => {
    render(<AgentStatusBadge status="Pending" />)
    expect(screen.getByText(/pending/i)).toBeInTheDocument()
  })

  it('renders Stopped status', () => {
    render(<AgentStatusBadge status="Stopped" />)
    expect(screen.getByText(/stopped/i)).toBeInTheDocument()
  })

  it('renders Failed status', () => {
    render(<AgentStatusBadge status="Failed" />)
    expect(screen.getByText(/failed/i)).toBeInTheDocument()
  })

  it('defaults to badge variant', () => {
    const { container } = render(<AgentStatusBadge status="Running" />)
    // Badge variant renders a span with text; dot variant would render a circle without text
    expect(screen.getByText(/running/i)).toBeInTheDocument()
    expect(container.firstChild).toBeInTheDocument()
  })
})
