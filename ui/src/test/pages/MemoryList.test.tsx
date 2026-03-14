/**
 * Tests for MemoryList page component.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { MemoryList } from '@/pages/memories/MemoryList'
import { memoryClient } from '@/services/memory'
import type { Memory } from '@/types/memory'

// ---------------------------------------------------------------------------
// Mock useToast (used by useMemories)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeMemory(overrides: Partial<Memory> = {}): Memory {
  return {
    id: 'mem_1234567890_abcdef12',
    content: 'Test memory content',
    type: 'information',
    tags: ['test'],
    created_by: 'agent-1',
    owner: undefined,
    created_at: '2024-01-15T10:00:00Z',
    updated_at: '2024-01-15T10:00:00Z',
    visibility: 'public',
    shared_with: [],
    references: [],
    ...overrides,
  }
}

function renderWithRouter(ui: React.ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('MemoryList', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('shows loading skeletons initially', () => {
    // Never resolve to keep loading state
    vi.spyOn(memoryClient, 'listMemories').mockReturnValue(new Promise(() => {}))
    renderWithRouter(<MemoryList />)
    expect(screen.getAllByLabelText('Loading…').length).toBeGreaterThan(0)
  })

  it('renders memory cards after loading', async () => {
    const mem = makeMemory({ content: 'Hello world memory' })
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [mem],
      total: 1,
      limit: 200,
      offset: 0,
    })

    renderWithRouter(<MemoryList />)
    await waitFor(() => {
      expect(screen.getByText('Hello world memory')).toBeTruthy()
    })
  })

  it('shows error state with retry button', async () => {
    vi.spyOn(memoryClient, 'listMemories').mockRejectedValue(
      new Error('Service unavailable'),
    )

    renderWithRouter(<MemoryList />)
    await waitFor(() => {
      expect(screen.getByText('Service unavailable')).toBeTruthy()
    })
    expect(screen.getByText('Retry')).toBeTruthy()
  })

  it('shows empty state when no memories exist', async () => {
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [],
      total: 0,
      limit: 200,
      offset: 0,
    })

    renderWithRouter(<MemoryList />)
    await waitFor(() => {
      expect(screen.getByText('No memories found')).toBeTruthy()
    })
    expect(
      screen.getByText('Get started by creating your first memory.'),
    ).toBeTruthy()
  })

  it('shows page header with count badge', async () => {
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [makeMemory({ id: 'm1' }), makeMemory({ id: 'm2' })],
      total: 2,
      limit: 200,
      offset: 0,
    })

    renderWithRouter(<MemoryList />)
    await waitFor(() => {
      expect(screen.getByText('Memories')).toBeTruthy()
    })
  })

  it('opens delete confirmation dialog', async () => {
    const mem = makeMemory()
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [mem],
      total: 1,
      limit: 200,
      offset: 0,
    })

    renderWithRouter(<MemoryList />)
    await waitFor(() => {
      expect(screen.getByText('Test memory content')).toBeTruthy()
    })

    fireEvent.click(screen.getByLabelText('Delete memory'))
    await waitFor(() => {
      expect(screen.getByText('Delete memory')).toBeTruthy()
      expect(screen.getByText(/cannot be undone/)).toBeTruthy()
    })
  })

  it('renders filter controls', async () => {
    vi.spyOn(memoryClient, 'listMemories').mockResolvedValue({
      items: [],
      total: 0,
      limit: 200,
      offset: 0,
    })

    renderWithRouter(<MemoryList />)
    await waitFor(() => {
      expect(screen.getByLabelText('Filter by type')).toBeTruthy()
      expect(screen.getByLabelText('Filter by visibility')).toBeTruthy()
      expect(screen.getByLabelText('Search memory content')).toBeTruthy()
    })
  })
})
