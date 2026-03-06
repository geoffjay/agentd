import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'

describe('ConfirmDialog', () => {
  it('renders nothing when closed', () => {
    render(
      <ConfirmDialog
        open={false}
        title="Delete?"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
  })

  it('renders title and description when open', () => {
    render(
      <ConfirmDialog
        open
        title="Delete agent?"
        description="This cannot be undone."
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    expect(screen.getByRole('alertdialog')).toBeInTheDocument()
    expect(screen.getByText('Delete agent?')).toBeInTheDocument()
    expect(screen.getByText('This cannot be undone.')).toBeInTheDocument()
  })

  it('calls onConfirm when confirm button clicked', () => {
    const onConfirm = vi.fn()
    render(
      <ConfirmDialog
        open
        title="Delete?"
        confirmLabel="Delete"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /delete/i }))
    expect(onConfirm).toHaveBeenCalledOnce()
  })

  it('calls onCancel when cancel button clicked', () => {
    const onCancel = vi.fn()
    render(
      <ConfirmDialog
        open
        title="Delete?"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(onCancel).toHaveBeenCalledOnce()
  })

  it('calls onCancel when Escape key is pressed', () => {
    const onCancel = vi.fn()
    render(
      <ConfirmDialog
        open
        title="Delete?"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    )
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalledOnce()
  })

  it('calls onCancel when backdrop clicked', () => {
    const onCancel = vi.fn()
    render(
      <ConfirmDialog
        open
        title="Delete?"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    )
    // The backdrop is the aria-hidden overlay div
    const overlay = document.querySelector('.absolute.inset-0.bg-black\\/50')
    expect(overlay).toBeTruthy()
    fireEvent.click(overlay!)
    expect(onCancel).toHaveBeenCalledOnce()
  })

  it('shows processing label and disables buttons when loading', () => {
    render(
      <ConfirmDialog
        open
        title="Delete?"
        confirmLabel="Delete"
        loading
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    expect(screen.getByText('Processing…')).toBeInTheDocument()
    const cancelBtn = screen.getByRole('button', { name: /cancel/i })
    expect(cancelBtn).toBeDisabled()
  })

  it('uses default labels when none are provided', () => {
    render(
      <ConfirmDialog
        open
        title="Confirm?"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: /^confirm$/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^cancel$/i })).toBeInTheDocument()
  })
})
