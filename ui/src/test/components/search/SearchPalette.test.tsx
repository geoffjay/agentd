import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { SearchPalette } from '@/components/search/SearchPalette'

// Mock useSearch to avoid real API calls
vi.mock('@/hooks/useSearch', () => ({
  useSearch: () => ({
    query: '',
    setQuery: vi.fn(),
    results: { actions: [], agents: [], notifications: [], total: 0 },
    loading: false,
    recentSearches: [],
    addRecentSearch: vi.fn(),
    clearRecentSearches: vi.fn(),
  }),
}))

// Mock useNavigate
const mockNavigate = vi.fn()
vi.mock('react-router-dom', async (importOriginal) => {
  const mod = await importOriginal()
  return {
    ...(mod as object),
    useNavigate: () => mockNavigate,
  }
})

function renderPalette(props: { isOpen: boolean; onClose?: () => void }) {
  return render(
    <MemoryRouter>
      <SearchPalette isOpen={props.isOpen} onClose={props.onClose ?? vi.fn()} />
    </MemoryRouter>,
  )
}

describe('SearchPalette', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('does not render when closed', () => {
    renderPalette({ isOpen: false })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('renders when open', () => {
    renderPalette({ isOpen: true })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  it('has aria-modal="true"', () => {
    renderPalette({ isOpen: true })
    expect(screen.getByRole('dialog')).toHaveAttribute('aria-modal', 'true')
  })

  it('renders the search input', () => {
    renderPalette({ isOpen: true })
    expect(screen.getByRole('combobox')).toBeInTheDocument()
  })

  it('calls onClose when Escape is pressed', () => {
    const onClose = vi.fn()
    renderPalette({ isOpen: true, onClose })
    fireEvent.keyDown(screen.getByRole('dialog').children[1], { key: 'Escape' })
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('calls onClose when backdrop is clicked', () => {
    const onClose = vi.fn()
    renderPalette({ isOpen: true, onClose })
    // The backdrop is the first child (aria-hidden div)
    const backdrop = screen.getByRole('dialog').querySelector('[aria-hidden="true"]')
    if (backdrop) fireEvent.click(backdrop)
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('shows keyboard shortcut hints in footer', () => {
    renderPalette({ isOpen: true })
    expect(screen.getByText('navigate')).toBeInTheDocument()
    expect(screen.getByText('open')).toBeInTheDocument()
    expect(screen.getByText('close')).toBeInTheDocument()
  })

  it('shows empty state with no recent searches', () => {
    renderPalette({ isOpen: true })
    expect(screen.getByText(/start typing/i)).toBeInTheDocument()
  })
})
