/**
 * Tests for MemorySearch component.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemorySearch } from '@/components/memories/MemorySearch'
import { memoryClient } from '@/services/memory'
import type { Memory } from '@/types/memory'

// Mock useToast
vi.mock('@/hooks/useToast', () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
    dismiss: vi.fn(),
    clear: vi.fn(),
    apiError: vi.fn(),
  }),
  mapApiError: (err: unknown) =>
    err instanceof Error ? err.message : String(err),
}))

function makeMemory(overrides: Partial<Memory> = {}): Memory {
  return {
    id: 'mem_123',
    content: 'Search result memory',
    type: 'information',
    tags: ['test'],
    created_by: 'agent-1',
    owner: undefined,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    visibility: 'public',
    shared_with: [],
    references: [],
    ...overrides,
  }
}

describe('MemorySearch', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  const defaultProps = {
    onSwitchToList: vi.fn(),
    onEditVisibility: vi.fn(),
    onDelete: vi.fn(),
  }

  it('renders search input and initial state', () => {
    render(<MemorySearch {...defaultProps} />)
    expect(screen.getByLabelText('Semantic search query')).toBeTruthy()
    expect(screen.getByText('Search')).toBeTruthy()
    expect(screen.getByText('Enter a query to search memories')).toBeTruthy()
  })

  it('disables search button when query is empty', () => {
    render(<MemorySearch {...defaultProps} />)
    const button = screen.getByText('Search')
    expect((button as HTMLButtonElement).disabled).toBe(true)
  })

  it('executes search on button click', async () => {
    const mem = makeMemory({ content: 'Deployment procedure' })
    vi.spyOn(memoryClient, 'searchMemories').mockResolvedValue({
      memories: [mem],
      total: 1,
    })

    render(<MemorySearch {...defaultProps} />)

    fireEvent.change(screen.getByLabelText('Semantic search query'), {
      target: { value: 'deployment' },
    })
    fireEvent.click(screen.getByText('Search'))

    await waitFor(() => {
      expect(screen.getByText('Deployment procedure')).toBeTruthy()
    })
    expect(screen.getByText(/1 result/)).toBeTruthy()
  })

  it('executes search on Enter key', async () => {
    vi.spyOn(memoryClient, 'searchMemories').mockResolvedValue({
      memories: [],
      total: 0,
    })

    render(<MemorySearch {...defaultProps} />)

    const input = screen.getByLabelText('Semantic search query')
    fireEvent.change(input, { target: { value: 'test query' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    await waitFor(() => {
      expect(memoryClient.searchMemories).toHaveBeenCalledWith(
        expect.objectContaining({ query: 'test query' }),
      )
    })
  })

  it('shows no results message', async () => {
    vi.spyOn(memoryClient, 'searchMemories').mockResolvedValue({
      memories: [],
      total: 0,
    })

    render(<MemorySearch {...defaultProps} />)

    fireEvent.change(screen.getByLabelText('Semantic search query'), {
      target: { value: 'nonexistent' },
    })
    fireEvent.click(screen.getByText('Search'))

    await waitFor(() => {
      expect(screen.getByText('No matching memories found')).toBeTruthy()
    })
  })

  it('shows error on search failure', async () => {
    vi.spyOn(memoryClient, 'searchMemories').mockRejectedValue(
      new Error('Search unavailable'),
    )

    render(<MemorySearch {...defaultProps} />)

    fireEvent.change(screen.getByLabelText('Semantic search query'), {
      target: { value: 'test' },
    })
    fireEvent.click(screen.getByText('Search'))

    await waitFor(() => {
      expect(screen.getByText('Search unavailable')).toBeTruthy()
    })
  })

  it('toggles advanced filters', () => {
    render(<MemorySearch {...defaultProps} />)

    expect(screen.queryByLabelText('Filter by type')).toBeNull()

    fireEvent.click(screen.getByText('Advanced filters'))

    expect(screen.getByLabelText('Filter by type')).toBeTruthy()
    expect(screen.getByLabelText('Filter by tags')).toBeTruthy()
    expect(screen.getByLabelText('From date')).toBeTruthy()
    expect(screen.getByLabelText('To date')).toBeTruthy()
    expect(screen.getByLabelText('Result limit')).toBeTruthy()
  })

  it('calls onSwitchToList when back link is clicked', () => {
    const onSwitchToList = vi.fn()
    render(<MemorySearch {...defaultProps} onSwitchToList={onSwitchToList} />)

    fireEvent.click(screen.getByText(/Back to memory list/))
    expect(onSwitchToList).toHaveBeenCalled()
  })

  it('clears results when clear button is clicked', async () => {
    const mem = makeMemory()
    vi.spyOn(memoryClient, 'searchMemories').mockResolvedValue({
      memories: [mem],
      total: 1,
    })

    render(<MemorySearch {...defaultProps} />)

    // Search first
    fireEvent.change(screen.getByLabelText('Semantic search query'), {
      target: { value: 'test' },
    })
    fireEvent.click(screen.getByText('Search'))

    await waitFor(() => {
      expect(screen.getByText('Search result memory')).toBeTruthy()
    })

    // Clear
    fireEvent.click(screen.getByLabelText('Clear search'))

    await waitFor(() => {
      expect(screen.queryByText('Search result memory')).toBeNull()
      expect(screen.getByText('Enter a query to search memories')).toBeTruthy()
    })
  })
})
