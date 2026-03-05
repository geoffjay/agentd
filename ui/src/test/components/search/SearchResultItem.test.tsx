import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { SearchResultItem } from '@/components/search/SearchResultItem'
import type { SearchResult } from '@/hooks/useSearch'

const agentResult: SearchResult = {
  id: 'agent-1',
  category: 'agent',
  title: 'build-bot',
  subtitle: 'Status: Running',
  href: '/agents/1',
}

const notifResult: SearchResult = {
  id: 'notif-1',
  category: 'notification',
  title: 'High memory alert',
  subtitle: 'High — Pending',
  href: '/notifications/1',
}

const actionResult: SearchResult = {
  id: 'action-agents',
  category: 'action',
  title: 'Go to Agents',
  subtitle: 'Navigate to the agents page',
  href: '/agents',
}

describe('SearchResultItem', () => {
  it('renders the result title', () => {
    render(<SearchResultItem result={agentResult} isActive={false} onClick={vi.fn()} />)
    expect(screen.getByText('build-bot')).toBeInTheDocument()
  })

  it('renders the result subtitle', () => {
    render(<SearchResultItem result={agentResult} isActive={false} onClick={vi.fn()} />)
    expect(screen.getByText('Status: Running')).toBeInTheDocument()
  })

  it('shows Agent category badge for agent results', () => {
    render(<SearchResultItem result={agentResult} isActive={false} onClick={vi.fn()} />)
    expect(screen.getByText('Agent')).toBeInTheDocument()
  })

  it('shows Notification category badge for notification results', () => {
    render(<SearchResultItem result={notifResult} isActive={false} onClick={vi.fn()} />)
    expect(screen.getByText('Notification')).toBeInTheDocument()
  })

  it('shows Action category badge for action results', () => {
    render(<SearchResultItem result={actionResult} isActive={false} onClick={vi.fn()} />)
    expect(screen.getByText('Action')).toBeInTheDocument()
  })

  it('calls onClick when clicked', () => {
    const onClick = vi.fn()
    render(<SearchResultItem result={agentResult} isActive={false} onClick={onClick} />)
    fireEvent.click(screen.getByRole('button'))
    expect(onClick).toHaveBeenCalledWith(agentResult)
  })

  it('calls onClick on Enter key', () => {
    const onClick = vi.fn()
    render(<SearchResultItem result={agentResult} isActive={false} onClick={onClick} />)
    fireEvent.keyDown(screen.getByRole('button'), { key: 'Enter' })
    expect(onClick).toHaveBeenCalledWith(agentResult)
  })

  it('calls onClick on Space key', () => {
    const onClick = vi.fn()
    render(<SearchResultItem result={agentResult} isActive={false} onClick={onClick} />)
    fireEvent.keyDown(screen.getByRole('button'), { key: ' ' })
    expect(onClick).toHaveBeenCalledWith(agentResult)
  })

  it('sets aria-selected=true when active', () => {
    render(<SearchResultItem result={agentResult} isActive={true} onClick={vi.fn()} />)
    expect(screen.getByRole('option')).toHaveAttribute('aria-selected', 'true')
  })

  it('sets aria-selected=false when not active', () => {
    render(<SearchResultItem result={agentResult} isActive={false} onClick={vi.fn()} />)
    expect(screen.getByRole('option')).toHaveAttribute('aria-selected', 'false')
  })
})
