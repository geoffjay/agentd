import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { ActivityTimeline } from '@/components/dashboard/ActivityTimeline'
import type { ActivityEvent } from '@/components/dashboard/ActivityTimeline'

const now = new Date()

const events: ActivityEvent[] = [
  {
    id: '1',
    type: 'agent',
    description: 'Agent "build-bot" is running',
    timestamp: new Date(now.getTime() - 2 * 60 * 1000), // 2 min ago
  },
  {
    id: '2',
    type: 'notification',
    description: 'High-priority alert received',
    timestamp: new Date(now.getTime() - 5 * 60 * 1000),
  },
  {
    id: '3',
    type: 'question',
    description: 'Question: Are tmux sessions running?',
    timestamp: new Date(now.getTime() - 10 * 60 * 1000),
  },
]

function renderTimeline(props: Partial<React.ComponentProps<typeof ActivityTimeline>> = {}) {
  return render(
    <MemoryRouter>
      <ActivityTimeline events={events} {...props} />
    </MemoryRouter>,
  )
}

describe('ActivityTimeline', () => {
  it('renders the section heading', () => {
    renderTimeline()
    expect(screen.getByRole('heading', { name: /recent activity/i })).toBeInTheDocument()
  })

  it('renders all event descriptions', () => {
    renderTimeline()
    expect(screen.getByText(/Agent "build-bot"/)).toBeInTheDocument()
    expect(screen.getByText(/High-priority alert/)).toBeInTheDocument()
    expect(screen.getByText(/Are tmux sessions/)).toBeInTheDocument()
  })

  it('renders a list with the correct number of items', () => {
    renderTimeline()
    const list = screen.getByRole('list', { name: /activity feed/i })
    expect(list.children).toHaveLength(3)
  })

  it('shows empty state when events array is empty', () => {
    renderTimeline({ events: [] })
    expect(screen.getByText(/no recent activity/i)).toBeInTheDocument()
  })

  it('shows loading skeleton when loading=true', () => {
    renderTimeline({ events: [], loading: true })
    expect(document.querySelector('[aria-busy="true"]')).toBeTruthy()
  })

  it('shows error message when error is provided', () => {
    renderTimeline({ events: [], error: 'Service unavailable' })
    expect(screen.getByText('Service unavailable')).toBeInTheDocument()
  })

  it('caps display at 10 events', () => {
    const manyEvents: ActivityEvent[] = Array.from({ length: 15 }, (_, i) => ({
      id: String(i),
      type: 'agent' as const,
      description: `Event ${i}`,
      timestamp: new Date(),
    }))
    renderTimeline({ events: manyEvents })
    const list = screen.getByRole('list', { name: /activity feed/i })
    expect(list.children).toHaveLength(10)
  })

  it('renders a "View All" link', () => {
    renderTimeline()
    expect(screen.getByRole('link', { name: /view all/i })).toBeInTheDocument()
  })
})
