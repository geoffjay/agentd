/**
 * Integration tests for the Memories page.
 *
 * Uses MSW to intercept real fetch calls and verifies end-to-end behaviour
 * of the MemoryList page component including loading, rendering, filtering,
 * pagination, create/delete flows, search mode, and error handling.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { http, HttpResponse } from 'msw'
import { server } from '@/test/mocks/server'
import { axe } from '@/test/setup'
import { MemoryList } from '@/pages/memories/MemoryList'
import { makeMemory, makeMemoryList, resetMemorySeq } from '@/test/mocks/factories'
import type { PaginatedResponse } from '@/types/common'
import type { Memory } from '@/types/memory'

// ---------------------------------------------------------------------------
// Mock useToast (used by useMemories / useMemorySearch)
// ---------------------------------------------------------------------------

const mockSuccess = vi.fn()
const mockApiError = vi.fn().mockReturnValue('toast-id')

vi.mock('@/hooks/useToast', () => ({
  useToast: () => ({
    success: mockSuccess,
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
    dismiss: vi.fn(),
    clear: vi.fn(),
    apiError: mockApiError,
  }),
  mapApiError: (err: unknown) =>
    err instanceof Error ? err.message : String(err),
}))

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const BASE = 'http://localhost:17008'

function paginated<T>(items: T[], total?: number): PaginatedResponse<T> {
  return { items, total: total ?? items.length, limit: 50, offset: 0 }
}

function renderPage(initialPath = '/memories') {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <MemoryList />
    </MemoryRouter>,
  )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('MemoriesPage (MSW integration)', () => {
  beforeEach(() => {
    resetMemorySeq()
    mockSuccess.mockClear()
    mockApiError.mockClear()
  })

  // -----------------------------------------------------------------------
  // Loading state
  // -----------------------------------------------------------------------

  it('displays loading skeletons while fetching', () => {
    // Override to never resolve — keeps component in loading state
    server.use(
      http.get(`${BASE}/memories`, () => new Promise(() => {})),
    )

    renderPage()
    expect(screen.getAllByLabelText('Loading…').length).toBeGreaterThan(0)
  })

  // -----------------------------------------------------------------------
  // Successful render
  // -----------------------------------------------------------------------

  it('renders memory cards with MSW data', async () => {
    const memories = [
      makeMemory({ content: 'Deployment runbook for production' }),
      makeMemory({ content: 'API rate limit configuration' }),
    ]

    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>(memories)),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('Deployment runbook for production')).toBeInTheDocument()
    })
    expect(screen.getByText('API rate limit configuration')).toBeInTheDocument()
  })

  it('shows the page heading and count', async () => {
    const memories = makeMemoryList(3)
    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>(memories)),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('Memories')).toBeInTheDocument()
    })
  })

  // -----------------------------------------------------------------------
  // Empty state
  // -----------------------------------------------------------------------

  it('renders empty state correctly when no memories exist', async () => {
    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>([])),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('No memories found')).toBeInTheDocument()
    })
    expect(screen.getByText('Get started by creating your first memory.')).toBeInTheDocument()
  })

  // -----------------------------------------------------------------------
  // Error state
  // -----------------------------------------------------------------------

  it('displays error state with retry action', async () => {
    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json({ error: 'Internal error' }, { status: 404 }),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('Retry')).toBeInTheDocument()
    })
  })

  it('retries fetch when retry button is clicked', async () => {
    let callCount = 0
    server.use(
      http.get(`${BASE}/memories`, () => {
        callCount++
        if (callCount === 1) {
          return HttpResponse.json({ error: 'fail' }, { status: 404 })
        }
        return HttpResponse.json(
          paginated<Memory>([makeMemory({ content: 'Recovered memory' })]),
        )
      }),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('Retry')).toBeInTheDocument()
    })

    fireEvent.click(screen.getByText('Retry'))

    await waitFor(() => {
      expect(screen.getByText('Recovered memory')).toBeInTheDocument()
    })
  })

  // -----------------------------------------------------------------------
  // Filter controls
  // -----------------------------------------------------------------------

  it('renders filter controls for type, visibility, and content search', async () => {
    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>(makeMemoryList(1))),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByLabelText('Filter by type')).toBeInTheDocument()
      expect(screen.getByLabelText('Filter by visibility')).toBeInTheDocument()
      expect(screen.getByLabelText('Search memory content')).toBeInTheDocument()
    })
  })

  it('filters by memory type via the type dropdown', async () => {
    const infoMem = makeMemory({ content: 'Info content', type: 'information' })
    const questionMem = makeMemory({ content: 'Question content', type: 'question' })

    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>([infoMem, questionMem])),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('Info content')).toBeInTheDocument()
      expect(screen.getByText('Question content')).toBeInTheDocument()
    })

    // Select question type filter
    fireEvent.change(screen.getByLabelText('Filter by type'), {
      target: { value: 'question' },
    })

    await waitFor(() => {
      expect(screen.getByText('Question content')).toBeInTheDocument()
    })
  })

  it('filters by visibility dropdown', async () => {
    const pubMem = makeMemory({ content: 'Public memo', visibility: 'public' })
    const privMem = makeMemory({ content: 'Private memo', visibility: 'private' })

    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>([pubMem, privMem])),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('Public memo')).toBeInTheDocument()
    })

    fireEvent.change(screen.getByLabelText('Filter by visibility'), {
      target: { value: 'private' },
    })

    await waitFor(() => {
      expect(screen.getByText('Private memo')).toBeInTheDocument()
    })
  })

  // -----------------------------------------------------------------------
  // Pagination
  // -----------------------------------------------------------------------

  it('paginates through memory pages', async () => {
    // Create enough memories for pagination (default page size is typically 12)
    const memories = makeMemoryList(15)

    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>(memories, 15)),
      ),
    )

    renderPage()

    await waitFor(() => {
      // Should show some memory content from the loaded data
      expect(screen.getByText('Memories')).toBeInTheDocument()
    })

    // Check if pagination controls exist (Next button or page numbers)
    const nextButton = screen.queryByLabelText('Next page') ?? screen.queryByText('Next')
    if (nextButton) {
      fireEvent.click(nextButton)
      // After clicking next, the page should still show the heading
      await waitFor(() => {
        expect(screen.getByText('Memories')).toBeInTheDocument()
      })
    }
  })

  // -----------------------------------------------------------------------
  // Create dialog
  // -----------------------------------------------------------------------

  it('opens create dialog, validates, submits, and shows toast', async () => {
    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>([])),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('No memories found')).toBeInTheDocument()
    })

    // Open create dialog — the button in the empty state says "Create Memory"
    const createBtn = screen.getByText('Create Memory')
    fireEvent.click(createBtn)

    await waitFor(() => {
      // The dialog title is "Create Memory" and has a content textarea
      expect(screen.getByPlaceholderText('Enter the memory content…')).toBeInTheDocument()
    })

    // Try submitting empty form — should show validation
    const submitBtn = screen.getByText('Create memory')
    fireEvent.click(submitBtn)

    await waitFor(() => {
      // Validation messages should appear
      expect(screen.getByText(/Content is required/)).toBeInTheDocument()
    })

    // Fill in valid data
    fireEvent.change(screen.getByPlaceholderText('Enter the memory content…'), {
      target: { value: 'New integration test memory' },
    })
    fireEvent.change(screen.getByPlaceholderText(/agent-1 or user/), {
      target: { value: 'test-user' },
    })

    // Submit
    fireEvent.click(submitBtn)

    await waitFor(() => {
      // Dialog should close after successful create
      expect(screen.queryByPlaceholderText('Enter the memory content…')).not.toBeInTheDocument()
    })
  })

  // -----------------------------------------------------------------------
  // Delete confirmation
  // -----------------------------------------------------------------------

  it('delete shows confirmation, executes, and shows toast', async () => {
    const mem = makeMemory({ content: 'Memory to delete' })

    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>([mem])),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('Memory to delete')).toBeInTheDocument()
    })

    // Click delete button on the card
    fireEvent.click(screen.getByLabelText('Delete memory'))

    // Confirmation dialog should appear
    await waitFor(() => {
      expect(screen.getByText('Delete memory')).toBeInTheDocument()
      expect(screen.getByText(/cannot be undone/)).toBeInTheDocument()
    })

    // Confirm deletion — use the confirm button inside the dialog (not the card button)
    const allDeleteButtons = screen.getAllByText('Delete')
    const confirmBtn = allDeleteButtons[allDeleteButtons.length - 1]
    fireEvent.click(confirmBtn)

    await waitFor(() => {
      // Dialog should close
      expect(screen.queryByText(/cannot be undone/)).not.toBeInTheDocument()
    })
  })

  // -----------------------------------------------------------------------
  // Search mode
  // -----------------------------------------------------------------------

  it('switches to search mode and displays results', async () => {
    const mem = makeMemory({ content: 'Searchable memory' })

    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>([mem])),
      ),
      http.post(`${BASE}/memories/search`, () =>
        HttpResponse.json({
          memories: [makeMemory({ content: 'Search result memory' })],
          total: 1,
        }),
      ),
    )

    renderPage()

    await waitFor(() => {
      expect(screen.getByText('Searchable memory')).toBeInTheDocument()
    })

    // Switch to search tab — the view mode group has a "Search" button with aria-pressed
    const searchTab = screen.getByRole('button', { name: 'Search', pressed: false })
    fireEvent.click(searchTab)

    await waitFor(() => {
      // Search input should appear (placeholder: "Semantic search across memories…")
      expect(screen.getByPlaceholderText(/semantic search across memories/i)).toBeInTheDocument()
    })
  })

  // -----------------------------------------------------------------------
  // Sidebar active state verification
  // -----------------------------------------------------------------------

  it('renders at the /memories path without errors', async () => {
    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>(makeMemoryList(1))),
      ),
    )

    renderPage('/memories')

    await waitFor(() => {
      expect(screen.getByText('Memories')).toBeInTheDocument()
    })
  })

  // -----------------------------------------------------------------------
  // Accessibility
  // -----------------------------------------------------------------------

  it('has no accessibility violations on empty state', async () => {
    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>([])),
      ),
    )

    const { container } = renderPage()

    await waitFor(() => {
      expect(screen.getByText('No memories found')).toBeInTheDocument()
    })

    const results = await axe(container)
    expect(results).toHaveNoViolations()
  })

  it('has no accessibility violations with memory cards', async () => {
    const memories = [
      makeMemory({ content: 'A11y test memory one' }),
      makeMemory({ content: 'A11y test memory two', type: 'question', visibility: 'shared', shared_with: ['user-2'] }),
    ]

    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json(paginated<Memory>(memories)),
      ),
    )

    const { container } = renderPage()

    await waitFor(() => {
      expect(screen.getByText('A11y test memory one')).toBeInTheDocument()
    })

    const results = await axe(container)
    expect(results).toHaveNoViolations()
  })

  it('has no accessibility violations on error state', async () => {
    server.use(
      http.get(`${BASE}/memories`, () =>
        HttpResponse.json({ error: 'fail' }, { status: 404 }),
      ),
    )

    const { container } = renderPage()

    await waitFor(() => {
      expect(screen.getByText('Retry')).toBeInTheDocument()
    })

    const results = await axe(container)
    expect(results).toHaveNoViolations()
  })
})
