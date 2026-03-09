import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AgentFilters } from '@/components/agents/AgentFilters'

describe('AgentFilters', () => {
  const defaultProps = {
    status: '' as const,
    onStatusChange: vi.fn(),
    search: '',
    onSearchChange: vi.fn(),
    displayCount: 10,
    totalCount: 20,
  }

  it('renders status dropdown and search input', () => {
    render(<AgentFilters {...defaultProps} />)
    expect(screen.getByRole('combobox', { name: /filter by status/i })).toBeInTheDocument()
    expect(screen.getByRole('searchbox', { name: /search agents/i })).toBeInTheDocument()
  })

  it('shows count of displayed vs total agents', () => {
    render(<AgentFilters {...defaultProps} displayCount={5} totalCount={15} />)
    expect(screen.getByText('5')).toBeInTheDocument()
    expect(screen.getByText('15')).toBeInTheDocument()
  })

  it('shows "agent" (singular) when totalCount is 1', () => {
    render(<AgentFilters {...defaultProps} displayCount={1} totalCount={1} />)
    // The count is in a <span> and the word in a text node, so use textContent
    const p = screen.getByText(
      (_, el) => el?.tagName === 'P' && /1 agent$/.test(el.textContent?.trim() ?? ''),
    )
    expect(p).toBeInTheDocument()
  })

  it('shows "agents" (plural) when totalCount is 0', () => {
    render(<AgentFilters {...defaultProps} displayCount={0} totalCount={0} />)
    const p = screen.getByText(
      (_, el) => el?.tagName === 'P' && /0 agents$/.test(el.textContent?.trim() ?? ''),
    )
    expect(p).toBeInTheDocument()
  })

  it('calls onStatusChange when status dropdown changes', () => {
    const onStatusChange = vi.fn()
    render(<AgentFilters {...defaultProps} onStatusChange={onStatusChange} />)
    fireEvent.change(screen.getByRole('combobox', { name: /filter by status/i }), {
      target: { value: 'Running' },
    })
    expect(onStatusChange).toHaveBeenCalledWith('Running')
  })

  it('calls onSearchChange when search input changes', () => {
    const onSearchChange = vi.fn()
    render(<AgentFilters {...defaultProps} onSearchChange={onSearchChange} />)
    fireEvent.change(screen.getByRole('searchbox', { name: /search agents/i }), {
      target: { value: 'my-agent' },
    })
    expect(onSearchChange).toHaveBeenCalledWith('my-agent')
  })

  it('shows clear button when search has a value', () => {
    render(<AgentFilters {...defaultProps} search="my-agent" />)
    expect(screen.getByRole('button', { name: /clear search/i })).toBeInTheDocument()
  })

  it('does not show clear button when search is empty', () => {
    render(<AgentFilters {...defaultProps} search="" />)
    expect(screen.queryByRole('button', { name: /clear search/i })).not.toBeInTheDocument()
  })

  it('calls onSearchChange with empty string when clear is clicked', () => {
    const onSearchChange = vi.fn()
    render(<AgentFilters {...defaultProps} search="my-agent" onSearchChange={onSearchChange} />)
    fireEvent.click(screen.getByRole('button', { name: /clear search/i }))
    expect(onSearchChange).toHaveBeenCalledWith('')
  })

  it('shows all status options in dropdown', () => {
    render(<AgentFilters {...defaultProps} />)
    const dropdown = screen.getByRole('combobox', { name: /filter by status/i })
    const options = Array.from(dropdown.querySelectorAll('option')).map((o) => o.value)
    expect(options).toContain('')
    expect(options).toContain('Running')
    expect(options).toContain('Pending')
    expect(options).toContain('Stopped')
    expect(options).toContain('Failed')
  })
})
