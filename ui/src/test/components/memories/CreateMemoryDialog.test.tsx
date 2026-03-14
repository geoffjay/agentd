/**
 * Tests for CreateMemoryDialog component.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { CreateMemoryDialog } from '@/components/memories/CreateMemoryDialog'

// Mock useToast
const mockSuccess = vi.fn()
vi.mock('@/hooks/useToast', () => ({
  useToast: () => ({
    success: mockSuccess,
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

// Mock useFocusTrap (just render children)
vi.mock('@/hooks/useFocusTrap', () => ({
  useFocusTrap: () => ({ current: null }),
}))

describe('CreateMemoryDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('does not render when closed', () => {
    render(
      <CreateMemoryDialog open={false} onSave={vi.fn()} onClose={vi.fn()} />,
    )
    expect(screen.queryByText('Create Memory')).toBeNull()
  })

  it('renders form fields when open', () => {
    render(
      <CreateMemoryDialog open={true} onSave={vi.fn()} onClose={vi.fn()} />,
    )
    expect(screen.getByText('Create Memory')).toBeTruthy()
    expect(screen.getByPlaceholderText('Enter the memory content…')).toBeTruthy()
    expect(screen.getByPlaceholderText(/agent-1 or user/)).toBeTruthy()
    expect(screen.getByPlaceholderText(/deployment, api/)).toBeTruthy()
    expect(screen.getByText('Create memory')).toBeTruthy()
    expect(screen.getByText('Cancel')).toBeTruthy()
  })

  it('shows validation errors when submitting empty form', async () => {
    render(
      <CreateMemoryDialog open={true} onSave={vi.fn()} onClose={vi.fn()} />,
    )
    fireEvent.click(screen.getByText('Create memory'))
    expect(screen.getByText('Content is required')).toBeTruthy()
    expect(screen.getByText('Created by is required')).toBeTruthy()
  })

  it('calls onSave with form data on valid submit', async () => {
    const onSave = vi.fn().mockResolvedValue({})
    const onClose = vi.fn()
    render(
      <CreateMemoryDialog open={true} onSave={onSave} onClose={onClose} />,
    )

    fireEvent.change(screen.getByPlaceholderText('Enter the memory content…'), {
      target: { value: 'Test memory content' },
    })
    fireEvent.change(screen.getByPlaceholderText(/agent-1 or user/), {
      target: { value: 'test-user' },
    })
    fireEvent.change(screen.getByPlaceholderText(/deployment, api/), {
      target: { value: 'tag1, tag2' },
    })

    fireEvent.click(screen.getByText('Create memory'))

    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          content: 'Test memory content',
          created_by: 'test-user',
          type: 'information',
          tags: ['tag1', 'tag2'],
          visibility: 'public',
        }),
      )
    })

    await waitFor(() => {
      expect(mockSuccess).toHaveBeenCalledWith('Memory created')
      expect(onClose).toHaveBeenCalled()
    })
  })

  it('shows shared_with field when visibility is shared', () => {
    render(
      <CreateMemoryDialog open={true} onSave={vi.fn()} onClose={vi.fn()} />,
    )
    // Initially no shared_with field
    expect(screen.queryByPlaceholderText(/agent-2, user/)).toBeNull()

    // Change visibility to shared
    const visibilitySelect = screen.getAllByRole('combobox').find(
      (el) => (el as HTMLSelectElement).value === 'public',
    )
    fireEvent.change(visibilitySelect!, { target: { value: 'shared' } })

    // Now shared_with field should appear
    expect(screen.getByPlaceholderText(/agent-2, user/)).toBeTruthy()
  })

  it('shows tag preview chips', () => {
    render(
      <CreateMemoryDialog open={true} onSave={vi.fn()} onClose={vi.fn()} />,
    )
    fireEvent.change(screen.getByPlaceholderText(/deployment, api/), {
      target: { value: 'alpha, beta' },
    })
    expect(screen.getByText('alpha')).toBeTruthy()
    expect(screen.getByText('beta')).toBeTruthy()
  })

  it('shows save error message on failure', async () => {
    const onSave = vi.fn().mockRejectedValue(new Error('Server error'))
    render(
      <CreateMemoryDialog open={true} onSave={onSave} onClose={vi.fn()} />,
    )

    fireEvent.change(screen.getByPlaceholderText('Enter the memory content…'), {
      target: { value: 'Content' },
    })
    fireEvent.change(screen.getByPlaceholderText(/agent-1 or user/), {
      target: { value: 'user' },
    })
    fireEvent.click(screen.getByText('Create memory'))

    await waitFor(() => {
      expect(screen.getByText('Server error')).toBeTruthy()
    })
  })

  it('calls onClose when Cancel is clicked', () => {
    const onClose = vi.fn()
    render(
      <CreateMemoryDialog open={true} onSave={vi.fn()} onClose={onClose} />,
    )
    fireEvent.click(screen.getByText('Cancel'))
    expect(onClose).toHaveBeenCalled()
  })

  it('calls onClose when close button is clicked', () => {
    const onClose = vi.fn()
    render(
      <CreateMemoryDialog open={true} onSave={vi.fn()} onClose={onClose} />,
    )
    fireEvent.click(screen.getByLabelText('Close dialog'))
    expect(onClose).toHaveBeenCalled()
  })
})
