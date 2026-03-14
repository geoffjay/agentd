/**
 * Tests for MemoryCard component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryCard } from '@/components/memories/MemoryCard'
import type { Memory } from '@/types/memory'

function makeMemory(overrides: Partial<Memory> = {}): Memory {
  return {
    id: 'mem_1234567890_abcdef12',
    content: 'Test memory content for display',
    type: 'information',
    tags: ['test', 'ui'],
    created_by: 'agent-1',
    owner: undefined,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    visibility: 'public',
    shared_with: [],
    references: [],
    ...overrides,
  }
}

describe('MemoryCard', () => {
  it('renders memory content', () => {
    const mem = makeMemory()
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    expect(screen.getByText('Test memory content for display')).toBeTruthy()
  })

  it('renders type badge', () => {
    const mem = makeMemory({ type: 'question' })
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    expect(screen.getByText('Question')).toBeTruthy()
  })

  it('renders visibility badge', () => {
    const mem = makeMemory({ visibility: 'private' })
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    expect(screen.getByText('private')).toBeTruthy()
  })

  it('renders tags as chips', () => {
    const mem = makeMemory({ tags: ['alpha', 'beta'] })
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    expect(screen.getByText('alpha')).toBeTruthy()
    expect(screen.getByText('beta')).toBeTruthy()
  })

  it('renders creator', () => {
    const mem = makeMemory({ created_by: 'test-user' })
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    expect(screen.getByText('test-user')).toBeTruthy()
  })

  it('truncates long content and shows expand toggle', () => {
    const longContent = 'A'.repeat(250)
    const mem = makeMemory({ content: longContent })
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    // Content should be truncated
    expect(screen.queryByText(longContent)).toBeNull()
    // Expand toggle should be visible
    expect(screen.getByText('Show more')).toBeTruthy()
  })

  it('expands content on toggle click', () => {
    const longContent = 'A'.repeat(250)
    const mem = makeMemory({ content: longContent })
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    fireEvent.click(screen.getByText('Show more'))
    expect(screen.getByText(longContent)).toBeTruthy()
    expect(screen.getByText('Show less')).toBeTruthy()
  })

  it('calls onDelete when delete button is clicked', () => {
    const onDelete = vi.fn()
    const mem = makeMemory()
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={onDelete} />,
    )
    fireEvent.click(screen.getByLabelText('Delete memory'))
    expect(onDelete).toHaveBeenCalledWith(mem.id)
  })

  it('calls onEditVisibility when visibility button is clicked', () => {
    const onEditVisibility = vi.fn()
    const mem = makeMemory()
    render(
      <MemoryCard memory={mem} onEditVisibility={onEditVisibility} onDelete={vi.fn()} />,
    )
    fireEvent.click(screen.getByLabelText('Edit visibility'))
    expect(onEditVisibility).toHaveBeenCalledWith(mem)
  })

  it('shows shared_with when visibility is shared', () => {
    const mem = makeMemory({
      visibility: 'shared',
      shared_with: ['user-a', 'user-b'],
    })
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    expect(screen.getByText(/Shared with:/)).toBeTruthy()
    expect(screen.getByText(/user-a, user-b/)).toBeTruthy()
  })

  it('shows reference count when present', () => {
    const mem = makeMemory({ references: ['ref-1', 'ref-2'] })
    render(
      <MemoryCard memory={mem} onEditVisibility={vi.fn()} onDelete={vi.fn()} />,
    )
    expect(screen.getByText('2 refs')).toBeTruthy()
  })
})
