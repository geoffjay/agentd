import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect, vi } from 'vitest'
import { ApprovalActions } from './ApprovalActions'

describe('ApprovalActions', () => {
  it('renders approve and deny buttons', () => {
    render(
      <ApprovalActions approvalId="a1" onApprove={vi.fn()} onDeny={vi.fn()} />,
    )
    expect(screen.getByRole('button', { name: /approve/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /deny/i })).toBeInTheDocument()
  })

  it('calls onApprove with the id', async () => {
    const onApprove = vi.fn()
    render(
      <ApprovalActions approvalId="a1" onApprove={onApprove} onDeny={vi.fn()} />,
    )
    await userEvent.click(screen.getByRole('button', { name: /approve/i }))
    expect(onApprove).toHaveBeenCalledWith('a1')
  })

  it('calls onDeny with the id', async () => {
    const onDeny = vi.fn()
    render(
      <ApprovalActions approvalId="a1" onApprove={vi.fn()} onDeny={onDeny} />,
    )
    await userEvent.click(screen.getByRole('button', { name: /deny/i }))
    expect(onDeny).toHaveBeenCalledWith('a1')
  })

  it('disables buttons when busy', () => {
    render(
      <ApprovalActions approvalId="a1" busy onApprove={vi.fn()} onDeny={vi.fn()} />,
    )
    expect(screen.getByRole('button', { name: /approve/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /deny/i })).toBeDisabled()
  })
})
