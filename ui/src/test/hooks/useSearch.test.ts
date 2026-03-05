import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useSearch } from '@/hooks/useSearch'

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

vi.mock('@/services/orchestrator', () => ({
  orchestratorClient: {
    listAgents: vi.fn().mockResolvedValue({
      items: [
        { id: '1', name: 'build-bot', status: 'Running', config: {}, created_at: '', updated_at: '' },
        { id: '2', name: 'deploy-bot', status: 'Pending', config: {}, created_at: '', updated_at: '' },
        { id: '3', name: 'test-runner', status: 'Stopped', config: {}, created_at: '', updated_at: '' },
      ],
      total: 3,
      limit: 200,
      offset: 0,
    }),
  },
}))

vi.mock('@/services/notify', () => ({
  notifyClient: {
    listNotifications: vi.fn().mockResolvedValue({
      items: [
        {
          id: 'n1',
          title: 'High memory alert',
          message: 'Memory usage above 90%',
          priority: 'High',
          status: 'Pending',
          source: 'MonitorService',
          lifetime: { type: 'Persistent' },
          requires_response: false,
          created_at: '',
          updated_at: '',
        },
      ],
      total: 1,
      limit: 200,
      offset: 0,
    }),
  },
}))

beforeEach(() => {
  vi.useFakeTimers()
  localStorage.clear()
})

afterEach(() => {
  vi.useRealTimers()
})

/** Advance debounce timer and flush all pending promises */
async function flushDebounce() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(300)
  })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('useSearch', () => {
  it('returns empty results for an empty query', () => {
    const { result } = renderHook(() => useSearch())
    expect(result.current.results.total).toBe(0)
    expect(result.current.loading).toBe(false)
  })

  it('debounces — does not fire before 300ms have elapsed', async () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.setQuery('build')
    })

    // Only advance 100ms — debounce has not fired
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100)
    })

    // No results yet, not loading
    expect(result.current.results.total).toBe(0)
    expect(result.current.loading).toBe(false)
  })

  it('finds agent results matching the query after debounce', async () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.setQuery('build')
    })
    await flushDebounce()

    expect(result.current.results.agents.length).toBeGreaterThan(0)
    expect(result.current.results.agents[0].title).toBe('build-bot')
  })

  it('finds notification results matching the query after debounce', async () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.setQuery('memory')
    })
    await flushDebounce()

    expect(result.current.results.notifications.length).toBeGreaterThan(0)
    expect(result.current.results.notifications[0].title).toBe('High memory alert')
  })

  it('filters quick actions matching the query', async () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.setQuery('agents')
    })
    await flushDebounce()

    expect(result.current.results.actions.length).toBeGreaterThan(0)
    expect(result.current.results.actions.some((a) => a.title.includes('Agent'))).toBe(true)
  })

  it('clears results when query is set back to empty', async () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.setQuery('build')
    })
    await flushDebounce()
    expect(result.current.results.agents.length).toBeGreaterThan(0)

    act(() => {
      result.current.setQuery('')
    })
    await flushDebounce()

    expect(result.current.results.total).toBe(0)
  })

  it('ranks exact matches above prefix/contains', async () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.setQuery('build-bot')
    })
    await flushDebounce()

    const agents = result.current.results.agents
    expect(agents.length).toBeGreaterThan(0)
    // build-bot is an exact match → should be ranked first
    expect(agents[0].title).toBe('build-bot')
  })

  it('persists recent searches to localStorage', () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.addRecentSearch('build-bot')
    })

    expect(result.current.recentSearches).toContain('build-bot')
    const stored = JSON.parse(localStorage.getItem('agentd:recent-searches') ?? '[]') as string[]
    expect(stored).toContain('build-bot')
  })

  it('deduplicates recent searches', () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.addRecentSearch('build-bot')
      result.current.addRecentSearch('build-bot')
    })

    expect(result.current.recentSearches.filter((s) => s === 'build-bot')).toHaveLength(1)
  })

  it('limits recent searches to 5 entries', () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      for (let i = 0; i < 7; i++) {
        result.current.addRecentSearch(`search-${i}`)
      }
    })

    expect(result.current.recentSearches).toHaveLength(5)
  })

  it('clears all recent searches', () => {
    const { result } = renderHook(() => useSearch())

    act(() => {
      result.current.addRecentSearch('build-bot')
    })

    act(() => {
      result.current.clearRecentSearches()
    })

    expect(result.current.recentSearches).toHaveLength(0)
    expect(localStorage.getItem('agentd:recent-searches')).toBe('[]')
  })
})
