/**
 * Tests for MemoryFilters component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryFilters } from '@/components/memories/MemoryFilters'

describe('MemoryFilters', () => {
  const defaultProps = {
    filters: {},
    sortBy: 'created_at' as const,
    sortDir: 'desc' as const,
    search: '',
    onFiltersChange: vi.fn(),
    onSortChange: vi.fn(),
    onSearchChange: vi.fn(),
  }

  it('renders all filter controls', () => {
    render(<MemoryFilters {...defaultProps} />)
    expect(screen.getByLabelText('Search memory content')).toBeTruthy()
    expect(screen.getByLabelText('Filter by type')).toBeTruthy()
    expect(screen.getByLabelText('Filter by visibility')).toBeTruthy()
    expect(screen.getByLabelText('Filter by creator')).toBeTruthy()
    expect(screen.getByLabelText('Filter by tag')).toBeTruthy()
    expect(screen.getByLabelText('Sort order')).toBeTruthy()
  })

  it('calls onFiltersChange when type is changed', () => {
    const onFiltersChange = vi.fn()
    render(<MemoryFilters {...defaultProps} onFiltersChange={onFiltersChange} />)
    fireEvent.change(screen.getByLabelText('Filter by type'), {
      target: { value: 'question' },
    })
    expect(onFiltersChange).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'question' }),
    )
  })

  it('calls onFiltersChange when visibility is changed', () => {
    const onFiltersChange = vi.fn()
    render(<MemoryFilters {...defaultProps} onFiltersChange={onFiltersChange} />)
    fireEvent.change(screen.getByLabelText('Filter by visibility'), {
      target: { value: 'private' },
    })
    expect(onFiltersChange).toHaveBeenCalledWith(
      expect.objectContaining({ visibility: 'private' }),
    )
  })

  it('calls onSearchChange when search input changes', () => {
    const onSearchChange = vi.fn()
    render(<MemoryFilters {...defaultProps} onSearchChange={onSearchChange} />)
    fireEvent.change(screen.getByLabelText('Search memory content'), {
      target: { value: 'deploy' },
    })
    expect(onSearchChange).toHaveBeenCalledWith('deploy')
  })

  it('calls onSortChange when sort is changed', () => {
    const onSortChange = vi.fn()
    render(<MemoryFilters {...defaultProps} onSortChange={onSortChange} />)
    fireEvent.change(screen.getByLabelText('Sort order'), {
      target: { value: 'created_at:asc' },
    })
    expect(onSortChange).toHaveBeenCalledWith('created_at', 'asc')
  })

  it('shows reset button when filters are active', () => {
    render(
      <MemoryFilters {...defaultProps} filters={{ type: 'question' }} />,
    )
    expect(screen.getByText('Reset filters')).toBeTruthy()
  })

  it('hides reset button when no filters are active', () => {
    render(<MemoryFilters {...defaultProps} />)
    expect(screen.queryByText('Reset filters')).toBeNull()
  })

  it('resets filters and search on reset click', () => {
    const onFiltersChange = vi.fn()
    const onSearchChange = vi.fn()
    render(
      <MemoryFilters
        {...defaultProps}
        filters={{ type: 'question' }}
        search="test"
        onFiltersChange={onFiltersChange}
        onSearchChange={onSearchChange}
      />,
    )
    fireEvent.click(screen.getByText('Reset filters'))
    expect(onFiltersChange).toHaveBeenCalledWith({})
    expect(onSearchChange).toHaveBeenCalledWith('')
  })
})
