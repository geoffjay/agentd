import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { Pagination } from '@/components/common/Pagination'

describe('Pagination', () => {
  it('renders nothing when there is only 1 page', () => {
    const { container } = render(
      <Pagination
        page={1}
        totalPages={1}
        totalItems={5}
        pageSize={20}
        onPageChange={vi.fn()}
      />,
    )
    expect(container.firstChild).toBeNull()
  })

  it('renders page buttons for multiple pages', () => {
    render(
      <Pagination
        page={1}
        totalPages={3}
        totalItems={60}
        pageSize={20}
        onPageChange={vi.fn()}
      />,
    )
    expect(screen.getByRole('navigation', { name: /pagination/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /page 1/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /page 2/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /page 3/i })).toBeInTheDocument()
  })

  it('marks current page with aria-current', () => {
    render(
      <Pagination
        page={2}
        totalPages={3}
        totalItems={60}
        pageSize={20}
        onPageChange={vi.fn()}
      />,
    )
    const page2Btn = screen.getByRole('button', { name: /page 2/i })
    expect(page2Btn).toHaveAttribute('aria-current', 'page')
  })

  it('calls onPageChange with correct page when a page button is clicked', () => {
    const onPageChange = vi.fn()
    render(
      <Pagination
        page={1}
        totalPages={3}
        totalItems={60}
        pageSize={20}
        onPageChange={onPageChange}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /page 3/i }))
    expect(onPageChange).toHaveBeenCalledWith(3)
  })

  it('calls onPageChange with page+1 when next is clicked', () => {
    const onPageChange = vi.fn()
    render(
      <Pagination
        page={1}
        totalPages={3}
        totalItems={60}
        pageSize={20}
        onPageChange={onPageChange}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /next page/i }))
    expect(onPageChange).toHaveBeenCalledWith(2)
  })

  it('calls onPageChange with page-1 when previous is clicked', () => {
    const onPageChange = vi.fn()
    render(
      <Pagination
        page={2}
        totalPages={3}
        totalItems={60}
        pageSize={20}
        onPageChange={onPageChange}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /previous page/i }))
    expect(onPageChange).toHaveBeenCalledWith(1)
  })

  it('disables previous button on first page', () => {
    render(
      <Pagination
        page={1}
        totalPages={3}
        totalItems={60}
        pageSize={20}
        onPageChange={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: /previous page/i })).toBeDisabled()
  })

  it('disables next button on last page', () => {
    render(
      <Pagination
        page={3}
        totalPages={3}
        totalItems={60}
        pageSize={20}
        onPageChange={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: /next page/i })).toBeDisabled()
  })

  it('shows item range text', () => {
    render(
      <Pagination
        page={2}
        totalPages={3}
        totalItems={55}
        pageSize={20}
        onPageChange={vi.fn()}
      />,
    )
    // Items 21–40 of 55
    expect(screen.getByText('21')).toBeInTheDocument()
    expect(screen.getByText('40')).toBeInTheDocument()
    expect(screen.getByText('55')).toBeInTheDocument()
  })
})
