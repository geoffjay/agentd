import { render, screen } from '@testing-library/react'
import { describe, it, expect } from 'vitest'
import { ApprovalBadge } from './ApprovalBadge'

describe('ApprovalBadge', () => {
  it('renders nothing when count is 0 and showZero is false', () => {
    const { container } = render(<ApprovalBadge count={0} />)
    expect(container.firstChild).toBeNull()
  })

  it('renders zero when showZero is true', () => {
    render(<ApprovalBadge count={0} showZero />)
    expect(screen.getByText('0')).toBeInTheDocument()
  })

  it('renders the count', () => {
    render(<ApprovalBadge count={5} />)
    expect(screen.getByText('5')).toBeInTheDocument()
  })

  it('caps display at 99+', () => {
    render(<ApprovalBadge count={100} />)
    expect(screen.getByText('99+')).toBeInTheDocument()
  })

  it('has correct aria-label for singular', () => {
    render(<ApprovalBadge count={1} />)
    expect(screen.getByLabelText('1 pending approval')).toBeInTheDocument()
  })

  it('has correct aria-label for plural', () => {
    render(<ApprovalBadge count={3} />)
    expect(screen.getByLabelText('3 pending approvals')).toBeInTheDocument()
  })
})
