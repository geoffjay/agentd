import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { SearchResults, RecentSearches } from '@/components/search/SearchResults'
import type { GroupedSearchResults } from '@/hooks/useSearch'

const emptyResults: GroupedSearchResults = {
  actions: [],
  agents: [],
  notifications: [],
  total: 0,
}

const richResults: GroupedSearchResults = {
  actions: [
    {
      id: 'action-agents',
      category: 'action',
      title: 'Go to Agents',
      subtitle: 'Navigate to agents',
      href: '/agents',
    },
  ],
  agents: [
    {
      id: 'agent-1',
      category: 'agent',
      title: 'build-bot',
      subtitle: 'Status: Running',
      href: '/agents/1',
    },
    {
      id: 'agent-2',
      category: 'agent',
      title: 'deploy-bot',
      subtitle: 'Status: Pending',
      href: '/agents/2',
    },
  ],
  notifications: [
    {
      id: 'notif-1',
      category: 'notification',
      title: 'Alert',
      subtitle: 'High — Pending',
      href: '/notifications/1',
    },
  ],
  total: 4,
}

describe('SearchResults', () => {
  it('shows no-results state when results are empty', () => {
    render(
      <SearchResults
        query="foobar"
        results={emptyResults}
        loading={false}
        activeId={null}
        onSelect={vi.fn()}
      />,
    )
    expect(screen.getByText(/no results for/i)).toBeInTheDocument()
    expect(screen.getByText(/"foobar"/)).toBeInTheDocument()
  })

  it('shows a loading spinner while loading', () => {
    render(
      <SearchResults
        query="agent"
        results={emptyResults}
        loading={true}
        activeId={null}
        onSelect={vi.fn()}
      />,
    )
    expect(screen.getByRole('status', { name: /searching/i })).toBeInTheDocument()
  })

  it('renders section headings for each category', () => {
    render(
      <SearchResults
        query="bot"
        results={richResults}
        loading={false}
        activeId={null}
        onSelect={vi.fn()}
      />,
    )
    // Use role="group" with aria-label to target section headings specifically
    expect(screen.getByRole('group', { name: /quick actions/i })).toBeInTheDocument()
    expect(screen.getByRole('group', { name: /agents/i })).toBeInTheDocument()
    expect(screen.getByRole('group', { name: /notifications/i })).toBeInTheDocument()
  })

  it('renders all result items', () => {
    render(
      <SearchResults
        query="bot"
        results={richResults}
        loading={false}
        activeId={null}
        onSelect={vi.fn()}
      />,
    )
    expect(screen.getByText('Go to Agents')).toBeInTheDocument()
    expect(screen.getByText('build-bot')).toBeInTheDocument()
    expect(screen.getByText('deploy-bot')).toBeInTheDocument()
    expect(screen.getByText('Alert')).toBeInTheDocument()
  })

  it('calls onSelect when a result is clicked', () => {
    const onSelect = vi.fn()
    render(
      <SearchResults
        query="bot"
        results={richResults}
        loading={false}
        activeId={null}
        onSelect={onSelect}
      />,
    )
    fireEvent.click(screen.getAllByRole('button')[0])
    expect(onSelect).toHaveBeenCalled()
  })
})

describe('RecentSearches', () => {
  it('shows empty state when no recent searches', () => {
    render(<RecentSearches searches={[]} onSelect={vi.fn()} onClear={vi.fn()} />)
    expect(screen.getByText(/start typing/i)).toBeInTheDocument()
  })

  it('renders recent search terms', () => {
    render(
      <RecentSearches searches={['build-bot', 'alert']} onSelect={vi.fn()} onClear={vi.fn()} />,
    )
    expect(screen.getByText('build-bot')).toBeInTheDocument()
    expect(screen.getByText('alert')).toBeInTheDocument()
  })

  it('calls onSelect when a recent search is clicked', () => {
    const onSelect = vi.fn()
    render(<RecentSearches searches={['build-bot']} onSelect={onSelect} onClear={vi.fn()} />)
    fireEvent.click(screen.getByText('build-bot'))
    expect(onSelect).toHaveBeenCalledWith('build-bot')
  })

  it('calls onClear when Clear button is clicked', () => {
    const onClear = vi.fn()
    render(<RecentSearches searches={['build-bot']} onSelect={vi.fn()} onClear={onClear} />)
    fireEvent.click(screen.getByRole('button', { name: /clear/i }))
    expect(onClear).toHaveBeenCalledOnce()
  })
})
