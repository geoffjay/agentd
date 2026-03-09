import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { AgentApprovals } from '@/components/agents/AgentApprovals'
import { makeApprovalList, resetAgentSeq } from '@/test/mocks/factories'

beforeEach(() => resetAgentSeq())

describe('AgentApprovals', () => {
  const defaultProps = {
    approvals: [],
    loading: false,
    onApprove: vi.fn().mockResolvedValue(undefined),
    onDeny: vi.fn().mockResolvedValue(undefined),
  }

  it('shows "No pending approvals" when list is empty', () => {
    render(<AgentApprovals {...defaultProps} />)
    expect(screen.getByText(/no pending approvals/i)).toBeInTheDocument()
  })

  it('renders approval rows', () => {
    const approvals = makeApprovalList(2)
    render(<AgentApprovals {...defaultProps} approvals={approvals} />)
    // Both approvals share the same tool_name so use getAllByText
    expect(screen.getAllByText(approvals[0].tool_name)).toHaveLength(approvals.length)
  })

  it('shows count badge when there are pending approvals', () => {
    const approvals = makeApprovalList(3)
    render(<AgentApprovals {...defaultProps} approvals={approvals} />)
    expect(screen.getByText('3')).toBeInTheDocument()
  })

  it('does not show count badge when empty', () => {
    render(<AgentApprovals {...defaultProps} />)
    // No number badge for 0
    expect(screen.queryByText('0')).not.toBeInTheDocument()
  })

  it('shows loading skeleton when loading', () => {
    render(<AgentApprovals {...defaultProps} loading />)
    // Loading state renders skeleton — no "no pending approvals" text
    expect(screen.queryByText(/no pending approvals/i)).not.toBeInTheDocument()
  })

  it('shows error when error is provided', () => {
    render(<AgentApprovals {...defaultProps} error="Failed to load" />)
    expect(screen.getByRole('alert')).toHaveTextContent('Failed to load')
  })

  it('calls onApprove with correct id when Approve clicked', async () => {
    const onApprove = vi.fn().mockResolvedValue(undefined)
    const approvals = makeApprovalList(1)
    render(<AgentApprovals {...defaultProps} approvals={approvals} onApprove={onApprove} />)

    fireEvent.click(screen.getByRole('button', { name: /approve/i }))
    await waitFor(() => expect(onApprove).toHaveBeenCalledWith(approvals[0].id))
  })

  it('calls onDeny with correct id when Deny clicked', async () => {
    const onDeny = vi.fn().mockResolvedValue(undefined)
    const approvals = makeApprovalList(1)
    render(<AgentApprovals {...defaultProps} approvals={approvals} onDeny={onDeny} />)

    fireEvent.click(screen.getByRole('button', { name: /deny/i }))
    await waitFor(() => expect(onDeny).toHaveBeenCalledWith(approvals[0].id))
  })

  it('shows tool input when expanded', () => {
    const approvals = makeApprovalList(1)
    render(<AgentApprovals {...defaultProps} approvals={approvals} />)

    fireEvent.click(screen.getByRole('button', { name: /tool input/i }))
    // The tool input is rendered in a <pre> element
    expect(document.querySelector('pre')).toBeTruthy()
  })
})
